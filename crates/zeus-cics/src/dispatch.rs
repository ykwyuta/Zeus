//! Tasks as coroutines, as the quasi-reentrant TCB of CICS runs them: a
//! task runs on its own stack until it waits (for terminal input, for a
//! time, for an enqueued resource) or has used its time slice; the
//! dispatcher of the server then runs other tasks and resumes it when its
//! wait is over. Outside a coroutine (zcics scripts, tasks run one after
//! the other) the waits do not suspend.
//!
//! This is the only module with unsafe code: the running coroutine's
//! yielder is reached through a pointer, so that a wait deep inside a
//! program (an EXEC CICS command, the interpreter's time slice) can
//! suspend it.
#![allow(unsafe_code)]

use std::cell::Cell;
use std::time::{Duration, Instant};

use corosensei::stack::DefaultStack;
use corosensei::{Coroutine, CoroutineResult, Yielder};

/// What a suspended task waits for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    /// The next input of its terminal (a conversational RECEIVE).
    Input,
    /// A time (DELAY).
    Until(Instant),
    /// Nothing: it gave up the processor (end of its time slice, a busy
    /// resource it asks for again).
    Dispatch,
}

/// What a suspended task is resumed with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resume {
    /// The attention key of the input it waited for; None when no input
    /// will come (the terminal disconnected).
    Input(Option<u8>),
    Go,
}

type TaskYielder = Yielder<Resume, Wait>;

thread_local! {
    /// The yielder of the coroutine running on this thread (null: none).
    static CURRENT: Cell<*const TaskYielder> = const { Cell::new(std::ptr::null()) };
    /// When the running task was last dispatched.
    static SLICE_START: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// The stack of a task (allocated as it is used).
const STACK: usize = 64 << 20;

/// How long a task runs before others get the processor.
const SLICE: Duration = Duration::from_millis(50);

/// Whether a task coroutine runs (waits suspend it).
pub fn in_task() -> bool {
    !CURRENT.get().is_null()
}

/// Suspends the running task until `w` is over; None outside a task.
pub fn suspend(w: Wait) -> Option<Resume> {
    let y = CURRENT.get();
    if y.is_null() {
        return None;
    }
    // SAFETY: CURRENT is not null only while the coroutine whose body owns
    // the yielder runs on this thread (`Task::resume` clears it when the
    // coroutine suspends or ends, the body sets it again when resumed), so
    // the yielder is alive and is used by its own coroutine.
    let r = unsafe { &*y }.suspend(w);
    CURRENT.set(y);
    SLICE_START.set(Some(Instant::now()));
    Some(r)
}

/// Gives up the processor when the running task has used its time slice
/// (called by the interpreter now and then).
pub fn time_slice() {
    if in_task() && SLICE_START.get().is_some_and(|t| t.elapsed() >= SLICE) {
        suspend(Wait::Dispatch);
    }
}

/// Waits until `t`: other tasks run meanwhile, or the thread sleeps.
pub fn delay_until(t: Instant) {
    if suspend(Wait::Until(t)).is_none() {
        std::thread::sleep(t.saturating_duration_since(Instant::now()));
    }
}

/// A task coroutine.
pub struct Task {
    co: Coroutine<Resume, Wait, (), DefaultStack>,
    /// What it waits for (before its first dispatch: nothing).
    pub wait: Wait,
}

impl Task {
    pub fn new(f: impl FnOnce() + 'static) -> Task {
        let stack = DefaultStack::new(STACK).expect("zcics: no memory for a task stack");
        let co = Coroutine::with_stack(stack, move |y: &TaskYielder, _: Resume| {
            CURRENT.set(y as *const TaskYielder);
            f();
            CURRENT.set(std::ptr::null());
        });
        Task {
            co,
            wait: Wait::Dispatch,
        }
    }

    /// Runs the task until it waits or ends; true when it ended.
    pub fn resume(&mut self, r: Resume) -> bool {
        let prev = CURRENT.replace(std::ptr::null());
        let slice = SLICE_START.replace(Some(Instant::now()));
        // a panic of the task goes on in the dispatcher, with the state
        // of the thread as it was
        struct Restore(*const TaskYielder, Option<Instant>);
        impl Drop for Restore {
            fn drop(&mut self) {
                CURRENT.set(self.0);
                SLICE_START.set(self.1);
            }
        }
        let _restore = Restore(prev, slice);
        match self.co.resume(r) {
            CoroutineResult::Yield(w) => {
                self.wait = w;
                false
            }
            CoroutineResult::Return(()) => true,
        }
    }

    /// Whether the task can run now (with what to resume it with), given
    /// the input its terminal has for it.
    pub fn ready(&self, input: &mut Option<Option<u8>>, now: Instant) -> Option<Resume> {
        match self.wait {
            Wait::Dispatch => Some(Resume::Go),
            Wait::Until(t) => (t <= now).then_some(Resume::Go),
            Wait::Input => input.take().map(Resume::Input),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn deep(n: u32) -> u32 {
        if n == 0 {
            match suspend(Wait::Input) {
                Some(Resume::Input(Some(a))) => a as u32,
                _ => 0,
            }
        } else {
            deep(n - 1) + 1
        }
    }

    #[test]
    fn tasks_interleave() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let (l1, l2) = (log.clone(), log.clone());
        let mut a = Task::new(move || {
            l1.borrow_mut().push("a1");
            let v = deep(100);
            l1.borrow_mut().push(if v == 107 { "a2" } else { "a?" });
        });
        let mut b = Task::new(move || {
            l2.borrow_mut().push("b1");
            assert_eq!(suspend(Wait::Dispatch), Some(Resume::Go));
            l2.borrow_mut().push("b2");
        });
        assert!(!a.resume(Resume::Go));
        assert_eq!(a.wait, Wait::Input);
        assert!(!in_task());
        assert!(!b.resume(Resume::Go));
        assert_eq!(b.wait, Wait::Dispatch);
        assert!(b.resume(Resume::Go));
        assert!(a.resume(Resume::Input(Some(7))));
        assert_eq!(*log.borrow(), ["a1", "b1", "b2", "a2"]);
        // outside a task nothing suspends
        assert_eq!(suspend(Wait::Dispatch), None);
    }
}

//! A run unit shared by host threads. Requests are serialized on an owning
//! worker, so WS, EXTERNAL, files and runtime state have one consistent owner;
//! each invocation still receives freshly initialized LOCAL-STORAGE.
use super::Machine;
use crate::interlang::{XEnd, XRef};
use std::{
    io::Write,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

#[derive(Debug)]
pub struct CallResult {
    pub end: XEnd,
    /// Reference parameters copied back to the caller (None is OMITTED).
    pub arguments: Vec<Option<Vec<u8>>>,
    pub output: Vec<u8>,
}
struct Invocation {
    name: String,
    arguments: Vec<Option<Vec<u8>>>,
    reply: mpsc::Sender<Result<CallResult, String>>,
}
enum Request {
    Call(Invocation),
    Finish(mpsc::Sender<Result<(), String>>),
}
/// Clone this handle into host threads. A completed call observes all writes
/// of earlier calls in this run unit; separate handles created with spawn
/// create separate run units. No VM or Rc is transferred between threads.
#[derive(Clone)]
pub struct ThreadedRunUnit {
    sender: mpsc::Sender<Request>,
    finished: Arc<AtomicBool>,
}
#[derive(Clone, Default)]
struct Output(Arc<Mutex<Vec<u8>>>);
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("thread output poisoned"))?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl ThreadedRunUnit {
    /// Configure native services, DDs, or EXEC hosts on the owner thread.
    /// All COBOL programs entered in this run unit must use THREAD.
    pub fn spawn(
        search_dirs: Vec<PathBuf>,
        configure: impl FnOnce(&mut Machine) + Send + 'static,
    ) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::channel::<Request>();
        let finished = Arc::new(AtomicBool::new(false));
        let worker_finished = finished.clone();
        std::thread::Builder::new()
            .name("zeus-cobol-run-unit".into())
            .spawn(move || {
                let mut machine = Machine::new(search_dirs);
                configure(&mut machine);
                machine.threaded_run_unit = true;
                let output = Output::default();
                machine.set_stdout(Box::new(output.clone()));
                // Reuse parameter storage rather than growing it on every call.
                let mut slots: Vec<(usize, usize)> = Vec::new();
                for request in receiver {
                    let request = match request {
                        Request::Call(request) => request,
                        Request::Finish(reply) => {
                            let result = machine.try_finish().map_err(|error| error.0);
                            worker_finished.store(true, Ordering::Release);
                            let _ = reply.send(result);
                            return;
                        }
                    };
                    output.0.lock().expect("thread output").clear();
                    let mut refs = Vec::new();
                    for (index, argument) in request.arguments.iter().enumerate() {
                        if slots.len() <= index {
                            slots.push((0, 0));
                        }
                        let mut reference = XRef::default();
                        if let Some(bytes) = argument {
                            if slots[index].1 < bytes.len().max(1) {
                                slots[index] =
                                    (machine.getmain(bytes.len().max(1), 0), bytes.len().max(1));
                            }
                            let address = slots[index].0;
                            machine.mem[address..address + bytes.len()].copy_from_slice(bytes);
                            reference.addr = Some(address);
                            reference.len = bytes.len();
                        }
                        refs.push(reference);
                    }
                    let result = machine.call_program(&request.name, &refs).and_then(|end| {
                        let end =
                            end.ok_or_else(|| format!("program {} not found", request.name))?;
                        let arguments = refs
                            .iter()
                            .map(|arg| {
                                arg.addr
                                    .map(|address| machine.mem[address..address + arg.len].to_vec())
                            })
                            .collect();
                        let output = output
                            .0
                            .lock()
                            .map_err(|_| "thread output poisoned".to_string())?
                            .clone();
                        Ok(CallResult {
                            end,
                            arguments,
                            output,
                        })
                    });
                    let stop = result.as_ref().is_ok_and(|result| result.end.stop);
                    let failed = result.is_err();
                    // STOP RUN or an unhandled runtime error ends the shared run
                    // unit; pending callers receive a disconnected reply channel.
                    if stop || failed {
                        let cleanup = machine.try_finish().map_err(|error| error.0);
                        worker_finished.store(true, Ordering::Release);
                        let _ = request
                            .reply
                            .send(result.and_then(|result| cleanup.map(|_| result)));
                        return;
                    }
                    let _ = request.reply.send(result);
                }
                machine.finish();
                worker_finished.store(true, Ordering::Release);
            })?;
        Ok(Self { sender, finished })
    }
    pub fn call(&self, name: &str, arguments: Vec<Option<Vec<u8>>>) -> Result<CallResult, String> {
        if self.finished.load(Ordering::Acquire) {
            return Err("threaded run unit has ended".into());
        }
        let (reply, result) = mpsc::channel();
        self.sender
            .send(Request::Call(Invocation {
                name: name.into(),
                arguments,
                reply,
            }))
            .map_err(|_| "threaded run unit has ended".to_string())?;
        result
            .recv()
            .map_err(|_| "threaded run unit has ended".to_string())?
    }
    /// End this run unit for every handle, waiting for open files and
    /// coverage to be flushed. Call after joining the host callers when
    /// programs returned with GOBACK instead of STOP RUN.
    pub fn finish(&self) -> Result<(), String> {
        if self.finished.load(Ordering::Acquire) {
            return Ok(());
        }
        let (reply, finished) = mpsc::channel();
        if self.sender.send(Request::Finish(reply)).is_err() {
            return Ok(());
        }
        match finished.recv() {
            Ok(result) => result,
            Err(_) if self.finished.load(Ordering::Acquire) => Ok(()),
            Err(_) => Err("threaded run unit ended before shutdown completed".into()),
        }
    }
}

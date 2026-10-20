//! The time of day programs see (CURRENT-DATE, ACCEPT FROM DATE/TIME, LE
//! services, data set dates): the system's, or a fixed one for runs that
//! must repeat (test generation).

use std::cell::Cell;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

thread_local! {
    static FIXED: Cell<Option<Duration>> = const { Cell::new(None) };
}

/// Fixes the time (since 1970-01-01 UTC) for this thread; None: the
/// system's.
pub fn fix(t: Option<Duration>) {
    FIXED.with(|f| f.set(t));
}

/// The time since 1970-01-01 UTC.
pub fn now() -> Duration {
    FIXED.with(|f| f.get()).unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
    })
}

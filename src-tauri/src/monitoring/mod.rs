// The monitoring engine. Engine-internal: nothing here crosses the IPC boundary
// except what 2b explicitly exposes through the overview command. The allow is
// because these types are deliberately not all wired up yet, and a warning on
// every build would train us to ignore real ones.
#![allow(dead_code)]

pub mod config;
pub mod health;
pub mod probe;
pub mod runner;
pub mod state;
pub mod store;
pub mod testing;

use std::time::{SystemTime, UNIX_EPOCH};

/// Milliseconds since the Unix epoch, or zero if the system clock is set before
/// it.
///
/// One definition, because the clock is load bearing in two places at once: it
/// stamps every sample on the way in, and it decides which samples are still
/// inside the window on the way out. Two copies of this would eventually
/// disagree, and the disagreement would only show up as stale data.
pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since_epoch| since_epoch.as_millis() as i64)
        .unwrap_or(0)
}

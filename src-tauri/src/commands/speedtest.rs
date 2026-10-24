//! The command the interface calls to run a speed test.

use tauri::{AppHandle, Emitter};

use crate::speedtest::{Progress, SpeedTestOutcome, SpeedTestProgress, run};

/// Publishes the running test's progress to the interface.
///
/// The interface moves its needle from these, so the needle is showing a rate
/// that was really measured rather than an animation that only looks like one.
struct EventProgress {
    app: AppHandle,
}

impl Progress for EventProgress {
    fn report(&self, progress: SpeedTestProgress) {
        // A failure to publish is not a failure of the test. The test carries
        // on and reports its figures at the end either way.
        let _ = self.app.emit(crate::speedtest::PROGRESS_EVENT, progress);
    }
}

/// Runs an internet speed test and reports what it measured.
///
/// The test moves about thirty megabytes and takes several seconds, and it is
/// blocking work, so it runs on a thread of its own.
///
/// That matters more than it sounds. A command without the `async` keyword runs
/// on the main thread, and the main thread is the one that draws the window: a
/// test on it would freeze the whole app for as long as the test took. The
/// interface would show a spinner that could not animate and a window that
/// would not respond to the pointer.
#[tauri::command]
pub async fn run_speed_test(app: AppHandle) -> SpeedTestOutcome {
    let progress = EventProgress { app };

    match tauri::async_runtime::spawn_blocking(move || run(&progress)).await {
        Ok(outcome) => outcome,
        Err(error) => SpeedTestOutcome::Failed {
            message: format!("The speed test could not be started: {error}"),
        },
    }
}

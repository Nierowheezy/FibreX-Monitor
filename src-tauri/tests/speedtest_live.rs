// Live check of the speed test, so the numbers are real before they are shown.
//
// Ignored by default, because it needs the internet.

use fibrex_monitor_lib::speedtest::{RecordedProgress, run};

#[test]
#[ignore = "needs the internet"]
fn the_speed_test_measures_a_real_line() {
    let progress = RecordedProgress::default();
    let outcome = run(&progress);
    println!("outcome: {outcome:?}");
    println!(
        "reports: {}",
        progress.reports.lock().map(|r| r.len()).unwrap_or(0)
    );

    match outcome {
        fibrex_monitor_lib::speedtest::SpeedTestOutcome::Completed { result } => {
            println!("latency:  {:?} ms", result.latency_ms);
            println!("download: {:?} B/s", result.download_bytes_per_second);
            println!("upload:   {:?} B/s", result.upload_bytes_per_second);
            assert!(result.latency_ms.is_some(), "the host must answer");
            assert!(
                result.download_bytes_per_second.is_some(),
                "the download must measure"
            );
        }
        fibrex_monitor_lib::speedtest::SpeedTestOutcome::Failed { message } => {
            panic!("the speed test failed: {message}");
        }
    }
}

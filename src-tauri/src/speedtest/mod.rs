//! An internet speed test.
//!
//! What a speed test should measure is the rate the line can sustain, and the
//! figures people quote to each other — the ones from speedtest.net and its
//! peers — are measured a particular way: several connections at once, for a
//! window of time, counting the steady rate and not the opening of the
//! transfer. This module measures the same way, so its numbers mean the same
//! thing as the ones a person compares them against.
//!
//! Two things a one-request measurement gets wrong, and why each matters:
//!
//! **One connection cannot fill the line.** A single TCP stream's speed is
//! negotiated over the round trip, and it settles where the congestion window
//! and the round trip let it, which on a line with a long round trip can be
//! well under what the same line delivers to several streams at once. Every
//! test people use runs several connections; so does this one.
//!
//! **The opening of a transfer is not its rate.** The first second is spent
//! setting the connection up and ramping TCP toward the line's rate. Counting
//! that in the average reads a nearer number than the line's, so the first
//! second is thrown away and the figure is taken from what moved after it.
//!
//! The host is Cloudflare's speed test endpoint, which needs no account and no
//! key, and which is the same network the app's own probe target sits on. A
//! measurement against a host the user cannot reach would be a measurement of
//! nothing, so a failure is reported as a failure rather than as a number.
//!
//! One honesty note about the figures, since a person will compare them with a
//! website's: the round trip here is to Cloudflare's edge, and a website test
//! usually runs against a server in town. Both numbers are real; they are
//! measurements of different paths, and this module measures the one it says
//! it does.
//!
//! Nothing here is a probe of the router. It measures the line to the
//! internet, which is what a speed test is for, and it is kept apart from the
//! monitoring loop so a test never becomes an incident.

use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// The event the running test publishes, so the interface can move its needle
/// while the test is still going.
pub const PROGRESS_EVENT: &str = "speedtest://progress";

/// How many connections the transfer phases run at once.
const STREAMS: usize = 6;

/// How long each transfer phase runs for.
const MEASURE_SECONDS: u64 = 10;

/// The head of a transfer phase that is thrown away.
///
/// It is spent setting connections up and ramping TCP toward the line's rate.
/// Counting it would mix a ramp into a rate.
const WARMUP: Duration = Duration::from_secs(1);

/// How often the running figures are observed and published.
const REPORT_EVERY: Duration = Duration::from_millis(150);

/// How many bytes one download request asks for.
///
/// Twenty megabytes. Each connection asks again when a request finishes, so
/// the requests are the rhythm of the window rather than its length; the size
/// keeps per-request overhead small and a single request under the agent's
/// timeout even on a slow line.
const DOWNLOAD_CHUNK: u64 = 20_000_000;

/// How many small requests the latency is taken from.
///
/// One request is a single sample of a number that varies, and several are
/// steadier and cost nothing. The median of the samples is shown, because a
/// median says where the round trip usually sits while letting one slow sample
/// fall where it falls.
const LATENCY_SAMPLES: usize = 6;

/// The longest a single request may take.
const TEST_TIMEOUT: Duration = Duration::from_secs(30);

/// The host the test runs against.
const TEST_HOST: &str = "speed.cloudflare.com";

/// Which part of the test is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SpeedTestPhase {
    /// Measuring the round trip to the host.
    Ping,
    /// Measuring how fast data comes down.
    Download,
    /// Measuring how fast data goes up.
    Upload,
}

/// What the running test has got so far.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedTestProgress {
    pub phase: SpeedTestPhase,
    /// The rate so far in this phase, in bytes per second.
    ///
    /// Null in the ping phase, where there is no rate.
    pub bytes_per_second: Option<f64>,
    /// How many bytes have moved in this phase so far.
    pub bytes_so_far: Option<u64>,
    /// The latest round-trip sample, in milliseconds.
    ///
    /// Non-null only in the ping phase, where each sample is worth showing as
    /// it lands, the way a speed test's dial counts its pings up. Null
    /// elsewhere; the interface keeps the last one it saw.
    pub latency_ms: Option<f64>,
}

/// The result of a speed test.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedTestResult {
    /// Round trip time to the test host, in milliseconds.
    pub latency_ms: Option<f64>,
    /// How much the round trip varied between the samples, in milliseconds.
    ///
    /// A steady line has a low jitter, and a line that swings has a high one.
    /// It is the mean of the differences between consecutive samples, which is
    /// the same definition the common speed tests use.
    pub jitter_ms: Option<f64>,
    /// Bytes per second coming down.
    pub download_bytes_per_second: Option<f64>,
    /// Bytes per second going up.
    pub upload_bytes_per_second: Option<f64>,
    /// The host the test ran against.
    pub host: String,
    /// When the test finished, UTC epoch milliseconds.
    pub finished_at: i64,
}

/// How a test ended.
///
/// Tagged with a `kind` field, so the interface can tell the two apart with one
/// check rather than by looking for which field is present.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SpeedTestOutcome {
    /// The test ran and these are its figures.
    Completed { result: SpeedTestResult },
    /// The test could not run, and this is why.
    Failed { message: String },
}

/// Where the running test publishes its progress.
///
/// A trait rather than the app handle itself, so the measurement code can be
/// tested without an app, and so a caller that does not want progress can
/// ignore it.
pub trait Progress {
    fn report(&self, progress: SpeedTestProgress);
}

/// A progress sink that keeps the last report, for tests.
#[derive(Default)]
pub struct RecordedProgress {
    pub reports: std::sync::Mutex<Vec<SpeedTestProgress>>,
}

impl Progress for RecordedProgress {
    fn report(&self, progress: SpeedTestProgress) {
        self.reports
            .lock()
            .expect("progress poisoned")
            .push(progress);
    }
}

/// Runs the test.
///
/// The three measurements are independent, so one of them failing does not lose
/// the others. A line that uploads at nothing but downloads at 200 Mbps is a
/// real and common situation, and the interface should be able to say so.
pub fn run(progress: &dyn Progress) -> SpeedTestOutcome {
    let agent = agent();

    // The download runs first, so the latency that follows is the round trip
    // over connections the pool has already opened, rather than the cost of
    // opening one. A speed test that reports its setup time as its ping is
    // reporting the wrong number.
    let download = measure_download(&agent, progress);
    let (latency, jitter) = measure_latency(&agent, progress);
    let upload = measure_upload(&agent, progress);

    if latency.is_none() && download.is_none() && upload.is_none() {
        return SpeedTestOutcome::Failed {
            message: format!("{TEST_HOST} could not be reached, so there is nothing to measure."),
        };
    }

    SpeedTestOutcome::Completed {
        result: SpeedTestResult {
            latency_ms: latency,
            jitter_ms: jitter,
            download_bytes_per_second: download,
            upload_bytes_per_second: upload,
            host: TEST_HOST.to_string(),
            finished_at: crate::monitoring::now_ms(),
        },
    }
}

/// The agent the test uses.
///
/// One agent, built once, and every connection shares its pool. The transfer
/// phases clone it for their threads, which is cheap, and it means the
/// connections those threads open are the same pool the latency samples later
/// reuse.
pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(TEST_TIMEOUT))
        .build()
        .into()
}

/// The shared state one transfer measurement runs between.
///
/// A window of time, a counter of everything that actually moved, and a flag
/// the workers watch so nothing keeps going after the window closes.
struct Transfer {
    started: Instant,
    deadline: Instant,
    moved: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
}

impl Transfer {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            deadline: Instant::now() + Duration::from_secs(MEASURE_SECONDS),
            moved: Arc::new(AtomicU64::new(0)),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// How fast data comes down.
///
/// Six connections each pull blocks for ten seconds, and the figure is the
/// bytes that moved after the first second, divided by the time after the
/// first second.
fn measure_download(agent: &ureq::Agent, progress: &dyn Progress) -> Option<f64> {
    let transfer = Transfer::new();
    let mut workers = Vec::new();

    for _ in 0..STREAMS {
        let agent = agent.clone();
        let moved = Arc::clone(&transfer.moved);
        let stop = Arc::clone(&transfer.stop);
        let deadline = transfer.deadline;

        workers.push(thread::spawn(move || {
            let mut buffer = vec![0u8; 256 * 1024];

            while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
                let Ok(mut response) = agent
                    .get(&format!(
                        "https://{TEST_HOST}/__down?bytes={DOWNLOAD_CHUNK}"
                    ))
                    .call()
                else {
                    // A request that fails is not a reason to give up while the
                    // others may still be moving data, but it must not become a
                    // tight loop either.
                    thread::sleep(Duration::from_millis(250));
                    continue;
                };

                let mut reader = response.body_mut().as_reader();
                loop {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(count) => {
                            moved.fetch_add(count as u64, Ordering::Relaxed);
                        }
                        Err(_) => break,
                    }
                }
            }
        }));
    }

    let rate = observe_window(progress, SpeedTestPhase::Download, &transfer);
    transfer.stop.store(true, Ordering::Relaxed);

    for worker in workers {
        let _ = worker.join();
    }

    rate
}

/// How fast data goes up.
///
/// Six connections each push blocks of zeroes for ten seconds, and the figure
/// is the bytes produced to the host after the first second, divided by the
/// time after the first second.
///
/// The push is continuous, the way a speed test's own upload is: one stream of
/// data per connection that only ends when the window ends, rather than a
/// series of one-shot bodies. A series of fixed bodies would carry the
/// opening ramp of TCP on every request, and that ramp read into the figure is
/// part of what makes an upload read slow when the line is not.
///
/// The body is zeroes because the content does not matter to a throughput
/// measurement, and a block of zeroes compresses to nothing on the way, so it
/// is not sent as an incompressible pattern, which would cost the CPU and
/// measure the machine rather than the line.
fn measure_upload(agent: &ureq::Agent, progress: &dyn Progress) -> Option<f64> {
    let transfer = Transfer::new();
    let mut workers = Vec::new();

    for _ in 0..STREAMS {
        let agent = agent.clone();
        let moved = Arc::clone(&transfer.moved);
        let stop = Arc::clone(&transfer.stop);
        let deadline = transfer.deadline;

        workers.push(thread::spawn(move || {
            // The stream produces zeroes until the window ends and produces
            // none after, so the body ends exactly with the measurement and
            // nothing is counted or sent beyond the window. What it produces
            // is what it counts.
            let stream = UploadStream {
                deadline,
                stop,
                moved,
            };

            let _ = agent
                .post(&format!("https://{TEST_HOST}/__up"))
                .send(ureq::SendBody::from_owned_reader(stream));
        }));
    }

    let rate = observe_window(progress, SpeedTestPhase::Upload, &transfer);
    transfer.stop.store(true, Ordering::Relaxed);

    for worker in workers {
        let _ = worker.join();
    }

    rate
}

/// The body one upload connection pushes: zeroes, counted as they are handed
/// over, until the window ends.
struct UploadStream {
    deadline: Instant,
    stop: Arc<AtomicBool>,
    moved: Arc<AtomicU64>,
}

impl Read for UploadStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.stop.load(Ordering::Relaxed) || Instant::now() >= self.deadline {
            // The body ends here, which is what closes the chunked request.
            return Ok(0);
        }

        let count = buf.len();
        buf.fill(0);
        self.moved.fetch_add(count as u64, Ordering::Relaxed);
        Ok(count)
    }
}

/// Watches a transfer window, publishing the running figures and returning the
/// window's own.
///
/// The running rate is published every report interval, taken across the whole
/// window from wherever its warm-up boundary sits, so the figure the interface
/// shows climbs onto the steady rate as the ramp gets left behind.
///
/// The window's own figure is taken from the last observed sample, which is
/// inside one report interval of the deadline. Nothing after it is counted, so
/// the figure can never exceed what actually moved in the window it names.
fn observe_window(
    progress: &dyn Progress,
    phase: SpeedTestPhase,
    transfer: &Transfer,
) -> Option<f64> {
    // Where the counted window starts: the moment the warm-up was seen to end.
    // Until then the running rate is the whole window, ramp included, and the
    // interface can watch it rise.
    let mut boundary: Option<(Instant, u64)> = None;
    let mut last: Option<(Instant, u64)> = None;

    loop {
        thread::sleep(REPORT_EVERY);

        let now = Instant::now();
        if now >= transfer.deadline {
            break;
        }

        let bytes = transfer.moved.load(Ordering::Relaxed);
        if boundary.is_none() && now.duration_since(transfer.started) >= WARMUP {
            boundary = Some((now, bytes));
        }

        let (base_time, base_bytes) = boundary.unwrap_or((transfer.started, 0));
        let elapsed = now.duration_since(base_time).as_secs_f64();
        let rate = if elapsed > 0.0 {
            (bytes.saturating_sub(base_bytes)) as f64 / elapsed
        } else {
            0.0
        };

        progress.report(SpeedTestProgress {
            phase,
            bytes_per_second: Some(rate),
            bytes_so_far: Some(bytes),
            latency_ms: None,
        });

        last = Some((now, bytes));
    }

    let (end_time, end_bytes) = last.unwrap_or((transfer.started, 0));
    let (base_time, base_bytes) = boundary.unwrap_or((transfer.started, 0));

    if end_bytes == 0 {
        // Nothing moved at all. That is not a rate of zero; the host was not
        // reachable for data.
        return None;
    }

    let window = end_time.duration_since(base_time).as_secs_f64();
    if window <= 0.0 {
        // Degenerate windows only happen when the observation never properly
        // started, which is the same fact as nothing moving.
        return None;
    }

    let sustained = end_bytes.saturating_sub(base_bytes);
    if sustained == 0 {
        // Everything that moved happened during the warm-up, and then the
        // path went quiet. A rate of zero would claim the line moved nothing
        // while carrying a working connection, which is a different fact from
        // the measurement not having been able to finish. Unavailable it is.
        return None;
    }

    Some(sustained as f64 / window)
}

/// How long the host takes to answer, and how much that answer varied.
///
/// Measured as the time for a request that returns nothing, so the number is
/// the round trip and not the transfer. The samples reuse the connections the
/// download phase opened, so a sample is the round trip and not the cost of
/// setting a connection up.
fn measure_latency(agent: &ureq::Agent, progress: &dyn Progress) -> (Option<f64>, Option<f64>) {
    let mut samples: Vec<f64> = Vec::new();

    for _ in 0..LATENCY_SAMPLES {
        let started = Instant::now();
        let Ok(mut response) = agent
            .get(&format!("https://{TEST_HOST}/__down?bytes=0"))
            .call()
        else {
            break;
        };
        // The body is drained so the connection is not left half read.
        let _ = response.body_mut().read_to_vec();
        samples.push(started.elapsed().as_secs_f64() * 1000.0);

        // Each sample is worth publishing as it lands, because the interface
        // can count the pings up the way a speed test's dial does. A null rate
        // keeps the needle honest: a round trip is not a rate.
        progress.report(SpeedTestProgress {
            phase: SpeedTestPhase::Ping,
            bytes_per_second: None,
            bytes_so_far: None,
            latency_ms: samples.last().copied(),
        });
    }

    if samples.is_empty() {
        return (None, None);
    }

    // Jitter is the mean of the differences between consecutive samples. With
    // one sample there is no difference to take, so there is no jitter to
    // report, which is different from a jitter of zero.
    let jitter = if samples.len() > 1 {
        let differences: Vec<f64> = samples
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .collect();
        Some(differences.iter().sum::<f64>() / differences.len() as f64)
    } else {
        None
    };

    (Some(median(&samples)), jitter)
}

/// The middle of a list of numbers, or the mean of the two middle ones.
fn median(samples: &[f64]) -> f64 {
    let mut sorted: Vec<f64> = samples.to_vec();
    sorted.sort_by(|left, right| left.total_cmp(right));

    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitoring::now_ms;

    /// The result shape has to stay in step with the interface's copy of it.
    #[test]
    fn the_result_carries_the_four_figures_the_interface_shows() {
        let result = SpeedTestResult {
            latency_ms: Some(24.0),
            jitter_ms: Some(1.5),
            download_bytes_per_second: Some(5_000_000.0),
            upload_bytes_per_second: Some(500_000.0),
            host: "speed.cloudflare.com".to_string(),
            finished_at: now_ms(),
        };

        let json = serde_json::to_value(&result).expect("the result must serialise");

        assert_eq!(json["latencyMs"], serde_json::json!(24.0));
        assert_eq!(json["jitterMs"], serde_json::json!(1.5));
        assert!(json["downloadBytesPerSecond"].is_number());
        assert!(json["uploadBytesPerSecond"].is_number());
    }

    /// A failure has to say what failed and not carry a number.
    #[test]
    fn a_failure_carries_no_figures() {
        let outcome = SpeedTestOutcome::Failed {
            message: "speed.cloudflare.com could not be reached.".to_string(),
        };

        let json = serde_json::to_value(&outcome).expect("the outcome must serialise");

        assert_eq!(json["kind"], serde_json::json!("failed"));
        assert!(json["message"].is_string());
    }

    /// A progress report names the phase, so the interface can say which
    /// measurement is running rather than showing a moving needle with no
    /// explanation.
    #[test]
    fn a_progress_report_names_its_phase() {
        let progress = SpeedTestProgress {
            phase: SpeedTestPhase::Download,
            bytes_per_second: Some(1_000_000.0),
            bytes_so_far: Some(2_000_000),
            latency_ms: None,
        };

        let json = serde_json::to_value(&progress).expect("the progress must serialise");

        assert_eq!(json["phase"], serde_json::json!("download"));
        assert!(json["bytesPerSecond"].is_number());
    }

    /// A ping progress report carries the sample that landed, so the interface
    /// can count the pings up while they are being taken.
    #[test]
    fn a_ping_report_carries_its_sample() {
        let progress = SpeedTestProgress {
            phase: SpeedTestPhase::Ping,
            bytes_per_second: None,
            bytes_so_far: None,
            latency_ms: Some(24.0),
        };

        let json = serde_json::to_value(&progress).expect("the progress must serialise");

        assert_eq!(json["phase"], serde_json::json!("ping"));
        assert_eq!(json["latencyMs"], serde_json::json!(24.0));
        assert!(json["bytesPerSecond"].is_null());
    }

    /// The median is the middle for odd counts and the mean of the two middle
    /// values for even ones, and it does not care about the order the samples
    /// arrived in.
    #[test]
    fn the_median_is_the_middle_of_the_samples() {
        assert_eq!(median(&[3.0, 1.0, 5.0]), 3.0);
        assert_eq!(median(&[3.0, 1.0]), 2.0);
        assert_eq!(median(&[40.0, 170.0, 180.0, 172.0]), 171.0);
    }
}

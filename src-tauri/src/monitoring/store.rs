//! Local SQLite history: every probe, and the incidents derived from them.
//!
//! Two rules shape this file.
//!
//! First, a sample is written exactly as it was measured. A failed probe has no
//! latency column value at all rather than a zero, because a stored zero is
//! indistinguishable from a measured zero and would outlive the code that
//! wrote it.
//!
//! Second, an incident is derived from stored probes, never invented. It records
//! what was observed (probes started failing, then started succeeding again) and
//! how long that lasted. It does not record a cause, because a failed TCP
//! connect is not evidence of one.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use super::probe::{ProbeErrorKind, ProbeOutcome};

/// The current schema version. Bumping this is what makes a migration run.
pub const SCHEMA_VERSION: i64 = 1;

/// How many consecutive failed probes make an incident.
///
/// One is a blip, not an outage. Two consecutive failures at the shipped
/// two-second interval is roughly a four second gap, which is already longer
/// than a healthy connection ever stalls.
pub const INCIDENT_THRESHOLD: u32 = 2;

/// How much raw history is kept before the oldest samples are dropped.
///
/// Chosen for roughly a fortnight at the shipped interval, which is long enough
/// to answer "was it bad last Tuesday evening" without letting an unbounded
/// table grow forever on a machine that monitors for months.
pub const RETENTION_MS: i64 = 14 * 24 * 60 * 60 * 1_000;

/// A stored probe, exactly as it was recorded.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredSample {
    pub target_id: i64,
    pub captured_at: i64,
    /// `None` for a failed probe. Never coerced to zero.
    pub latency_ms: Option<f64>,
}

/// A stretch of consecutive failed probes, with its recovery.
#[derive(Debug, Clone, PartialEq)]
pub struct Incident {
    pub id: i64,
    pub target_id: i64,
    pub started_at: i64,
    /// `None` while the outage is still open.
    pub recovered_at: Option<i64>,
    /// Failed probes counted so far.
    pub probe_count: u32,
    /// Whether the router answered during the outage, when it was probed at all.
    /// `None` means no router reading covers this window, which is not the same
    /// as the router being down.
    pub router_reachable: Option<bool>,
}

impl Incident {
    /// How long the outage lasted, or has lasted so far.
    pub fn duration_ms(&self, now: i64) -> i64 {
        (self.recovered_at.unwrap_or(now)) - self.started_at
    }

    pub fn is_open(&self) -> bool {
        self.recovered_at.is_none()
    }
}

/// The store failed. Carried rather than panicked so a corrupt or unwritable
/// database degrades the app instead of killing it: monitoring keeps working
/// with history switched off, which is better than no monitoring at all.
#[derive(Debug)]
pub struct StoreError(pub String);

impl std::fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        StoreError(error.to_string())
    }
}

type Result<T> = std::result::Result<T, StoreError>;

/// The local history database.
///
/// One connection behind one mutex. Probe writes and UI reads are both small and
/// local, so a pool would add machinery without buying anything; a single
/// connection also means SQLite never has to resolve a write conflict.
pub struct Store {
    connection: std::sync::Mutex<Connection>,
    path: PathBuf,
}

impl Store {
    /// Opens the database at `path`, creating and migrating it as needed.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| StoreError(format!("could not create {parent:?}: {error}")))?;
        }

        let connection = Connection::open(path)?;
        let store = Store {
            connection: std::sync::Mutex::new(connection),
            path: path.to_path_buf(),
        };
        store.migrate()?;
        Ok(store)
    }

    /// An in-memory database, for tests and for a run that has nowhere to write.
    pub fn in_memory() -> Result<Self> {
        let connection = Connection::open_in_memory()?;
        let store = Store {
            connection: std::sync::Mutex::new(connection),
            path: PathBuf::from(":memory:"),
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Creates the schema, or brings an older one forward.
    ///
    /// `user_version` rather than a table of migrations, because SQLite stores it
    /// in the file header and it cannot be lost by a half-applied migration the
    /// way a bookkeeping row can.
    fn migrate(&self) -> Result<()> {
        let connection = self.conn();

        // WAL keeps a read from blocking the probe write behind it, which matters
        // because the UI polls while the loop is still writing.
        connection.pragma_update(None, "journal_mode", "WAL")?;
        // Referential integrity is off by default in SQLite, and an incident that
        // outlives its samples is worse than no incident.
        connection.pragma_update(None, "foreign_keys", "ON")?;

        let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;

        if version >= SCHEMA_VERSION {
            return Ok(());
        }

        if version < 1 {
            connection.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS probe_sample (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    target_id INTEGER NOT NULL,
                    captured_at INTEGER NOT NULL,
                    success INTEGER NOT NULL,
                    -- NULL for a failure, and never 0.
                    latency_ms REAL,
                    error_kind TEXT
                );

                -- The read path is always \"samples since T\", so the index is on
                -- time rather than on the autoincrement id.
                CREATE INDEX IF NOT EXISTS probe_sample_captured_at
                    ON probe_sample (captured_at);

                CREATE TABLE IF NOT EXISTS incident (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    target_id INTEGER NOT NULL,
                    started_at INTEGER NOT NULL,
                    recovered_at INTEGER,
                    probe_count INTEGER NOT NULL,
                    -- NULL when no router reading covers the outage.
                    router_reachable INTEGER
                );

                CREATE INDEX IF NOT EXISTS incident_started_at
                    ON incident (started_at);
                ",
            )?;
        }

        connection.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(())
    }

    /// Writes one probe and folds it into incident detection, in one transaction.
    ///
    /// Both in one transaction because a sample stored without its incident would
    /// leave the history describing a failure that never became an outage.
    ///
    /// `router_reachable` is the router's state at the moment of this probe, when
    /// a router reading exists at all. It is only recorded onto a new incident,
    /// so the router's state at the moment the outage began is preserved rather
    /// than being overwritten by the state at recovery.
    pub fn record(
        &self,
        outcome: &ProbeOutcome,
        router_reachable: Option<bool>,
    ) -> Result<Option<Incident>> {
        let mut connection = self.conn();
        let transaction = connection.transaction()?;

        transaction.execute(
            "INSERT INTO probe_sample (target_id, captured_at, success, latency_ms, error_kind)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                outcome.target_id,
                outcome.captured_at,
                outcome.success as i64,
                outcome.latency_ms,
                outcome.error_kind.map(error_kind_name),
            ],
        )?;

        let open = open_incident(&transaction, outcome.target_id)?;

        let incident = if outcome.success {
            // A success closes an open incident. If none is open this is the
            // ordinary case and nothing is written.
            match open {
                Some(incident) if incident.probe_count >= INCIDENT_THRESHOLD => {
                    transaction.execute(
                        "UPDATE incident SET recovered_at = ?1 WHERE id = ?2",
                        params![outcome.captured_at, incident.id],
                    )?;
                    Some(Incident {
                        recovered_at: Some(outcome.captured_at),
                        ..incident
                    })
                }
                // Below the threshold it was never an incident, so the partial
                // run is discarded rather than reported as a recovered outage.
                Some(incident) => {
                    transaction
                        .execute("DELETE FROM incident WHERE id = ?1", params![incident.id])?;
                    None
                }
                None => None,
            }
        } else {
            match open {
                Some(incident) => {
                    transaction.execute(
                        "UPDATE incident SET probe_count = probe_count + 1 WHERE id = ?1",
                        params![incident.id],
                    )?;
                    Some(Incident {
                        probe_count: incident.probe_count + 1,
                        ..incident
                    })
                }
                None => {
                    transaction.execute(
                        "INSERT INTO incident
                            (target_id, started_at, recovered_at, probe_count, router_reachable)
                         VALUES (?1, ?2, NULL, 1, ?3)",
                        params![
                            outcome.target_id,
                            outcome.captured_at,
                            router_reachable.map(|value| value as i64),
                        ],
                    )?;
                    Some(Incident {
                        id: transaction.last_insert_rowid(),
                        target_id: outcome.target_id,
                        started_at: outcome.captured_at,
                        recovered_at: None,
                        probe_count: 1,
                        router_reachable,
                    })
                }
            }
        };

        // Retention runs on write so the table cannot grow without a separate
        // scheduler. `captured_at` is the only sensible cutoff, and samples are
        // deleted rather than aggregated because an aggregate would be a derived
        // number with no stored evidence behind it.
        let cutoff = outcome.captured_at - RETENTION_MS;
        transaction.execute(
            "DELETE FROM probe_sample WHERE captured_at < ?1",
            params![cutoff],
        )?;

        transaction.commit()?;

        Ok(incident)
    }

    /// Every incident that reached the threshold, newest first.
    ///
    /// Partial runs below the threshold are excluded, because a single failed
    /// probe is not an outage and listing it would make the app look like it
    /// records problems that did not happen.
    pub fn incidents(&self, limit: u32) -> Result<Vec<Incident>> {
        let connection = self.conn();
        let mut statement = connection.prepare(
            "SELECT id, target_id, started_at, recovered_at, probe_count, router_reachable
               FROM incident
              WHERE probe_count >= ?1
              ORDER BY started_at DESC
              LIMIT ?2",
        )?;

        let rows = statement.query_map(params![INCIDENT_THRESHOLD, limit], |row| {
            Ok(Incident {
                id: row.get(0)?,
                target_id: row.get(1)?,
                started_at: row.get(2)?,
                recovered_at: row.get(3)?,
                probe_count: row.get::<_, i64>(4)? as u32,
                router_reachable: row.get::<_, Option<i64>>(5)?.map(|value| value != 0),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Samples for one target since `since`, oldest first.
    pub fn samples_since(&self, target_id: i64, since: i64) -> Result<Vec<StoredSample>> {
        let connection = self.conn();
        let mut statement = connection.prepare(
            "SELECT target_id, captured_at, latency_ms
               FROM probe_sample
              WHERE target_id = ?1 AND captured_at >= ?2
              ORDER BY captured_at ASC",
        )?;

        let rows = statement.query_map(params![target_id, since], |row| {
            Ok(StoredSample {
                target_id: row.get(0)?,
                captured_at: row.get(1)?,
                latency_ms: row.get(2)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// How many probes were made and how many failed, since `since`.
    pub fn counts_since(&self, target_id: i64, since: i64) -> Result<(u32, u32)> {
        let connection = self.conn();
        let row = connection.query_row(
            "SELECT COUNT(*), COALESCE(SUM(success = 0), 0)
               FROM probe_sample
              WHERE target_id = ?1 AND captured_at >= ?2",
            params![target_id, since],
            |row| Ok((row.get::<_, i64>(0)? as u32, row.get::<_, i64>(1)? as u32)),
        )?;

        Ok(row)
    }

    /// The oldest stored sample time, or `None` on an empty database.
    ///
    /// Read by the interface so a range can say what it actually covers rather
    /// than assuming the whole retention window is populated.
    pub fn earliest_sample_at(&self, target_id: i64) -> Result<Option<i64>> {
        let connection = self.conn();
        let value = connection
            .query_row(
                "SELECT MIN(captured_at) FROM probe_sample WHERE target_id = ?1",
                params![target_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten();

        Ok(value)
    }

    /// The newest stored sample time, or `None` on an empty database.
    pub fn latest_sample_at(&self, target_id: i64) -> Result<Option<i64>> {
        let connection = self.conn();
        let value = connection
            .query_row(
                "SELECT MAX(captured_at) FROM probe_sample WHERE target_id = ?1",
                params![target_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten();

        Ok(value)
    }

    /// Total samples held, for the settings screen's storage line.
    pub fn sample_count(&self) -> Result<u32> {
        let connection = self.conn();
        let count: i64 =
            connection.query_row("SELECT COUNT(*) FROM probe_sample", [], |row| row.get(0))?;
        Ok(count as u32)
    }

    /// Deletes everything. Used by the settings screen's clear action, and by
    /// nothing else.
    pub fn clear(&self) -> Result<()> {
        let connection = self.conn();
        connection.execute_batch("DELETE FROM probe_sample; DELETE FROM incident;")?;
        Ok(())
    }
}

/// The incident still open for a target, if any.
///
/// At most one is open per target by construction: a new one is only inserted
/// when none is already open, so the loop cannot create two overlapping outages.
fn open_incident(connection: &Connection, target_id: i64) -> Result<Option<Incident>> {
    let incident = connection
        .query_row(
            "SELECT id, target_id, started_at, probe_count, router_reachable
               FROM incident
              WHERE target_id = ?1 AND recovered_at IS NULL
              ORDER BY started_at DESC
              LIMIT 1",
            params![target_id],
            |row| {
                Ok(Incident {
                    id: row.get(0)?,
                    target_id: row.get(1)?,
                    started_at: row.get(2)?,
                    recovered_at: None,
                    probe_count: row.get::<_, i64>(3)? as u32,
                    router_reachable: row.get::<_, Option<i64>>(4)?.map(|value| value != 0),
                })
            },
        )
        .optional()?;

    Ok(incident)
}

/// The stored name of a failure kind. Written as text rather than an integer so
/// the database stays readable with any SQLite tool, and so adding a new kind
/// cannot silently change the meaning of an old row.
fn error_kind_name(kind: ProbeErrorKind) -> &'static str {
    match kind {
        ProbeErrorKind::Timeout => "timeout",
        ProbeErrorKind::Unreachable => "unreachable",
        ProbeErrorKind::Dns => "dns",
        ProbeErrorKind::Other => "other",
    }
}

/// Where the history database lives, under the app's own data directory.
pub fn default_path(data_dir: &Path) -> PathBuf {
    data_dir.join("history.sqlite3")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(target_id: i64, at: i64, success: bool) -> ProbeOutcome {
        ProbeOutcome {
            target_id,
            captured_at: at,
            success,
            latency_ms: success.then_some(12.5),
            error_kind: (!success).then_some(ProbeErrorKind::Timeout),
        }
    }

    fn store() -> Store {
        Store::in_memory().expect("an in-memory store must open")
    }

    #[test]
    fn a_successful_sample_is_stored_with_its_latency() {
        let store = store();

        store.record(&outcome(1, 1_000, true), None).expect("write");

        let samples = store.samples_since(1, 0).expect("read");
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].latency_ms, Some(12.5));
        assert_eq!(samples[0].captured_at, 1_000);
    }

    /// The rule the whole storage layer exists to protect. A failure stores
    /// NULL, so a later reader cannot mistake it for a measurement of zero even
    /// if the code that wrote it is long gone.
    #[test]
    fn a_failed_sample_stores_no_latency_at_all() {
        let store = store();

        store
            .record(&outcome(1, 1_000, false), None)
            .expect("write");

        let samples = store.samples_since(1, 0).expect("read");
        assert_eq!(samples.len(), 1);
        assert_eq!(
            samples[0].latency_ms, None,
            "a failure must never be stored as a zero latency"
        );
    }

    #[test]
    fn the_schema_survives_reopening_the_same_file() {
        let directory = std::env::temp_dir().join(format!("fibrex-test-{}", std::process::id()));
        let path = default_path(&directory);

        {
            let store = Store::open(&path).expect("open");
            store.record(&outcome(1, 5_000, true), None).expect("write");
        }

        // Reopening runs `migrate` a second time, which must be a no-op rather
        // than an error or a duplicate schema.
        {
            let store = Store::open(&path).expect("reopen");
            assert_eq!(store.sample_count().expect("count"), 1);
        }

        let _ = std::fs::remove_dir_all(&directory);
    }

    // ---------------------------------------------------------------------
    // Incident detection.
    // ---------------------------------------------------------------------

    #[test]
    fn two_consecutive_failures_open_an_incident() {
        let store = store();

        let first = store
            .record(&outcome(1, 1_000, false), None)
            .expect("write");
        assert!(first.is_some(), "the first failure opens a candidate");
        assert_eq!(
            store.incidents(10).expect("read").len(),
            0,
            "one failure is a blip and must not be listed as an outage"
        );

        let second = store
            .record(&outcome(1, 3_000, false), None)
            .expect("write")
            .expect("an incident");
        assert_eq!(second.started_at, 1_000);
        assert_eq!(
            second.probe_count, 2,
            "the incident reaches the threshold on the second failure"
        );

        let incidents = store.incidents(10).expect("read");
        assert_eq!(incidents.len(), 1);
        assert_eq!(
            incidents[0].started_at, 1_000,
            "it began at the first failure"
        );
        assert!(incidents[0].is_open());
    }

    #[test]
    fn a_success_after_an_incident_records_the_recovery() {
        let store = store();

        store
            .record(&outcome(1, 1_000, false), None)
            .expect("write");
        store
            .record(&outcome(1, 3_000, false), None)
            .expect("write");
        store.record(&outcome(1, 5_000, true), None).expect("write");

        let incidents = store.incidents(10).expect("read");
        assert_eq!(incidents.len(), 1);
        assert_eq!(incidents[0].recovered_at, Some(5_000));
        assert_eq!(
            incidents[0].duration_ms(9_999),
            4_000,
            "duration is measured to the recovery, not to now"
        );
    }

    /// A single failure followed by a success is not an outage. It must leave no
    /// trace, because otherwise a flaky moment would appear in the history as a
    /// recorded incident that never happened.
    #[test]
    fn a_lone_failure_that_recovers_leaves_no_incident() {
        let store = store();

        store.record(&outcome(1, 1_000, true), None).expect("write");
        store
            .record(&outcome(1, 3_000, false), None)
            .expect("write");
        store.record(&outcome(1, 5_000, true), None).expect("write");

        assert!(
            store.incidents(10).expect("read").is_empty(),
            "one failed probe is not an outage"
        );
    }

    #[test]
    fn a_second_outage_is_a_separate_incident() {
        let store = store();

        for success in [false, false, true, true, false, false, true] {
            let at = 1_000 + store.sample_count().expect("count") as i64 * 2_000;
            store.record(&outcome(1, at, success), None).expect("write");
        }

        let incidents = store.incidents(10).expect("read");
        assert_eq!(incidents.len(), 2, "two separate outages");
        assert!(
            incidents[0].started_at > incidents[1].started_at,
            "they come back newest first"
        );
        assert!(
            incidents
                .iter()
                .all(|incident| incident.recovered_at.is_some()),
            "both are closed"
        );
    }

    #[test]
    fn an_open_incident_reports_its_duration_against_now() {
        let store = store();

        store
            .record(&outcome(1, 1_000, false), None)
            .expect("write");
        store
            .record(&outcome(1, 3_000, false), None)
            .expect("write");

        let incidents = store.incidents(10).expect("read");
        assert!(incidents[0].is_open());
        assert_eq!(
            incidents[0].duration_ms(10_000),
            9_000,
            "an open outage is still running"
        );
    }

    /// The router's state is captured when the outage begins, not when it ends.
    /// Overwriting it at recovery would erase the one fact that distinguishes a
    /// local fault from an upstream one.
    #[test]
    fn the_router_state_is_recorded_when_the_outage_begins() {
        let store = store();

        store
            .record(&outcome(1, 1_000, false), Some(true))
            .expect("write");
        store
            .record(&outcome(1, 3_000, false), Some(true))
            .expect("write");

        // The router goes down too, later in the same outage. The stored value
        // must still be the one from the start.
        store
            .record(&outcome(1, 5_000, false), Some(false))
            .expect("write");
        store.record(&outcome(1, 7_000, true), None).expect("write");

        let incidents = store.incidents(10).expect("read");
        assert_eq!(
            incidents[0].router_reachable,
            Some(true),
            "the router was up when the outage started, and that is the fact worth keeping"
        );
    }

    #[test]
    fn an_incident_with_no_router_reading_says_so_rather_than_guessing() {
        let store = store();

        store
            .record(&outcome(1, 1_000, false), None)
            .expect("write");
        store
            .record(&outcome(1, 3_000, false), None)
            .expect("write");

        let incidents = store.incidents(10).expect("read");
        assert_eq!(
            incidents[0].router_reachable, None,
            "no router reading is not the same as an unreachable router"
        );
    }

    #[test]
    fn incidents_are_tracked_per_target() {
        let store = store();

        store
            .record(&outcome(1, 1_000, false), None)
            .expect("write");
        store
            .record(&outcome(1, 3_000, false), None)
            .expect("write");
        store
            .record(&outcome(2, 2_000, false), None)
            .expect("write");
        store
            .record(&outcome(2, 4_000, false), None)
            .expect("write");

        let incidents = store.incidents(10).expect("read");
        assert_eq!(incidents.len(), 2);
        assert!(
            incidents.iter().any(|incident| incident.target_id == 1)
                && incidents.iter().any(|incident| incident.target_id == 2),
            "an outage on one target is not an outage on another"
        );
    }

    // ---------------------------------------------------------------------
    // Retention and counting.
    // ---------------------------------------------------------------------

    #[test]
    fn samples_older_than_the_retention_window_are_dropped() {
        let store = store();
        let now = 1_000_000_000_000;

        store
            .record(&outcome(1, now - RETENTION_MS - 5_000, true), None)
            .expect("old");
        store.record(&outcome(1, now, true), None).expect("new");

        let samples = store.samples_since(1, 0).expect("read");
        assert_eq!(samples.len(), 1, "the aged-out sample is gone");
        assert_eq!(samples[0].captured_at, now);
    }

    #[test]
    fn counts_report_attempts_and_failures_separately() {
        let store = store();
        let base = 1_000_000;

        for (index, success) in [true, false, true, false, false].iter().enumerate() {
            store
                .record(&outcome(1, base + index as i64 * 1_000, *success), None)
                .expect("write");
        }

        let (total, failed) = store.counts_since(1, 0).expect("counts");
        assert_eq!(total, 5);
        assert_eq!(
            failed, 3,
            "true, false, true, false, false is three failures"
        );
    }

    #[test]
    fn counts_are_zero_on_an_empty_store_rather_than_missing() {
        let store = store();

        assert_eq!(store.counts_since(1, 0).expect("counts"), (0, 0));
        assert_eq!(store.sample_count().expect("count"), 0);
        assert_eq!(store.earliest_sample_at(1).expect("earliest"), None);
    }

    #[test]
    fn the_earliest_sample_time_is_reported_for_a_range_label() {
        let store = store();

        store.record(&outcome(1, 7_000, true), None).expect("write");
        store.record(&outcome(1, 3_000, true), None).expect("write");

        assert_eq!(store.earliest_sample_at(1).expect("earliest"), Some(3_000));
    }

    #[test]
    fn clearing_removes_samples_and_incidents_together() {
        let store = store();

        store
            .record(&outcome(1, 1_000, false), None)
            .expect("write");
        store
            .record(&outcome(1, 3_000, false), None)
            .expect("write");

        store.clear().expect("clear");

        assert_eq!(store.sample_count().expect("count"), 0);
        assert!(store.incidents(10).expect("read").is_empty());
    }

    /// Two consecutive failures on different targets are not one outage. Without
    /// the per-target open lookup this would merge them.
    #[test]
    fn a_failure_on_another_target_does_not_extend_this_incident() {
        let store = store();

        store
            .record(&outcome(1, 1_000, false), None)
            .expect("write");
        store
            .record(&outcome(2, 2_000, false), None)
            .expect("write");
        store
            .record(&outcome(1, 3_000, false), None)
            .expect("write");

        let incidents = store.incidents(10).expect("read");
        assert_eq!(incidents.len(), 1, "only the first target reached two");
        assert_eq!(incidents[0].target_id, 1);
        assert_eq!(incidents[0].probe_count, 2);
    }
}

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
pub const SCHEMA_VERSION: i64 = 2;

/// How many consecutive failed probes make an incident.
///
/// One is a blip, not an outage. Two consecutive failures at the shipped
/// two-second interval is roughly a four second gap, which is already longer
/// than a healthy connection ever stalls.
pub const INCIDENT_THRESHOLD: u32 = 2;

/// The longest silence between two probes that still counts as observation.
///
/// Five missed probes at the shipped two-second interval. Anything wider — a
/// pause, a sleep, a quit, a probe that hung — means the app did not see what
/// happened in between, and an incident must not claim that time as downtime it
/// measured.
pub const UNOBSERVED_GAP_MS: i64 = 10_000;

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

/// Why an incident stopped being open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosedBy {
    /// A successful probe closed it. The recovery was observed.
    Probe,
    /// Observation stopped — a pause, a quit, a sleep, a missed tick — so the
    /// incident was closed where the evidence ends. The line may well have
    /// stayed down; the app simply was not watching.
    Unobserved,
}

impl ClosedBy {
    pub fn label(self) -> &'static str {
        match self {
            ClosedBy::Probe => "probe",
            ClosedBy::Unobserved => "unobserved",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "probe" => Some(ClosedBy::Probe),
            "unobserved" => Some(ClosedBy::Unobserved),
            _ => None,
        }
    }
}

/// A stretch of consecutive failed probes, with its recovery.
#[derive(Debug, Clone, PartialEq)]
pub struct Incident {
    pub id: i64,
    pub target_id: i64,
    pub started_at: i64,
    /// When the incident stopped being open: the successful probe, or the last
    /// probe that was observed when observation stopped. `None` while open.
    pub recovered_at: Option<i64>,
    /// The last probe folded into this incident, which is how far the evidence
    /// actually reaches. Distinct from `recovered_at`: an incident closed
    /// unobserved ends at the last failed probe, not at the next sample.
    pub observed_until: i64,
    /// Failed probes counted so far.
    pub probe_count: u32,
    /// Whether the router answered during the outage, when it was probed at all.
    /// `None` means no router reading covers this window, which is not the same
    /// as the router being down.
    pub router_reachable: Option<bool>,
    /// How the incident closed, or `None` while it is open.
    pub closed_by: Option<ClosedBy>,
}

impl Incident {
    /// How long the outage lasted, or has lasted so far.
    ///
    /// For an incident closed unobserved this is the span that was actually
    /// watched, which is a lower bound on the outage rather than its length.
    pub fn duration_ms(&self, now: i64) -> i64 {
        (self.recovered_at.unwrap_or(now)) - self.started_at
    }

    pub fn is_open(&self) -> bool {
        self.recovered_at.is_none()
    }

    /// Whether the recovery was seen, rather than the incident being closed
    /// because probing stopped.
    pub fn recovery_observed(&self) -> bool {
        self.closed_by != Some(ClosedBy::Unobserved)
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

        // v2 records how far each incident was actually watched, and why it
        // closed. Without it a pause, a quit or a sleep between the first
        // failure and the next success was stored as confirmed downtime, which
        // is the one thing the incident log must never do.
        if version < 2 {
            connection.execute_batch(
                "
                ALTER TABLE incident ADD COLUMN observed_until INTEGER;
                ALTER TABLE incident ADD COLUMN closed_by TEXT;
                ",
            )?;

            // Existing rows: the last observation was the recovery when there
            // is one, and otherwise the first (and possibly only) failure.
            connection.execute(
                "UPDATE incident
                    SET observed_until = COALESCE(recovered_at, started_at)
                  WHERE observed_until IS NULL",
                [],
            )?;
            // Every recovered row written before v2 was closed by a successful
            // probe, because nothing else could close one.
            connection.execute(
                "UPDATE incident SET closed_by = 'probe' WHERE recovered_at IS NOT NULL",
                [],
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

        let mut open = open_incident(&transaction, outcome.target_id)?;

        // A silence wider than a plausible probe cadence means the app was not
        // watching. The incident ends where the last probe was seen; this probe
        // starts a fresh observation. Without this, pausing during an outage
        // and resuming ten minutes later would record ten minutes of downtime
        // that was never measured.
        if let Some(incident) = open.as_ref()
            && outcome.captured_at.saturating_sub(incident.observed_until) > UNOBSERVED_GAP_MS
        {
            close_unobserved(&transaction, incident)?;
            open = None;
        }

        let incident = if outcome.success {
            // A success closes an open incident. If none is open this is the
            // ordinary case and nothing is written.
            match open {
                Some(incident) if incident.probe_count >= INCIDENT_THRESHOLD => {
                    transaction.execute(
                        "UPDATE incident
                            SET recovered_at = ?1, observed_until = ?1, closed_by = 'probe'
                          WHERE id = ?2",
                        params![outcome.captured_at, incident.id],
                    )?;
                    Some(Incident {
                        recovered_at: Some(outcome.captured_at),
                        observed_until: outcome.captured_at,
                        closed_by: Some(ClosedBy::Probe),
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
                        "UPDATE incident
                            SET probe_count = probe_count + 1, observed_until = ?1
                          WHERE id = ?2",
                        params![outcome.captured_at, incident.id],
                    )?;
                    Some(Incident {
                        probe_count: incident.probe_count + 1,
                        observed_until: outcome.captured_at,
                        ..incident
                    })
                }
                None => {
                    transaction.execute(
                        "INSERT INTO incident
                            (target_id, started_at, recovered_at, observed_until,
                             probe_count, router_reachable, closed_by)
                         VALUES (?1, ?2, NULL, ?2, 1, ?3, NULL)",
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
                        observed_until: outcome.captured_at,
                        probe_count: 1,
                        router_reachable,
                        closed_by: None,
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

    /// Closes every open incident where its evidence ends.
    ///
    /// Called when monitoring is paused: the user asked the app to stop
    /// watching, so an outage must not stay open and keep counting. Partial
    /// runs below the threshold are discarded exactly as a success would
    /// discard them — they were never incidents.
    pub fn close_open_incidents(&self) -> Result<Vec<Incident>> {
        let mut connection = self.conn();
        let transaction = connection.transaction()?;

        let open = {
            let mut statement = transaction.prepare(
                "SELECT id, target_id, started_at, recovered_at, probe_count,
                        router_reachable, COALESCE(observed_until, started_at), closed_by
                   FROM incident
                  WHERE recovered_at IS NULL
                  ORDER BY started_at ASC",
            )?;
            let rows = statement.query_map([], incident_from_row)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        let mut closed = Vec::new();
        for incident in open {
            if let Some(incident) = close_unobserved(&transaction, &incident)? {
                closed.push(incident);
            }
        }

        transaction.commit()?;
        Ok(closed)
    }

    /// Every incident that reached the threshold, newest first.
    ///
    /// Partial runs below the threshold are excluded, because a single failed
    /// probe is not an outage and listing it would make the app look like it
    /// records problems that did not happen.
    pub fn incidents(&self, limit: u32) -> Result<Vec<Incident>> {
        let connection = self.conn();
        let mut statement = connection.prepare(
            "SELECT id, target_id, started_at, recovered_at, probe_count,
                    router_reachable, COALESCE(observed_until, recovered_at, started_at),
                    closed_by
               FROM incident
              WHERE probe_count >= ?1
              ORDER BY started_at DESC
              LIMIT ?2",
        )?;

        let rows = statement.query_map(params![INCIDENT_THRESHOLD, limit], incident_from_row)?;

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

/// One incident row, in the column order every read selects.
fn incident_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Incident> {
    Ok(Incident {
        id: row.get(0)?,
        target_id: row.get(1)?,
        started_at: row.get(2)?,
        recovered_at: row.get(3)?,
        probe_count: row.get::<_, i64>(4)? as u32,
        router_reachable: row.get::<_, Option<i64>>(5)?.map(|value| value != 0),
        observed_until: row.get(6)?,
        closed_by: row
            .get::<_, Option<String>>(7)?
            .and_then(|value| ClosedBy::parse(&value)),
    })
}

/// The incident still open for a target, if any.
///
/// At most one is open per target by construction: a new one is only inserted
/// when none is already open, so the loop cannot create two overlapping outages.
fn open_incident(connection: &Connection, target_id: i64) -> Result<Option<Incident>> {
    let incident = connection
        .query_row(
            "SELECT id, target_id, started_at, recovered_at, probe_count,
                    router_reachable, COALESCE(observed_until, started_at), closed_by
               FROM incident
              WHERE target_id = ?1 AND recovered_at IS NULL
              ORDER BY started_at DESC
              LIMIT 1",
            params![target_id],
            incident_from_row,
        )
        .optional()?;

    Ok(incident)
}

/// Ends one incident where its evidence ends.
///
/// `recovered_at` is the last probe that was observed, not the moment the app
/// noticed the gap, so the recorded duration is the part that was actually
/// watched. Returns the closed incident, or `None` when it was still below the
/// threshold and therefore never an incident at all.
fn close_unobserved(connection: &Connection, incident: &Incident) -> Result<Option<Incident>> {
    if incident.probe_count < INCIDENT_THRESHOLD {
        connection.execute("DELETE FROM incident WHERE id = ?1", params![incident.id])?;
        return Ok(None);
    }

    connection.execute(
        "UPDATE incident
            SET recovered_at = COALESCE(observed_until, ?2), closed_by = 'unobserved'
          WHERE id = ?1",
        params![incident.id, incident.observed_until],
    )?;

    Ok(Some(Incident {
        recovered_at: Some(incident.observed_until),
        closed_by: Some(ClosedBy::Unobserved),
        ..incident.clone()
    }))
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
        assert_eq!(incidents[0].closed_by, Some(ClosedBy::Probe));
        assert!(incidents[0].recovery_observed());
        assert_eq!(
            incidents[0].duration_ms(9_999),
            4_000,
            "duration is measured to the recovery, not to now"
        );
    }

    /// The pause case: an outage is open, probing stops, the line comes back,
    /// and the next probe arrives ten minutes later. That probe must not close
    /// the incident as if the whole ten minutes had been watched.
    #[test]
    fn a_gap_between_failure_and_success_is_not_counted_as_downtime() {
        let store = store();

        store
            .record(&outcome(1, 1_000, false), None)
            .expect("write");
        store
            .record(&outcome(1, 3_000, false), None)
            .expect("write");
        store
            .record(&outcome(1, 603_000, true), None)
            .expect("write");

        let incidents = store.incidents(10).expect("read");
        assert_eq!(incidents.len(), 1, "a gap must not invent a second outage");
        assert_eq!(incidents[0].probe_count, 2);
        assert_eq!(
            incidents[0].recovered_at,
            Some(3_000),
            "the incident ends at the last probe that was actually observed"
        );
        assert_eq!(incidents[0].closed_by, Some(ClosedBy::Unobserved));
        assert_eq!(incidents[0].duration_ms(603_000), 2_000);
        assert!(!incidents[0].recovery_observed());
    }

    /// A gap followed by another failure is a new observation window, not a
    /// continuation of the old one.
    #[test]
    fn a_gap_then_a_failure_starts_a_new_incident() {
        let store = store();

        store
            .record(&outcome(1, 1_000, false), None)
            .expect("write");
        store
            .record(&outcome(1, 3_000, false), None)
            .expect("write");
        store
            .record(&outcome(1, 603_000, false), None)
            .expect("write");
        store
            .record(&outcome(1, 605_000, false), None)
            .expect("write");

        let incidents = store.incidents(10).expect("read");
        assert_eq!(incidents.len(), 2);
        assert_eq!(
            incidents[0].started_at, 603_000,
            "the newest incident begins after the gap"
        );
        assert_eq!(incidents[0].probe_count, 2);
        assert!(incidents[0].is_open());
        assert_eq!(incidents[1].closed_by, Some(ClosedBy::Unobserved));
        assert_eq!(incidents[1].recovered_at, Some(3_000));
    }

    #[test]
    fn pausing_closes_an_open_incident_where_it_was_last_seen() {
        let store = store();

        store
            .record(&outcome(1, 1_000, false), None)
            .expect("write");
        store
            .record(&outcome(1, 3_000, false), None)
            .expect("write");

        let closed = store.close_open_incidents().expect("close");
        assert_eq!(closed.len(), 1);
        assert_eq!(closed[0].recovered_at, Some(3_000));
        assert_eq!(closed[0].closed_by, Some(ClosedBy::Unobserved));
        assert!(!closed[0].recovery_observed());

        let incidents = store.incidents(10).expect("read");
        assert_eq!(incidents.len(), 1);
        assert!(incidents[0].recovered_at.is_some());
        assert!(
            store.close_open_incidents().expect("close again").is_empty(),
            "closing twice must not rewrite a closed incident"
        );
    }

    #[test]
    fn pausing_discards_a_candidate_that_never_became_an_incident() {
        let store = store();

        store
            .record(&outcome(1, 1_000, false), None)
            .expect("write");

        assert!(store.close_open_incidents().expect("close").is_empty());
        assert!(
            store.incidents(10).expect("read").is_empty(),
            "one failure is a blip, and a pause must not promote it"
        );
    }

    /// A database written by v0.1.1 has incidents but no `observed_until` or
    /// `closed_by`. Opening it must add both and keep every row.
    #[test]
    fn a_version_one_database_migrates_its_incidents() {
        let directory =
            std::env::temp_dir().join(format!("fibrex-migrate-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("create");
        let path = default_path(&directory);

        {
            let connection = Connection::open(&path).expect("open");
            connection
                .execute_batch(
                    "
                    CREATE TABLE probe_sample (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        target_id INTEGER NOT NULL,
                        captured_at INTEGER NOT NULL,
                        success INTEGER NOT NULL,
                        latency_ms REAL,
                        error_kind TEXT
                    );
                    CREATE TABLE incident (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        target_id INTEGER NOT NULL,
                        started_at INTEGER NOT NULL,
                        recovered_at INTEGER,
                        probe_count INTEGER NOT NULL,
                        router_reachable INTEGER
                    );
                    INSERT INTO incident
                        (target_id, started_at, recovered_at, probe_count, router_reachable)
                    VALUES (1, 1000, 5000, 3, 1);
                    INSERT INTO incident
                        (target_id, started_at, recovered_at, probe_count, router_reachable)
                    VALUES (1, 7000, NULL, 2, NULL);
                    PRAGMA user_version = 1;
                    ",
                )
                .expect("v1 schema");
        }

        let store = Store::open(&path).expect("migrate");
        let incidents = store.incidents(10).expect("read");
        assert_eq!(incidents.len(), 2, "migration must not drop incidents");

        let recovered = incidents
            .iter()
            .find(|incident| incident.recovered_at == Some(5_000))
            .expect("the recovered incident");
        assert_eq!(recovered.closed_by, Some(ClosedBy::Probe));
        assert_eq!(recovered.observed_until, 5_000);

        let open = incidents
            .iter()
            .find(|incident| incident.recovered_at.is_none())
            .expect("the open incident");
        assert_eq!(open.observed_until, 7_000);
        assert_eq!(open.closed_by, None);

        let _ = std::fs::remove_dir_all(&directory);
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

//! Durable provider state in SQLite. Deduplication records, subjects and counters
//! commit in one transaction per command (CORE section 10 step 8).

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

pub struct Store {
    connection: Connection,
    /// Mutant `lose-dedupe-on-restart`: records live only in this map.
    volatile_commands: Option<HashMap<(String, String), StoredCommand>>,
}

#[derive(Clone)]
pub struct StoredCommand {
    pub digest: String,
    pub response: String,
}

pub struct Window {
    pub oldest: i64,
    pub current: i64,
}

impl Store {
    pub fn open(data_dir: &Path, volatile_commands: bool) -> rusqlite::Result<Self> {
        let connection = Connection::open(data_dir.join("reference-provider.sqlite3"))?;
        connection.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = FULL;
             CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS commands (
               scope TEXT NOT NULL, command_id TEXT NOT NULL, generation INTEGER NOT NULL,
               digest TEXT NOT NULL, response TEXT NOT NULL, PRIMARY KEY (scope, command_id));
             CREATE TABLE IF NOT EXISTS subjects (
               kind TEXT NOT NULL, id TEXT NOT NULL, revision INTEGER NOT NULL,
               value TEXT NOT NULL, applied_count INTEGER NOT NULL, PRIMARY KEY (kind, id));
             CREATE TABLE IF NOT EXISTS grants (
               id TEXT PRIMARY KEY, revision INTEGER NOT NULL, record TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS events (
               epoch INTEGER NOT NULL, sequence INTEGER NOT NULL, record TEXT NOT NULL,
               PRIMARY KEY (epoch, sequence));
             CREATE TABLE IF NOT EXISTS epoch_changes (
               to_epoch INTEGER PRIMARY KEY, from_epoch INTEGER NOT NULL, vouched_through INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS meta_text (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT OR IGNORE INTO meta VALUES ('stream_epoch', 1), ('discarded_epoch', 0), ('discarded_sequence', 0), ('capability_revision', 0);
             INSERT OR IGNORE INTO meta VALUES ('dedupe_oldest', 1), ('dedupe_current', 1), ('operation_seq', 0);",
        )?;
        let seed = format!(
            "{}:{:?}",
            data_dir.display(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
        );
        let stream_id = format!(
            "stream-{}",
            &crate::json::sha256_digest(seed.as_bytes())[7..23]
        );
        connection.execute(
            "INSERT OR IGNORE INTO meta_text VALUES ('stream_id', ?1)",
            [stream_id],
        )?;
        Ok(Self {
            connection,
            volatile_commands: volatile_commands.then(HashMap::new),
        })
    }

    fn meta(&self, key: &str) -> rusqlite::Result<i64> {
        self.connection
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
                row.get(0)
            })
    }

    pub fn window(&self) -> rusqlite::Result<Window> {
        Ok(Window {
            oldest: self.meta("dedupe_oldest")?,
            current: self.meta("dedupe_current")?,
        })
    }

    /// Launch-configuration retention policy: advance generations and discard old records.
    pub fn apply_retention(&mut self, advance: i64, retain: i64) -> rusqlite::Result<()> {
        let tx = self.connection.transaction()?;
        let current: i64 = tx.query_row(
            "SELECT value FROM meta WHERE key='dedupe_current'",
            [],
            |r| r.get(0),
        )?;
        let oldest: i64 = tx.query_row(
            "SELECT value FROM meta WHERE key='dedupe_oldest'",
            [],
            |r| r.get(0),
        )?;
        let current = current + advance;
        let oldest = oldest.max(current - retain + 1);
        tx.execute(
            "UPDATE meta SET value=?1 WHERE key='dedupe_current'",
            [current],
        )?;
        tx.execute(
            "UPDATE meta SET value=?1 WHERE key='dedupe_oldest'",
            [oldest],
        )?;
        tx.execute("DELETE FROM commands WHERE generation < ?1", [oldest])?;
        tx.commit()
    }

    pub fn find_command(
        &self,
        scope: &str,
        command_id: &str,
    ) -> rusqlite::Result<Option<StoredCommand>> {
        if let Some(map) = &self.volatile_commands {
            return Ok(map
                .get(&(scope.to_string(), command_id.to_string()))
                .cloned());
        }
        self.connection
            .query_row(
                "SELECT digest, response FROM commands WHERE scope=?1 AND command_id=?2",
                params![scope, command_id],
                |row| {
                    Ok(StoredCommand {
                        digest: row.get(0)?,
                        response: row.get(1)?,
                    })
                },
            )
            .optional()
    }

    pub fn subject(&self, kind: &str, id: &str) -> rusqlite::Result<Option<(i64, String, i64)>> {
        self.connection
            .query_row(
                "SELECT revision, value, applied_count FROM subjects WHERE kind=?1 AND id=?2",
                params![kind, id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
    }

    pub fn revision(&self, kind: &str, id: &str) -> rusqlite::Result<i64> {
        if kind == "core.grant" {
            return Ok(self.grant(id)?.map_or(0, |(revision, _)| revision));
        }
        Ok(self
            .subject(kind, id)?
            .map_or(0, |(revision, _, _)| revision))
    }

    /// Record a rejected command identity (only for mutant `bind-on-rejection`).
    pub fn bind_rejection(
        &mut self,
        scope: &str,
        command_id: &str,
        generation: i64,
        digest: &str,
        response: &str,
    ) -> rusqlite::Result<()> {
        self.insert_command(scope, command_id, generation, digest, response)
    }

    fn insert_command(
        &mut self,
        scope: &str,
        command_id: &str,
        generation: i64,
        digest: &str,
        response: &str,
    ) -> rusqlite::Result<()> {
        if let Some(map) = &mut self.volatile_commands {
            map.insert(
                (scope.into(), command_id.into()),
                StoredCommand {
                    digest: digest.into(),
                    response: response.into(),
                },
            );
            return Ok(());
        }
        self.connection.execute(
            "INSERT OR REPLACE INTO commands VALUES (?1, ?2, ?3, ?4, ?5)",
            params![scope, command_id, generation, digest, response],
        )?;
        Ok(())
    }

    /// Apply one change and bind the command identity atomically. `apply` runs inside the
    /// owner transaction with the new operation sequence number and returns the stored response.
    pub fn commit_with(
        &mut self,
        scope: &str,
        command_id: &str,
        generation: i64,
        digest: &str,
        apply: impl FnOnce(&rusqlite::Transaction, i64) -> rusqlite::Result<String>,
    ) -> rusqlite::Result<String> {
        let volatile = self.volatile_commands.is_some();
        let tx = self.connection.transaction()?;
        let sequence: i64 = tx.query_row(
            "SELECT value + 1 FROM meta WHERE key='operation_seq'",
            [],
            |r| r.get(0),
        )?;
        tx.execute(
            "UPDATE meta SET value=?1 WHERE key='operation_seq'",
            [sequence],
        )?;
        let response = apply(&tx, sequence)?;
        if !volatile {
            tx.execute(
                "INSERT OR REPLACE INTO commands VALUES (?1, ?2, ?3, ?4, ?5)",
                params![scope, command_id, generation, digest, response],
            )?;
        }
        tx.commit()?;
        if volatile {
            self.insert_command(scope, command_id, generation, digest, &response)?;
        }
        Ok(response)
    }

    pub fn grant(&self, id: &str) -> rusqlite::Result<Option<(i64, serde_json::Value)>> {
        let row: Option<(i64, String)> = self
            .connection
            .query_row(
                "SELECT revision, record FROM grants WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(row.map(|(revision, record)| {
            (
                revision,
                serde_json::from_str(&record).unwrap_or(serde_json::Value::Null),
            )
        }))
    }

    pub fn all_grants(&self) -> rusqlite::Result<Vec<(i64, serde_json::Value)>> {
        let mut statement = self
            .connection
            .prepare("SELECT revision, record FROM grants ORDER BY id")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.map(|row| {
            row.map(|(revision, record)| {
                (
                    revision,
                    serde_json::from_str(&record).unwrap_or(serde_json::Value::Null),
                )
            })
        })
        .collect()
    }

    pub fn stream_id(&self) -> rusqlite::Result<String> {
        self.connection.query_row(
            "SELECT value FROM meta_text WHERE key='stream_id'",
            [],
            |r| r.get(0),
        )
    }

    pub fn stream_epoch(&self) -> rusqlite::Result<i64> {
        self.meta("stream_epoch")
    }

    pub fn discarded_through(&self) -> rusqlite::Result<(i64, i64)> {
        Ok((
            self.meta("discarded_epoch")?,
            self.meta("discarded_sequence")?,
        ))
    }

    pub fn max_sequence(&self, epoch: i64) -> rusqlite::Result<i64> {
        self.connection.query_row(
            "SELECT COALESCE(MAX(sequence), 0) FROM events WHERE epoch=?1",
            [epoch],
            |r| r.get(0),
        )
    }

    /// Highest sequence of `epoch` ever assigned, including discarded events.
    pub fn assigned_through(&self, epoch: i64) -> rusqlite::Result<i64> {
        let (discarded_epoch, discarded_sequence) = self.discarded_through()?;
        let stored = self.max_sequence(epoch)?;
        Ok(if discarded_epoch == epoch {
            stored.max(discarded_sequence)
        } else {
            stored
        })
    }

    pub fn vouched_through(&self, epoch: i64) -> rusqlite::Result<Option<i64>> {
        self.connection
            .query_row(
                "SELECT vouched_through FROM epoch_changes WHERE from_epoch=?1",
                [epoch],
                |r| r.get(0),
            )
            .optional()
    }

    pub fn next_event(
        &self,
        epoch: i64,
        after: i64,
    ) -> rusqlite::Result<Option<(i64, serde_json::Value)>> {
        let row: Option<(i64, String)> = self
            .connection
            .query_row(
                "SELECT sequence, record FROM events WHERE epoch=?1 AND sequence>?2 ORDER BY sequence LIMIT 1",
                params![epoch, after],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(row.map(|(sequence, record)| {
            (
                sequence,
                serde_json::from_str(&record).unwrap_or(serde_json::Value::Null),
            )
        }))
    }

    pub fn all_subjects(&self) -> rusqlite::Result<Vec<(String, String, i64, String)>> {
        let mut statement = self
            .connection
            .prepare("SELECT kind, id, revision, value FROM subjects ORDER BY kind, id")?;
        let rows = statement.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        rows.collect()
    }

    /// Launch-configuration stream controls: start a new epoch, discard old events.
    pub fn apply_event_config(
        &mut self,
        new_epoch: bool,
        retain_last: Option<i64>,
        volatile: bool,
    ) -> rusqlite::Result<()> {
        let tx = self.connection.transaction()?;
        if volatile {
            tx.execute("DELETE FROM events", [])?;
        }
        if new_epoch {
            let epoch: i64 =
                tx.query_row("SELECT value FROM meta WHERE key='stream_epoch'", [], |r| {
                    r.get(0)
                })?;
            let vouched: i64 = tx.query_row(
                "SELECT COALESCE(MAX(sequence), 0) FROM events WHERE epoch=?1",
                [epoch],
                |r| r.get(0),
            )?;
            tx.execute(
                "INSERT INTO epoch_changes VALUES (?1, ?2, ?3)",
                params![epoch + 1, epoch, vouched],
            )?;
            tx.execute(
                "UPDATE meta SET value=?1 WHERE key='stream_epoch'",
                [epoch + 1],
            )?;
        }
        if let Some(keep) = retain_last {
            let doomed: Vec<(i64, i64)> = {
                let mut statement = tx.prepare(
                    "SELECT epoch, sequence FROM events ORDER BY epoch DESC, sequence DESC LIMIT -1 OFFSET ?1",
                )?;
                let rows = statement.query_map([keep], |r| Ok((r.get(0)?, r.get(1)?)))?;
                rows.collect::<rusqlite::Result<_>>()?
            };
            if let Some(&(epoch, sequence)) = doomed.first() {
                tx.execute(
                    "DELETE FROM events WHERE epoch<?1 OR (epoch=?1 AND sequence<=?2)",
                    params![epoch, sequence],
                )?;
                tx.execute(
                    "UPDATE meta SET value=?1 WHERE key='discarded_epoch'",
                    [epoch],
                )?;
                tx.execute(
                    "UPDATE meta SET value=?1 WHERE key='discarded_sequence'",
                    [sequence],
                )?;
            }
        }
        tx.commit()
    }

    /// Append an event outside a command (only for mutant `replay-appends-event`).
    pub fn append_standalone_event(
        &mut self,
        record: serde_json::Value,
        gap: bool,
    ) -> rusqlite::Result<()> {
        let tx = self.connection.transaction()?;
        append_event(&tx, record, gap)?;
        tx.commit()
    }

    pub fn capability_revision(&self) -> rusqlite::Result<i64> {
        self.meta("capability_revision")
    }

    /// Record the current capability predicates; a change raises the revision and appends a
    /// provider-origin event (CORE section 17.3). Returns the revision.
    pub fn apply_capabilities(
        &mut self,
        provider_id: &str,
        predicates: &serde_json::Value,
        recorded_at: &str,
        static_revision: bool,
    ) -> rusqlite::Result<i64> {
        let digest = crate::json::sha256_digest(predicates.to_string().as_bytes());
        let tx = self.connection.transaction()?;
        let stored: Option<String> = tx
            .query_row(
                "SELECT value FROM meta_text WHERE key='capability_digest'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        let mut revision: i64 = tx.query_row(
            "SELECT value FROM meta WHERE key='capability_revision'",
            [],
            |r| r.get(0),
        )?;
        if stored.as_deref() != Some(digest.as_str()) {
            tx.execute(
                "INSERT OR REPLACE INTO meta_text VALUES ('capability_digest', ?1)",
                [&digest],
            )?;
            if revision == 0 {
                // The initial snapshot of a new store is not a change; it appends no event.
                revision = 1;
                tx.execute(
                    "UPDATE meta SET value=?1 WHERE key='capability_revision'",
                    [revision],
                )?;
            } else if !static_revision {
                revision += 1;
                tx.execute(
                    "UPDATE meta SET value=?1 WHERE key='capability_revision'",
                    [revision],
                )?;
                let stream: String = tx.query_row(
                    "SELECT value FROM meta_text WHERE key='stream_id'",
                    [],
                    |r| r.get(0),
                )?;
                let record = serde_json::json!({
                    "stream": stream,
                    "origin": "provider",
                    "type": "core.capabilities.changed",
                    "subject": {"kind": "core.capabilities", "id": provider_id},
                    "revision": revision,
                    "caused_by": [],
                    "recorded_at": recorded_at,
                    "payload": {"predicates": predicates},
                });
                append_event(&tx, record, false)?;
            }
        }
        tx.commit()?;
        Ok(revision)
    }
}

/// Append one event at the next position of the current epoch; returns its position.
pub fn append_event(
    tx: &rusqlite::Transaction,
    mut record: serde_json::Value,
    gap: bool,
) -> rusqlite::Result<(i64, i64)> {
    let epoch: i64 = tx.query_row("SELECT value FROM meta WHERE key='stream_epoch'", [], |r| {
        r.get(0)
    })?;
    let stored: i64 = tx.query_row(
        "SELECT COALESCE(MAX(sequence), 0) FROM events WHERE epoch=?1",
        [epoch],
        |r| r.get(0),
    )?;
    let (discarded_epoch, discarded_sequence): (i64, i64) = (
        tx.query_row(
            "SELECT value FROM meta WHERE key='discarded_epoch'",
            [],
            |r| r.get(0),
        )?,
        tx.query_row(
            "SELECT value FROM meta WHERE key='discarded_sequence'",
            [],
            |r| r.get(0),
        )?,
    );
    let last = if discarded_epoch == epoch {
        stored.max(discarded_sequence)
    } else {
        stored
    };
    let sequence = last + if gap { 2 } else { 1 };
    record["epoch"] = serde_json::json!(epoch);
    record["sequence"] = serde_json::json!(sequence);
    tx.execute(
        "INSERT INTO events VALUES (?1, ?2, ?3)",
        params![epoch, sequence, record.to_string()],
    )?;
    Ok((epoch, sequence))
}

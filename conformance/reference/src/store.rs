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
             INSERT OR IGNORE INTO meta VALUES ('dedupe_oldest', 1), ('dedupe_current', 1), ('operation_seq', 0);",
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

    /// Apply one change and bind the command identity atomically. `build_response`
    /// receives the new operation sequence number and subject revision.
    #[allow(clippy::too_many_arguments)]
    pub fn commit(
        &mut self,
        scope: &str,
        command_id: &str,
        generation: i64,
        digest: &str,
        kind: &str,
        id: &str,
        value: &str,
        build_response: impl FnOnce(i64, i64) -> String,
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
        let revision: i64 = tx
            .query_row(
                "SELECT revision FROM subjects WHERE kind=?1 AND id=?2",
                params![kind, id],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0)
            + 1;
        tx.execute(
            "INSERT INTO subjects VALUES (?1, ?2, ?3, ?4, 1)
             ON CONFLICT (kind, id) DO UPDATE SET revision=excluded.revision, value=excluded.value,
             applied_count=applied_count+1",
            params![kind, id, revision, value],
        )?;
        let response = build_response(sequence, revision);
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
}

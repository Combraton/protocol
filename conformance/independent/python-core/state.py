"""Durable provider state in SQLite (standard library ``sqlite3``).

One database file in the data directory holds:

- ``meta``: the deduplication window (``gen_oldest``, ``gen_current``) and the
  ``operation_ref`` sequence;
- ``subjects``: every provider-owned subject with its revision, value and how
  many commands changed it (the authority subject's revision is the epoch);
- ``commands``: bound command records keyed by (deduplication scope,
  command_id) with the generation they were issued under, the digest as sent,
  and the canonical acknowledgment and outcome bytes returned on replay.

All writes of one command happen inside one ``BEGIN IMMEDIATE`` transaction
(CORE 10 step 8).
"""

from __future__ import annotations

import os
import sqlite3

SCHEMA = """
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS subjects (
    kind          TEXT NOT NULL,
    id            TEXT NOT NULL,
    revision      INTEGER NOT NULL,
    value         TEXT,
    applied_count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (kind, id)
);
CREATE TABLE IF NOT EXISTS commands (
    scope       TEXT NOT NULL,
    command_id  TEXT NOT NULL,
    generation  INTEGER NOT NULL,
    digest      TEXT NOT NULL,
    ack         BLOB NOT NULL,
    outcome     BLOB NOT NULL,
    PRIMARY KEY (scope, command_id)
);
CREATE INDEX IF NOT EXISTS commands_by_generation ON commands (generation);
"""


class Store:
    def __init__(self, data_dir: str):
        os.makedirs(data_dir, exist_ok=True)
        path = os.path.join(data_dir, "independent-python-core.sqlite3")
        # isolation_level=None: we issue BEGIN/COMMIT ourselves.
        self.db = sqlite3.connect(path, isolation_level=None)
        self.db.execute("PRAGMA journal_mode=WAL")
        self.db.execute("PRAGMA synchronous=FULL")
        self.db.executescript(SCHEMA)
        self.db.execute("BEGIN IMMEDIATE")
        for key in ("gen_oldest", "gen_current", "op_seq"):
            self.db.execute("INSERT OR IGNORE INTO meta (key, value) VALUES (?, 0)", (key,))
        self.db.execute("COMMIT")

    # -- transactions
    def begin(self) -> None:
        self.db.execute("BEGIN IMMEDIATE")

    def commit(self) -> None:
        self.db.execute("COMMIT")

    def rollback(self) -> None:
        if self.db.in_transaction:
            self.db.execute("ROLLBACK")

    # -- meta
    def _meta(self, key: str) -> int:
        return self.db.execute("SELECT value FROM meta WHERE key = ?", (key,)).fetchone()[0]

    def _set_meta(self, key: str, value: int) -> None:
        self.db.execute("UPDATE meta SET value = ? WHERE key = ?", (value, key))

    def window(self) -> dict:
        return {"oldest_retained": self._meta("gen_oldest"), "current": self._meta("gen_current")}

    def apply_generation_config(self, advance: int, retain: int | None) -> None:
        """Test-environment control at process start (CORE 13.1).

        ``advance``: how many generations ``current`` moves forward.
        ``retain``: how many generations, counting ``current``, stay retained;
        ``oldest_retained`` is raised to ``current - retain + 1`` (never
        lowered) and records of older generations are discarded (CORE 6.3).
        """
        self.begin()
        try:
            current = self._meta("gen_current") + advance
            oldest = self._meta("gen_oldest")
            if retain is not None:
                oldest = max(oldest, current - retain + 1)
            self._set_meta("gen_current", current)
            self._set_meta("gen_oldest", oldest)
            self.db.execute("DELETE FROM commands WHERE generation < ?", (oldest,))
            self.commit()
        except BaseException:
            self.rollback()
            raise

    def next_operation_ref(self) -> str:
        seq = self._meta("op_seq") + 1
        self._set_meta("op_seq", seq)
        return f"op-{seq}"

    # -- subjects
    def revision(self, kind: str, sid: str) -> int:
        row = self.db.execute("SELECT revision FROM subjects WHERE kind = ? AND id = ?", (kind, sid)).fetchone()
        return row[0] if row else 0

    def subject(self, kind: str, sid: str):
        return self.db.execute(
            "SELECT revision, value, applied_count FROM subjects WHERE kind = ? AND id = ?", (kind, sid)
        ).fetchone()

    def write_subject(self, kind: str, sid: str, revision: int, value) -> None:
        self.db.execute(
            "INSERT INTO subjects (kind, id, revision, value, applied_count) VALUES (?, ?, ?, ?, 1) "
            "ON CONFLICT (kind, id) DO UPDATE SET revision = excluded.revision, value = excluded.value, "
            "applied_count = applied_count + 1",
            (kind, sid, revision, value),
        )

    # -- command records
    def command_record(self, scope: str, command_id: str):
        return self.db.execute(
            "SELECT generation, digest, ack, outcome FROM commands WHERE scope = ? AND command_id = ?",
            (scope, command_id),
        ).fetchone()

    def bind(self, scope: str, command_id: str, generation: int, digest: str, ack: bytes, outcome: bytes) -> None:
        self.db.execute(
            "INSERT INTO commands (scope, command_id, generation, digest, ack, outcome) VALUES (?, ?, ?, ?, ?, ?)",
            (scope, command_id, generation, digest, ack, outcome),
        )

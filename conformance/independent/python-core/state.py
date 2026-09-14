"""Durable provider state in SQLite (standard library ``sqlite3``).

One database file in the data directory holds:

- ``meta`` (integers): the deduplication window (``gen_oldest``,
  ``gen_current``), the ``operation_ref`` sequence, the event stream head
  (``ev_epoch``, ``ev_seq``), the retention boundary (``disc_epoch``,
  ``disc_seq``: every position up to and including it was discarded) and the
  capability snapshot revision (``cap_revision``);
- ``kv`` (text): the stream ID and the last capability predicates;
- ``subjects``: every provider-owned subject with its revision, value and how
  many commands changed it. The authority subject's revision is the epoch; a
  ``core.grant`` subject's value is its canonical grant record;
- ``commands``: bound command records keyed by (deduplication scope,
  command_id) with the generation they were issued under, the digest as sent,
  and the canonical acknowledgment and outcome bytes returned on replay;
- ``events``: the durable event stream (CORE 16), one canonical record per
  position;
- ``epochs``: closed stream epochs with their last sequence and the sequence
  the provider vouches through;
- ``unvouched_events``: events of closed epochs after ``vouched_through``.
  They are no longer part of the stream and are never delivered; they are
  kept only so that retention snapshots taken after that epoch still reflect
  subject state (conformance README ``events.unvouched_last``);
- ``snapshot_base``: subject states as of the retention boundary, folded from
  the discarded events, used for ``gap`` snapshots (CORE 16.4);
- ``outputs``: each execution's output spool (EXECUTION 14.1): the offset of
  the oldest retained byte, the retained bytes and the declared lost ranges.

Execution, effect and controller records (M3) are ordinary ``subjects`` rows
whose value is canonical JSON (``get_json``/``put_json``).

All writes of one command, including its events, happen inside one
``BEGIN IMMEDIATE`` transaction (CORE 10 step 8, CORE 16.3).
"""

from __future__ import annotations

import os
import secrets
import sqlite3

import valuedomain as V

SCHEMA = """
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS kv (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
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
CREATE TABLE IF NOT EXISTS events (
    epoch    INTEGER NOT NULL,
    sequence INTEGER NOT NULL,
    body     TEXT NOT NULL,
    PRIMARY KEY (epoch, sequence)
);
CREATE TABLE IF NOT EXISTS epochs (
    epoch           INTEGER PRIMARY KEY,
    last_sequence   INTEGER NOT NULL,
    vouched_through INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS unvouched_events (
    epoch    INTEGER NOT NULL,
    sequence INTEGER NOT NULL,
    body     TEXT NOT NULL,
    PRIMARY KEY (epoch, sequence)
);
CREATE TABLE IF NOT EXISTS snapshot_base (
    kind     TEXT NOT NULL,
    id       TEXT NOT NULL,
    revision INTEGER NOT NULL,
    state    TEXT NOT NULL,
    PRIMARY KEY (kind, id)
);
CREATE TABLE IF NOT EXISTS outputs (
    execution TEXT PRIMARY KEY,
    start     INTEGER NOT NULL,
    data      BLOB NOT NULL,
    lost      TEXT NOT NULL
);
"""

META_DEFAULTS = {
    "gen_oldest": 0, "gen_current": 0, "op_seq": 0,
    "ev_epoch": 1, "ev_seq": 0, "disc_epoch": 1, "disc_seq": 0,
    "cap_revision": 0,
}


EXECUTION_AXIS_EVENTS = {
    "execution.admission.changed": "admission",
    "execution.delivery.observed": "delivery",
    "execution.delivery.reconciled": "delivery",
    "execution.runtime.changed": "runtime",
    "execution.result.changed": "result",
    "execution.exit.observed": "exit",
}


def reduce_state(prior: dict | None, event: dict) -> dict:
    """Fold one event into a subject's snapshot state (CORE 16.2: payloads are
    "sufficient for a consumer's reducer") into the per-kind ``state`` CORE
    16.4 "Snapshot state" defines: ``{value}``, ``{epoch}``, ``{grant}``,
    ``{predicates}``. A revocation updates the stored record's ``state``."""
    kind, payload = event["type"], event["payload"]
    subject_kind = event["subject"]["kind"]
    if subject_kind == "execution.execution":
        # EXECUTION defines no snapshot state; this provider folds the axes
        # its events carry (G-EXEC-SNAPSHOT).
        state = dict(prior or {})
        axis = EXECUTION_AXIS_EVENTS.get(kind)
        if axis is not None and axis in payload:
            state[axis] = payload[axis]
        return state
    if subject_kind == "execution.controller":
        return {"epoch": payload["epoch"]}
    if subject_kind == "core.effect":
        state = dict(prior or {})
        aborted = set(state.get("aborted_obligations", []))
        if "obligation" in payload:
            aborted.add(payload["obligation"])
        state["aborted_obligations"] = sorted(aborted)
        return state
    if kind == "core.grant.revoked":
        state = dict(prior or {})
        grant = dict(state.get("grant", {}))
        grant["state"] = payload["state"]
        state["grant"] = grant
        return state
    return dict(payload)


def meaning(predicates: list) -> set:
    """What CORE 17.1 says raises a capability revision: the name set and each
    predicate's status, enforcement and ``evidence.source``."""
    return {(p["name"], p["status"], p.get("enforcement"), p["evidence"]["source"]) for p in predicates}


class Store:
    def __init__(self, data_dir: str):
        os.makedirs(data_dir, exist_ok=True)
        path = os.path.join(data_dir, "independent-python-core.sqlite3")
        # isolation_level=None: we issue BEGIN/COMMIT ourselves. The idle
        # re-check thread uses the connection too, always under the provider's
        # processing lock.
        self.db = sqlite3.connect(path, isolation_level=None, check_same_thread=False)
        self.db.execute("PRAGMA journal_mode=WAL")
        self.db.execute("PRAGMA synchronous=FULL")
        self.db.executescript(SCHEMA)
        self.db.execute("BEGIN IMMEDIATE")
        for key, value in META_DEFAULTS.items():
            self.db.execute("INSERT OR IGNORE INTO meta (key, value) VALUES (?, ?)", (key, value))
        # One semantic stream per store with a stable ID (CORE 16.1).
        self.db.execute("INSERT OR IGNORE INTO kv (key, value) VALUES ('stream_id', ?)",
                        ("st" + secrets.token_hex(12),))
        self.db.execute("COMMIT")
        self._stream_id = self._kv("stream_id")

    # -- transactions
    def begin(self) -> None:
        self.db.execute("BEGIN IMMEDIATE")

    def commit(self) -> None:
        self.db.execute("COMMIT")

    def rollback(self) -> None:
        if self.db.in_transaction:
            self.db.execute("ROLLBACK")

    def _in_tx(self, fn):
        self.begin()
        try:
            result = fn()
            self.commit()
            return result
        except BaseException:
            self.rollback()
            raise

    # -- meta
    def _meta(self, key: str) -> int:
        return self.db.execute("SELECT value FROM meta WHERE key = ?", (key,)).fetchone()[0]

    def _set_meta(self, key: str, value: int) -> None:
        self.db.execute("UPDATE meta SET value = ? WHERE key = ?", (value, key))

    def _kv(self, key: str):
        row = self.db.execute("SELECT value FROM kv WHERE key = ?", (key,)).fetchone()
        return row[0] if row else None

    def _set_kv(self, key: str, value: str) -> None:
        self.db.execute("INSERT INTO kv (key, value) VALUES (?, ?) ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                        (key, value))

    def window(self) -> dict:
        return {"oldest_retained": self._meta("gen_oldest"), "current": self._meta("gen_current")}

    def apply_generation_config(self, advance: int, retain: int | None) -> None:
        """Test-environment control at process start (CORE 13.1).

        ``advance``: how many generations ``current`` moves forward.
        ``retain``: how many generations, counting ``current``, stay retained;
        ``oldest_retained`` is raised to ``current - retain + 1`` (never
        lowered) and records of older generations are discarded (CORE 6.3).
        """
        def run():
            current = self._meta("gen_current") + advance
            oldest = self._meta("gen_oldest")
            if retain is not None:
                oldest = max(oldest, current - retain + 1)
            self._set_meta("gen_current", current)
            self._set_meta("gen_oldest", oldest)
            self.db.execute("DELETE FROM commands WHERE generation < ?", (oldest,))
        self._in_tx(run)

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

    # -- JSON-valued subjects (execution records, effects)
    def get_json(self, kind: str, sid: str):
        """(revision, value) or None."""
        row = self.subject(kind, sid)
        if row is None:
            return None
        return row[0], V.loads(row[1])

    def put_json(self, kind: str, sid: str, revision: int, value) -> None:
        self.write_subject(kind, sid, revision, V.canonical_text(value))

    def ids_of(self, kind: str) -> list:
        """Subject IDs of one kind in creation order."""
        return [r[0] for r in self.db.execute("SELECT id FROM subjects WHERE kind = ? ORDER BY rowid", (kind,))]

    # -- output spools (EXECUTION 14.1)
    def output(self, execution: str):
        row = self.db.execute("SELECT start, data, lost FROM outputs WHERE execution = ?", (execution,)).fetchone()
        if row is None:
            return 0, b"", []
        return row[0], bytes(row[1]), V.loads(row[2])

    def set_output(self, execution: str, start: int, data: bytes, lost: list) -> None:
        self.db.execute(
            "INSERT INTO outputs (execution, start, data, lost) VALUES (?, ?, ?, ?) "
            "ON CONFLICT (execution) DO UPDATE SET start = excluded.start, data = excluded.data, lost = excluded.lost",
            (execution, start, data, V.canonical_text(lost)))

    # -- grants (subjects of kind core.grant)
    def grant(self, gid: str):
        """(revision, record) or None."""
        row = self.subject("core.grant", gid)
        if row is None:
            return None
        return row[0], V.loads(row[1])

    def grants_in_issue_order(self):
        rows = self.db.execute("SELECT id, revision, value FROM subjects WHERE kind = 'core.grant' ORDER BY rowid")
        return [(gid, rev, V.loads(value)) for gid, rev, value in rows]

    def write_grant(self, revision: int, record: dict) -> None:
        self.write_subject("core.grant", record["id"], revision, V.canonical_text(record))

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

    # -- event stream (CORE 16)
    def stream_id(self) -> str:
        return self._stream_id

    def head(self) -> tuple[int, int]:
        return self._meta("ev_epoch"), self._meta("ev_seq")

    def last_sequence(self, epoch: int) -> int:
        cur_epoch, cur_seq = self.head()
        if epoch == cur_epoch:
            return cur_seq
        row = self.db.execute("SELECT last_sequence FROM epochs WHERE epoch = ?", (epoch,)).fetchone()
        return row[0] if row else 0

    def end_of(self, epoch: int) -> int:
        """The last deliverable sequence of an epoch: the head for the
        current epoch, ``vouched_through`` for a closed one (CORE 16.1)."""
        cur_epoch, cur_seq = self.head()
        if epoch == cur_epoch:
            return cur_seq
        return self.vouched_through(epoch)

    def vouched_through(self, epoch: int) -> int:
        row = self.db.execute("SELECT vouched_through FROM epochs WHERE epoch = ?", (epoch,)).fetchone()
        return row[0] if row else 0

    def discarded_through(self) -> tuple[int, int]:
        return self._meta("disc_epoch"), self._meta("disc_seq")

    def append_event(self, fields: dict) -> dict:
        """Append at the head of the current epoch. Call inside a transaction."""
        epoch, seq = self.head()
        seq += 1
        record = {"stream": self._stream_id, "epoch": epoch, "sequence": seq}
        record.update(fields)
        self.db.execute("INSERT INTO events (epoch, sequence, body) VALUES (?, ?, ?)",
                        (epoch, seq, V.canonical_text(record)))
        self._set_meta("ev_seq", seq)
        return record

    def event_at(self, epoch: int, seq: int):
        row = self.db.execute("SELECT body FROM events WHERE epoch = ? AND sequence = ?", (epoch, seq)).fetchone()
        return V.loads(row[0]) if row else None

    def base_snapshot(self) -> list[dict]:
        rows = self.db.execute("SELECT kind, id, revision, state FROM snapshot_base ORDER BY kind, id")
        return [{"subject": {"kind": k, "id": i}, "revision": r, "state": V.loads(st)} for k, i, r, st in rows]

    def _fold_into_base(self, body: str) -> None:
        event = V.loads(body)
        subj = event["subject"]
        prior = self.db.execute("SELECT state FROM snapshot_base WHERE kind = ? AND id = ?",
                                (subj["kind"], subj["id"])).fetchone()
        state = reduce_state(V.loads(prior[0]) if prior else None, event)
        self.db.execute(
            "INSERT INTO snapshot_base (kind, id, revision, state) VALUES (?, ?, ?, ?) "
            "ON CONFLICT (kind, id) DO UPDATE SET revision = excluded.revision, state = excluded.state",
            (subj["kind"], subj["id"], event["revision"], V.canonical_text(state)))

    def apply_event_config(self, new_epoch: bool, unvouched_last: int, retain_last: int | None) -> None:
        """Test-environment control at process start (conformance README).

        ``new_epoch``: close the current epoch, vouched through its last
        sequence minus ``unvouched_last`` (never below 0), and continue in the
        next epoch from sequence 1. Events after ``vouched_through`` leave the
        stream (CORE 16.1: "The previous epoch keeps the events the provider
        still vouches for"); subject state is unchanged.
        ``retain_last``: discard all but the newest N stream events, folding
        each discarded event into ``snapshot_base`` first. Applied after
        ``new_epoch`` (conformance README start order).
        """
        def run():
            if new_epoch:
                epoch, seq = self.head()
                vouched = max(0, seq - unvouched_last)
                self.db.execute("INSERT INTO epochs (epoch, last_sequence, vouched_through) VALUES (?, ?, ?)",
                                (epoch, seq, vouched))
                self.db.execute("INSERT INTO unvouched_events (epoch, sequence, body) "
                                "SELECT epoch, sequence, body FROM events WHERE epoch = ? AND sequence > ?",
                                (epoch, vouched))
                self.db.execute("DELETE FROM events WHERE epoch = ? AND sequence > ?", (epoch, vouched))
                self._set_meta("ev_epoch", epoch + 1)
                self._set_meta("ev_seq", 0)
            if retain_last is not None:
                (total,) = self.db.execute("SELECT COUNT(*) FROM events").fetchone()
                excess = total - retain_last
                if excess > 0:
                    rows = self.db.execute(
                        "SELECT epoch, sequence, body FROM events ORDER BY epoch, sequence LIMIT ?", (excess,)
                    ).fetchall()
                    for epoch, seq, body in rows:
                        # Unvouched changes of earlier epochs happened before
                        # this event, so a snapshot as of it includes them.
                        for (ubody,) in self.db.execute(
                                "SELECT body FROM unvouched_events WHERE epoch < ? ORDER BY epoch, sequence",
                                (epoch,)).fetchall():
                            self._fold_into_base(ubody)
                        self.db.execute("DELETE FROM unvouched_events WHERE epoch < ?", (epoch,))
                        self._fold_into_base(body)
                        self.db.execute("DELETE FROM events WHERE epoch = ? AND sequence = ?", (epoch, seq))
                    self._set_meta("disc_epoch", rows[-1][0])
                    self._set_meta("disc_seq", rows[-1][1])
        self._in_tx(run)

    # -- capability snapshot (CORE 17)
    def capabilities(self) -> tuple[int, list]:
        text = self._kv("cap_predicates")
        return self._meta("cap_revision"), (V.loads(text) if text is not None else [])

    def apply_capabilities(self, predicates: list, event_fields) -> None:
        """Record the snapshot observed at this start. A new store's first
        snapshot is revision 1 without an event; each later change raises the
        revision and appends a provider-origin event (CORE 17.3)."""
        def run():
            revision, stored = self.capabilities()
            if revision and meaning(stored) == meaning(predicates):
                # CORE 17.1: observed_at alone does not raise the revision.
                self._set_kv("cap_predicates", V.canonical_text(predicates))
                return
            revision += 1
            self._set_meta("cap_revision", revision)
            self._set_kv("cap_predicates", V.canonical_text(predicates))
            if revision > 1:
                self.append_event(event_fields(revision))
        self._in_tx(run)

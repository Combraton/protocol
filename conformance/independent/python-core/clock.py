"""The provider clock (CORE 15.2, EXECUTION 8, decision 007 section 2).

Three sources, chosen by the launch configuration:

- ``clock.fixed``: one instant for the whole process;
- ``clock.file``: a file the runner replaces atomically. The provider reads it
  at launch (a missing or malformed file refuses the start) and again whenever
  it needs the time. During a run a missing or malformed file, or an instant
  earlier than the last good one, keeps the last good instant and writes a
  diagnostic to standard error. It never falls back to the system clock;
- neither: the system clock in UTC at second precision.

The clock is read once per unit of work (one request, one idle re-check) and
that reading is used for every decision the unit makes, so one command never
sees two different times (G-CLOCK-READING).
"""

from __future__ import annotations

import datetime as _dt
import re
import sys
import time

INSTANT = re.compile(r"[0-9]{4}-(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])T([01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9]Z")
FORMAT = "%Y-%m-%dT%H:%M:%SZ"


class ClockError(Exception):
    pass


def valid_instant(text) -> bool:
    if not isinstance(text, str) or not INSTANT.fullmatch(text):
        return False
    try:
        _dt.datetime.strptime(text, FORMAT)
    except ValueError:  # e.g. 2030-02-30
        return False
    return True


def add_seconds(instant: str, seconds: int) -> str:
    moment = _dt.datetime.strptime(instant, FORMAT) + _dt.timedelta(seconds=seconds)
    return moment.strftime(FORMAT)


def _diag(*parts) -> None:
    print("[independent-python-core] clock:", *parts, file=sys.stderr, flush=True)


class Clock:
    def __init__(self, fixed: str | None = None, path: str | None = None):
        self.fixed = fixed
        self.path = path
        self.last_good: str | None = None
        self.current: str | None = None
        self._last_diag = None
        if path is not None:
            instant = self._read_file()
            if instant is None:
                raise ClockError(f"clock file {path!r} is missing or does not hold one instant")
            self.last_good = instant
        self.refresh()

    def _read_file(self) -> str | None:
        try:
            with open(self.path, "rb") as fh:
                raw = fh.read(4096)
            text = raw.decode("utf-8").strip(" \t\r\n")
        except (OSError, UnicodeDecodeError):
            return None
        return text if valid_instant(text) else None

    def refresh(self) -> str:
        """Take one reading for the next unit of work."""
        if self.path is not None:
            instant = self._read_file()
            problem = None
            if instant is None:
                problem = ("clock file missing or malformed; keeping", self.last_good)
            elif instant < self.last_good:
                problem = ("clock file moved backward to", instant, "; keeping", self.last_good)
            else:
                self.last_good = instant
            if problem is not None and problem != self._last_diag:
                _diag(*problem)  # once per distinct problem, not on every idle re-check
            self._last_diag = problem
            self.current = self.last_good
        elif self.fixed is not None:
            self.current = self.fixed
        else:
            self.current = time.strftime(FORMAT, time.gmtime())
        return self.current

    def now(self) -> str:
        return self.current

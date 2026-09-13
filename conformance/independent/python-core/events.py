"""Event stream reading: cursors and ordered items (CORE 16.4, 16.5).

A position is ``(epoch, sequence)``, ordered lexicographically. A cursor
denotes the position of the last item a reader has passed; ``from: "start"``
is ``(1, 0)`` and ``from: "now"`` is the current head. Cursors are opaque to
callers; this implementation encodes ``<stream>.<epoch>.<sequence>``.
"""

from __future__ import annotations

import re

from valuedomain import MAX_SAFE

CURSOR = re.compile(r"([A-Za-z0-9]{1,64})\.([1-9][0-9]{0,15})\.(0|[1-9][0-9]{0,15})")


class CursorError(Exception):
    def __init__(self, reason: str):
        super().__init__(reason)
        self.reason = reason


def encode_cursor(store, pos: tuple[int, int]) -> str:
    return f"{store.stream_id()}.{pos[0]}.{pos[1]}"


def decode_cursor(store, text: str) -> tuple[int, int]:
    m = CURSOR.fullmatch(text)
    if m is None or int(m.group(2)) > MAX_SAFE or int(m.group(3)) > MAX_SAFE:
        raise CursorError("malformed")
    if m.group(1) != store.stream_id():
        raise CursorError("other_stream")
    pos = (int(m.group(2)), int(m.group(3)))
    # Only a position after the current head is beyond the end. A position in
    # a closed epoch past its vouched_through is accepted: the reader learns
    # from the epoch_change that it is not vouched for (E-CURSOR-OLD-EPOCH).
    if pos > store.head():
        raise CursorError("beyond_end")
    return pos


def start_position(store, payload: dict) -> tuple[int, int]:
    if "cursor" in payload:
        return decode_cursor(store, payload["cursor"])
    if payload["from"] == "now":
        return store.head()
    return (1, 0)


def read_items(store, pos: tuple[int, int], limit: int, visible, kinds):
    """Return ``(items, next_pos, hidden)``.

    ``visible(subject)`` says whether the reader's authorization covers a
    subject; ``kinds`` is the optional kind filter. ``hidden`` is true when an
    event in the scanned range was left out by either. ``next_pos`` is the
    last position scanned: the last item's position when ``limit`` is
    reached, otherwise the stream head (E-CURSOR-AFTER-HIDDEN).
    """
    items: list[dict] = []
    hidden = False
    head_epoch = store.head()[0]
    disc = store.discarded_through()

    def shown(subject) -> bool:
        return (kinds is None or subject["kind"] in kinds) and visible(subject)

    while len(items) < limit:
        epoch, seq = pos
        if seq >= store.last_sequence(epoch):
            if epoch >= head_epoch:
                break
            items.append({"epoch_change": {"from_epoch": epoch, "to_epoch": epoch + 1,
                                           "vouched_through": store.vouched_through(epoch)}})
            pos = (epoch + 1, 0)
            continue
        nxt = (epoch, seq + 1)
        if nxt <= disc:
            # No silent gaps (CORE 16.4): the first item after a discarded
            # range is a gap whose snapshot is as of the last discarded
            # position (E-GAP-TO).
            through, snapshot = disc, store.base_snapshot()
            subjects = [s for s in snapshot if shown(s["subject"])]
            if len(subjects) != len(snapshot):
                hidden = True
            as_of = {"epoch": through[0], "sequence": through[1]}
            items.append({"gap": {"kind": "retention",
                                  "from": {"epoch": nxt[0], "sequence": nxt[1]},
                                  "to": dict(as_of),
                                  "snapshot": {"as_of": dict(as_of), "subjects": subjects}}})
            pos = through
            continue
        pos = nxt
        event = store.event_at(*nxt)
        if event is None:  # cannot happen: stored epochs are contiguous
            raise RuntimeError(f"missing event at {nxt}")
        if shown(event["subject"]):
            items.append({"event": event})
        else:
            hidden = True
    return items, pos, hidden

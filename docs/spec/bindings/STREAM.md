# Local stream binding `stream/1` — release draft

> **Status: accepted draft for Protocol 0.1 (stdio form); Unix-socket form in M2.** Supported platforms: macOS and Linux. Milestone M1 specifies the stdio form. M2 adds Unix domain sockets. Evidence and alternatives: [decision 004](../../decisions/004-local-stream-binding.md). Domain semantics: [Core](../profiles/CORE.md).

This binding carries JSON-RPC 2.0 messages over a reliable, ordered byte stream between two local processes. The domain envelope does not depend on it: another binding could carry the same envelopes.

## 1. Frames

1. **Frame boundary.** A frame is a sequence of bytes terminated by one line feed (`0x0A`). The terminator is not part of the frame.
2. **Content.** A frame's bytes MUST be valid UTF-8 (UTF-8-encoded surrogates are invalid UTF-8) forming one JSON text that conforms to I-JSON ([RFC 7493](https://www.rfc-editor.org/rfc/rfc7493)):
   - no duplicate member names;
   - no unpaired surrogates or noncharacters, escaped or raw;
   - numbers that are integers within ±(2^53 − 1).

   Because JSON escapes every control character inside strings ([RFC 8259 §7](https://www.rfc-editor.org/rfc/rfc8259#section-7)), a line feed can never occur inside a compactly serialized JSON text.
3. **Senders** MUST serialize without line feeds, MUST NOT write a byte-order mark, and SHOULD NOT emit other insignificant whitespace.
4. **Receivers** accept RFC 8259 insignificant whitespace (space, tab, carriage return) around and inside the JSON text. A frame containing only such whitespace, or no bytes at all, is ignored.
5. **Frame limit.** Each receiver applies a frame limit counted in bytes, excluding the terminator.
   - Until negotiation completes on a session, the limit is **1,048,576 bytes** (1 MiB) in both directions.
   - After negotiation, which applies to every frame after the negotiate request, each side's limit is the value it advertised: the provider in its negotiation result, the caller in its negotiation request's `receive_limits`. The value is never below 1 MiB.
   - A receiver MUST NOT buffer more than limit + 1 bytes of a frame it has not finished reading.
6. **Unterminated frame.** Bytes after the last terminator when the stream ends are discarded without parsing.

## 2. Frame-level failures close the connection

These failures mean the stream can no longer be trusted. The receiver sends one error response with `"id": null`, `data.retry` `"no"` and empty `data.details`. It flushes for at most one second and closes the connection without reading further input:

| Failure | JSON-RPC `error.code` | `error.data.code` |
|---|---|---|
| Frame exceeds the limit | `-32010` | `frame_too_large` |
| Invalid UTF-8 | `-32700` | `invalid_utf8` |
| Not a JSON text, or violates I-JSON | `-32700` | `parse_error` |

A receiver MUST NOT act on any content of a failed frame.

## 3. JSON-RPC mapping

- **Object shape.** Every frame is one JSON-RPC 2.0 object with `"jsonrpc": "2.0"`. Batches (top-level arrays) are not supported. A top-level array or scalar gets an `invalid_request` error with `"id": null`, and the connection stays open.
- **Malformed requests.** An object with a valid `id` but a wrong `jsonrpc`, a missing or non-string `method`, a missing or non-object `params`, or any member other than `jsonrpc`, `id`, `method` and `params` is `invalid_request`, echoing the `id`. `invalid_request` always has `data.retry` `"no"` and empty `data.details`, and the connection stays open.
- **Request IDs.** A request `id` is a string of 1–128 Unicode code points or an integer within ±(2^53 − 1). A null, boolean, object, array or empty-string `id` is `invalid_request` with `"id": null`. A fractional or out-of-range number never reaches this check; it is a frame-level `parse_error` (§2). The `id` matches a response to a request on one connection only. It MUST NOT be used for deduplication, and a caller MAY reuse it after the response arrives.
- **Methods.** The `method` is the operation name and MUST equal the envelope's `operation`. A mismatch is `invalid_envelope`. `params` is the domain envelope object.
- **Notifications from a caller** — objects without `id` that have `"jsonrpc": "2.0"` and a string `method` — are not used in 0.1. Any other object without `id` is `invalid_request` with `"id": null`. A provider MUST NOT process one and MUST NOT reply. It MAY log it. A command sent as a notification therefore has no effect; the caller learns nothing, as JSON-RPC requires.
- **Provider notifications** carry events. They are **reserved for M2**.
- **Error codes.**

| Kind | JSON-RPC `code` | `data.code` examples |
|---|---|---|
| Frame-level (§2) | `-32700`, `-32010` | `parse_error`, `invalid_utf8`, `frame_too_large` |
| Malformed JSON-RPC object on a valid frame | `-32600` | `invalid_request` |
| Unknown operation | `-32601` | `method_not_found` |
| Provider overloaded; nothing processed | `-32011` | `overloaded` (retry `same_command`) |
| Any domain error from a profile | `1` | Profile error codes, such as `idempotency_conflict` |

The `data` object always has the members `code`, `retry` and `details` defined in [Core §12](../profiles/CORE.md#12-errors).

- **Ordering.** A provider MAY process requests on one connection concurrently and respond in any order. A caller that needs ordering sends the next request after the previous response. Commands on the same subject are serialized by the provider's owner transaction, not by arrival order.

## 4. Session end and disconnect

- **End of input.** When a side reads end of input, it sends no new requests on that connection. It may finish writing responses to requests already received within a bounded drain period, then closes.
- **Disconnect is not cancellation.** Closing or losing a connection neither cancels nor confirms any command. A command whose response was not received may or may not have been bound. The caller retransmits the same command with the same `command_id` on a new session, or queries the subject.
- **Write errors.** A writer that finds the peer gone MUST treat pending responses as undelivered and must not crash the owning service. For example, handle `EPIPE` rather than dying on `SIGPIPE`.

## 5. stdio form

- **Streams.** The caller spawns the provider. The caller writes frames to the provider's standard input and reads frames from its standard output. Standard output carries only frames; diagnostics go to standard error.
- **Principal.** The spawner assigns the principal through the provider's launch configuration. The anonymous pipe pair is private to the two processes, so the binding performs no further channel authentication.
- **Inherited descriptors.** A provider MUST NOT let child processes inherit its protocol standard input or output. A harness or tool child writing to the protocol stream would corrupt or inject frames.
- **Exit.** On end of standard input, after the drain period, the provider exits with status 0. After closing on a frame-level failure (§2) it also exits with status 0. A nonzero status means the provider could not start or failed. Durable state is kept; a new process is a new session over the same state.

## 6. Unix domain socket form (reserved for M2)

The planned form, not yet specified normatively:

- A pathname socket, never an abstract one, created with mode `0600` inside a directory with mode `0700` owned by the provider's user.
- Rejection of peers whose effective user ID differs from the provider's.

A peer credential proves only "same operating-system user at connect time". It does not distinguish a trusted caller from a coding agent running as the same user. Mapping a connection to a protocol principal therefore needs an application-level credential presented during the handshake. That credential's form is an open owner decision (release plan U12). Windows is unsupported in 0.1 (U4).

## 7. What this binding does not establish

Receipt of a frame is not processing. A response is not proof that a later external effect happened. A clean close is not completion of in-flight work. Transport request IDs, connection identity and process IDs are not domain identities.

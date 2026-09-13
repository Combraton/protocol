# 004: Local stream binding and framing

- **Status:** proposed. Needs owner review before release; the Unix-socket principal credential is an open owner decision.
- **Date:** 2026-09-13.
- **Owner/authority:** Protocol session under the Protocol 0.1 kickoff; [tracking issue #1](https://github.com/Combraton/protocol/issues/1).
- **Affects:** [STREAM binding](../spec/bindings/STREAM.md); matrix rows TRN-1, TRN-2, TRN-3, TRN-5, CORE-1, CORE-5.
- **Supersedes:** nothing. Refines [SPEC §11](../spec/SPEC.md#11-versioning-and-transport), which recommended JSON-RPC 2.0 over an authenticated Unix socket or named pipe with explicit bounded framing, and required the pinned binding to define encoding, maximum frame length, oversized-frame rejection and disconnect behavior.

## Problem

Two independently written local processes must agree on:

- where one message ends;
- how large a message may be;
- what happens to bad input;
- what a lost connection means;
- how a connection maps to a principal.

The same binding should work when a caller spawns a provider (stdio) and when it connects to a running service (Unix socket).

## Evidence consulted

A read-only research pass on 2026-09-13 used these sources. Repository links are pinned to the inspected commits; specification pages are cited by revision or date.

| Source | Finding |
|---|---|
| [JSON-RPC 2.0](https://www.jsonrpc.org/specification) | Transport-agnostic, with no framing or limits. IDs may be string, number or null, with null discouraged. Batches are optional. Error codes −32768..−32000 are reserved, −32000..−32099 implementation-defined. |
| [MCP stdio transport, revision 2026-07-28](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/cc2a84f5ca5404b2949683f7d7876f623344294f/docs/specification/2026-07-28/basic/transports/stdio.mdx) and [transport index](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/cc2a84f5ca5404b2949683f7d7876f623344294f/docs/specification/2026-07-28/basic/transports/index.mdx) | Newline-delimited messages with no embedded newlines, UTF-8. The same wire format is stated to work over Unix sockets. No size limit in the spec. MCP removed batches in [2025-06-18](https://modelcontextprotocol.io/specification/2025-06-18/changelog). |
| [MCP TypeScript SDK stdio](https://github.com/modelcontextprotocol/typescript-sdk/blob/b65426158ed9f29aea8ef3dc09ca22d7d9d6f970/packages/core-internal/src/shared/stdio.ts) | 10 MiB buffer default; overflow closes the transport; non-JSON lines skipped silently. |
| [Codex app-server docs](https://learn.chatgpt.com/docs/app-server) and [transport source](https://github.com/openai/codex/blob/1715e55076737158ba61d43158ede504de6d4ce1/codex-rs/app-server-transport/src/transport/mod.rs) | JSONL on stdio with no line cap. Invalid JSON logged and ignored. Overload answered with −32001. Socket mode 0600 in a 0700 directory. |
| [ACP transports](https://github.com/agentclientprotocol/agent-client-protocol/blob/ada6b108389a63a2625298f2be3eacde33a1d8c5/docs/protocol/v2/transports.mdx) | Newline-delimited UTF-8 JSON-RPC, no maximum; v2 allows batches. |
| [LSP 3.17 base protocol](https://github.com/microsoft/language-server-protocol/blob/3d9ba5d8e28ab7a577bb5aed1f27c166da0cb558/_specifications/lsp/3.17/specification.md) and [vscode-jsonrpc reader](https://github.com/microsoft/vscode-languageserver-node/blob/5010cdf9822e1038a30ee7eb6ee5d7aaa79acc4a/jsonrpc/src/common/messageReader.ts) | Content-Length headers, no maximum; a bad header leaves the stream unsynchronized. |
| [RFC 8259 §7](https://www.rfc-editor.org/rfc/rfc8259#section-7), [RFC 3629](https://www.rfc-editor.org/rfc/rfc3629), [RFC 7493](https://www.rfc-editor.org/rfc/rfc7493) | Control characters must be escaped in strings, and every byte of a multi-byte UTF-8 sequence has its high bit set. So a line-feed byte cannot occur inside compact JSON. I-JSON forbids duplicate names and restricts numbers to interoperable integers. |
| [unix(7)](https://man7.org/linux/man-pages/man7/unix.7.html), macOS getpeereid(3)/unix(4), [XNU uipc_usrreq.c](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/kern/uipc_usrreq.c) | Peer credentials are the effective user and group captured at connect time. Linux warns portable programs not to rely on socket file permissions alone. The macOS peer PID is the last user of the socket, not a connect-time snapshot. |
| [Microsoft named pipe security](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights) and [CreateNamedPipe](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipea) | The default DACL gives read access to Everyone. `FILE_FLAG_FIRST_PIPE_INSTANCE` is needed against pipe-name squatting. |
| [MCP authorization](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/cc2a84f5ca5404b2949683f7d7876f623344294f/docs/specification/2026-07-28/basic/authorization/index.mdx) | stdio implementations take credentials from the environment rather than a channel handshake. |

The following are inference from these sources:

- Operating-system peer credentials establish only "same user". Coding agents launched by PIO usually run as that user, so a same-user check cannot tell PIO's client from an agent.
- Silently skipping invalid lines, as MCP-TS and Codex do, hides bugs and stdout pollution on a local link.

## Decision (proposed)

- **Framing: bounded newline-delimited JSON**, identical over stdio and Unix sockets.
  - Strict UTF-8 and I-JSON.
  - Fixed 1 MiB frame limit until negotiation; afterwards a per-receiver advertised limit, never below 1 MiB.
  - Readers never buffer more than limit + 1 bytes.
- **Frame-level failures close the connection.** Oversized frames, invalid UTF-8, non-JSON or I-JSON violations get one error with `id: null` and then a close. A structurally invalid JSON-RPC object on a valid frame gets an error and the connection stays open.
- **JSON-RPC 2.0 rules.**
  - No batches.
  - String or safe-integer request IDs only.
  - `method` equals the envelope `operation`.
  - Caller notifications are never processed.
  - Domain errors use JSON-RPC code `1` with a symbolic `data.code`.
  - Binding errors use reserved or implementation-defined codes, avoiding `−32001` because Codex uses it differently.
- **Disconnect** is neither cancellation nor confirmation; callers reconcile by command identity.
- **stdio principal:** assigned by the spawner's launch configuration. Protocol descriptors must not be inherited by children.
- **Unix sockets (M2):** 0700 directory, 0600 pathname socket and a same-user check, **plus** an application-level credential in the handshake to establish the protocol principal. The credential's form is open for the owner (release plan U3/U9).
- **Windows named pipes:** deferred (U4).

## Alternatives

| Alternative | Why not selected |
|---|---|
| **Content-Length headers (LSP/DAP)** | Can reject an oversized body before reading it. But it adds a header grammar, cannot resynchronize after a bad header, and in practice is implemented without limits. It also diverges from the MCP/ACP/Codex stdio ecosystem. |
| **4-byte length prefix (gRPC, Chrome native messaging)** | Constant-time framing. But it is not inspectable with ordinary tools, a corrupted prefix desynchronizes permanently, and byte-order mistakes are documented in the wild (Chrome uses native order). |
| **Resynchronizing after an oversized frame** | Possible with newline framing, but the sender's request can never be answered because its ID was not read. Closing is simpler, and domain reconciliation already handles lost responses. |
| **Allowing batches (ACP v2)** | Adds nothing over pipelining on a stream and complicates limits and backpressure. |
| **Skipping invalid lines (MCP-TS, Codex)** | Tolerant but hides corruption. Rejected for determinism. |
| **WebSocket over Unix socket (Codex)** | Gives message framing and bearer authentication, but cannot be used identically over stdio. |

## Consequences

- **Pretty-printed JSON** cannot be sent; tooling must compact it.
- **Large content** (artifacts, packets) must not travel as oversized frames. Evidence and Context profiles will use staged transfer or fetch references.
- **Strict closing** means a buggy peer loses its connection rather than having frames skipped. Tests will expose such bugs early.

## Verification

M1 fixtures exercise:

- frames at the limit and at limit + 1;
- invalid UTF-8;
- a raw line feed inside a string;
- duplicate member names;
- a top-level array;
- a null request ID;
- a caller notification carrying a command (the command must have no effect);
- an unterminated frame at end of input;
- a method/operation mismatch.

A mutant provider with an unbounded reader and one that skips invalid lines must fail. Unix-socket same-user rejection is exercised on macOS and Linux CI in M2.

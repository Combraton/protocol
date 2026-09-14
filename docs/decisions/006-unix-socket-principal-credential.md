# 006: Unix-socket principal credentials

- **Status:** accepted on 2026-09-13. The owner chose "Credential file + authenticate" for release plan U12 among four presented options.
- **Date:** 2026-09-13.
- **Owner/authority:** repository owner; Protocol session implements. [Issue #1](https://github.com/Combraton/protocol/issues/1).
- **Affects:** [STREAM §6](../spec/bindings/STREAM.md#6-unix-domain-socket-form), [Core §18](../spec/profiles/CORE.md#18-principal-credentials-on-shared-connections); matrix rows TRN-3 and CORE-15.
- **Supersedes:** the open credential item in [decision 004](004-local-stream-binding.md).

## Problem

A Unix domain socket can be reached by any process the socket's permissions allow. Operating-system peer credentials establish only the effective user at connect time, per the [research recorded in decision 004](004-local-stream-binding.md#evidence-consulted). Coding agents launched by PIO normally run as that same user, so a same-user check cannot tell a supervising client from an agent. Grants, authority and per-principal deduplication all need a principal the provider can trust.

## Options presented to the owner

| Option | Summary | Outcome |
|---|---|---|
| **Credential file + `core.authenticate`** | Per-principal random secret, readable only by the user, presented before negotiation, on top of the same-user check | **Selected** |
| Same-user check only | Every same-user process is one principal | Rejected: an agent could act as its supervisor |
| Separate OS users | The peer user identifies the principal | Rejected for 0.1: heavy setup for standalone users |
| stdio only | No socket binding in 0.1 | Rejected: a long-running PIO daemon needs clients that connect later |

## Decision

1. **Socket placement.** Pathname socket, mode `0600`, inside a directory owned by the provider's user with mode `0700`. A provider refuses to start if the directory is a symlink, is not owned by its user, or grants any group or other permission.
2. **Peer check.** On every accepted connection the provider reads the peer's effective user ID (`SO_PEERCRED` on Linux, `getpeereid` on macOS). It closes a connection from any other user without sending a frame.
3. **Credentials.**
   - A credential is a string `ccred1.<principal>.<secret>`, where `<secret>` is 32 random bytes in unpadded base64url.
   - Providers store only a SHA-256 digest of the whole credential, compare in constant time, and never log, echo or publish credentials.
   - Issuing, rotating and revoking credentials is provider administration, outside the protocol operations.
   - A provider that issues a credential for a local caller writes it to a `0600` file inside a `0700` directory.
4. **Authentication.**
   - A socket session starts unauthenticated. Before authenticating, it may call only `core.describe` and `core.authenticate`; anything else is `authentication_required`.
   - `core.authenticate` with a valid, unrevoked credential binds the session principal for the rest of the connection. Any failure is `authentication_failed`, identical for unknown, malformed and revoked credentials.
   - A provider may close the connection after repeated failures.
   - stdio sessions are authenticated by their launch, so `core.authenticate` there is `already_authenticated`.
5. **Honest limit.** A credential separates principals only if agents cannot read the credential file. PIO must keep credential paths outside agent-readable scope where it enforces file access, and report `cooperative` protection where it cannot. Credentials never go in agent environments, command lines or logs.

## Consequences

- One provider process serves several concurrent authenticated sessions sharing state. Owner transactions must serialize command processing across sessions (Core §10), and subscriptions receive events committed by any session.
- Conformance runs every applicable fixture over both stdio and Unix-socket participants. The runner authenticates socket sessions automatically unless a fixture tests authentication itself.
- Rejecting a different peer user needs a second operating-system account, which a portable fixture cannot assume. *Updated 2026-09-13 in the M2 close-out:* `conformance/scripts/peer_user_check.py` now tests the rule in CI on Linux and macOS. A client run as root through passwordless `sudo` is a different user that directory permissions do not stop, so the provider's own peer-credential check is what must close it. A same-user control and the mutant `skip-peer-check` show the check detects the rule. **Remaining limit:** root is the only other user exercised, and hosts without passwordless `sudo` report `unsupported`. The directory-permission refusal is tested by a fixture.

## Verification

Socket fixtures cover:
- authentication required before negotiation;
- principal mapping across concurrent sessions;
- identical failures for unknown, malformed and revoked credentials, with no credential echoed;
- refusal to start with an unsafe socket directory;
- event delivery to a subscription from a command on another session.

Reference mutants cover skipped authentication, distinguishable failures, echoed credentials and unchecked directories. The full suite also runs against the reference provider over a Unix socket.

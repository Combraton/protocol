# Verification available now

This repository contains architecture documentation, **draft** Protocol 0.1 schemas and a **draft** Core conformance suite (milestone M1). The suite checks the Core command path and the stdio stream binding. It is not a released conformance claim, and it proves nothing about Execution, Evidence, Knowledge, Context or Verification, which have no fixtures yet.

## Documentation checks

From this repository's root:

```sh
python3 scripts/check_docs.py
git diff --check
```

Python 3 standard library is sufficient; there is no package install step. The script checks required entrypoints, the local `CLAUDE.md` import, ordinary Markdown file targets, balanced fences and private machine paths in Markdown. It exits nonzero on an error. It reports cross-repository links it could not check.

When all five clones are siblings under one directory, also run from each repository root:

```sh
python3 scripts/check_docs.py --workspace ..
```

This additionally resolves Combraton GitHub main-file links against the sibling checkouts, including benchmarks. It does not prove those checkouts match the remote branches. Record their commits when using the result as integration evidence.

The **Documentation** GitHub Actions job runs the first script on pushes and pull requests, with read-only contents permissions. It does not fetch sibling repositories. Remote URL reachability, Markdown fragment targets, Mermaid rendering, source-manifest consistency, semantic correctness, live harness instruction loading and product behavior need separate inspection.

## Conformance suite (draft, M1)

Toolchain: Rust `1.97.1`, pinned by `rust-toolchain.toml`; `rustup` installs it on first use. Dependencies are locked in `Cargo.lock`. From the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
cargo test --workspace --locked
./target/debug/combraton-conformance self-test
./target/debug/combraton-conformance check-fixtures
./target/debug/combraton-conformance run --participant conformance/participants/reference-provider.json
./target/debug/combraton-conformance check-mutants --participant conformance/participants/reference-provider.json
./target/debug/combraton-conformance run --participant conformance/participants/reference-provider-unix.json --out conformance/results/reference-unix
./target/debug/combraton-conformance check-mutants --participant conformance/participants/reference-provider-unix.json --out conformance/results/reference-unix
./target/debug/combraton-conformance run --participant conformance/participants/independent-python-core.json --out conformance/results/independent-python-core
python3 conformance/scripts/peer_user_check.py --require --out conformance/results/peer-user
python3 conformance/scripts/repeat_fixture.py --participant conformance/participants/reference-provider-unix.json --fixture socket.subscription-recheck-race-regression --runs 20 --out conformance/results/race-repeat/correct
python3 conformance/scripts/repeat_fixture.py --participant conformance/participants/reference-provider-unix.json --fixture socket.subscription-recheck-race-regression --mutant recheck-outside-lock --runs 20 --out conformance/results/race-repeat/recheck-outside-lock
python3 conformance/scripts/peer_user_check.py --require --mutant skip-peer-check --expect accepted --out conformance/results/peer-user-mutant
```

The two peer-user commands need passwordless `sudo`, as on GitHub-hosted runners. Without `--require` they exit 77 with outcome `unsupported` on hosts that lack it.

What each command establishes:

| Command | Exit 0 means | Does not mean |
|---|---|---|
| `cargo test` | The reference provider's hand-written parser and canonical encoder agree with `conformance/vectors/encoding.json`; runner pattern matching works. Reference-only checks also pass: its capability revision follows CORE §17.1 when only an evidence source or `observed_at` changes, and it refuses constructed cursors past its head. | Portable conformance: these tests know the reference's private cursor format and store |
| `self-test` | The runner's strict parser and RFC 8785 canonicalization agree with the vectors | That the vectors are correct; see the cross-checks below |
| `check-fixtures` | Every fixture validates against `conformance/schemas/fixture.schema.json`, IDs are unique, requirement IDs exist in the matrix, and negative fixtures declare mutants | That fixtures are semantically right |
| `run` | The named participant passed every applicable fixture over its binding (stdio or Unix socket); `unsupported` and `skipped` fixtures are reported separately and not run. A result manifest with fixture digests and per-case transcripts is written to `conformance/results/<participant>/` (not committed). | Conformance for any profile without fixtures; real-adapter behavior |
| `run` on `independent-python-core.json` | The spec-only Python implementation passes every fixture applicable to the profiles, features and binding it claims (Core with grants, events, capabilities and effects; `execution/1` with its optional features and context revalidation; `evidence/1` and `context/1`; the clock file, scripted executor, store faults, evidence store and context script; over stdio) | That it or the reference is correct where fixtures are silent; see its divergence log |
| `check-mutants` | Each of the reference provider's deliberately broken mutants fails every fixture that declares it, and every mutant is declared by at least one fixture | That no other wrong implementation can pass. Mutants share the reference author's assumptions. |
| `repeat_fixture.py` | Every one of the runs had the intended outcome: the correct provider passed each time, or the mutant failed each time at the stated step with the stated reason | That no other interleaving is wrong; it proves only the scripted ordering is deterministic |
| `peer_user_check.py` | A root client, a different OS user, is closed without a frame while a same-user client is answered; with mutant `skip-peer-check` the root client is answered, so the check detects the missing rule | Separation between principals of the same OS user; that is the credential's job (decision 006) |

To test another stdio provider, write a participant descriptor like `conformance/participants/reference-provider.json` (launch argv with `{repo}`, `{data_dir}` and `{config_file}` placeholders, plus claimed profiles) and pass it to `run`. The provider must accept a data directory and the launch configuration file ([decision 001](decisions/001-conformance-suite-architecture.md)).

### Independent encoding cross-checks

The encoding vectors are also checked by two non-Rust implementations:

```sh
# Python rfc8785 0.1.4 (uv)
cd conformance/crosscheck && uv run python generate_encoding_vectors.py --check ../vectors/encoding.json
# Node canonicalize 5.0.0, installed into a temporary directory
npm install --no-audit --no-fund --prefix "$TMPDIR/jcs" canonicalize@5.0.0
node conformance/crosscheck/crosscheck_encoding_vectors.mjs "$TMPDIR/jcs/node_modules/canonicalize/lib/canonicalize.js" conformance/vectors/encoding.json
```

The **Conformance** GitHub Actions workflow runs all of the above: the Rust job on Ubuntu and macOS, and the cross-check job on Ubuntu.

### Limits of this evidence

- **Bindings.** Both the stdio and Unix-socket forms are exercised; the socket participant runs every applicable fixture after automatic authentication.
- **Socket identity.** Rejecting a peer of a different operating-system user is not tested, because CI has one account (decision 006).
- **Deferred to M3.** Effects, telemetry lost ranges and backpressure.
- Two providers are tested: the reference provider (written by the fixture author) and an independent Python provider written from the documents only. The independent one covers Core with `core.grants`, `core.events`, `core.capabilities` and `core.effects`, and `execution/1` with its optional features, over stdio. The Unix-socket binding, credentials, `core.events.backpressure` and implementation-specific barriers and signals have only the reference implementation. Since the M4 independent pass, the independent provider also covers `evidence/1`, `context/1` and `execution.context_revalidation` on a single provider. It does not cover peers, fetch grants, evidence outputs or investigation executions, and it refuses to start with those launch keys. Composition fixtures (separately running participants) need the Unix-socket binding and run every named participant from the descriptor under test; stdio participants, including the independent provider, skip them. Mixed implementations in one composition are not exercised.
- States reached through launch configuration (restart, generation advancement) are test-environment control, not product operations.
- No PIO, CBR or benchmark integration has run.

For a change, report the command, exit status, environment, tested revision, real versus simulated dependencies, evidence location and untested limitations. Preserve the producer exit code when displaying shortened logs. Review the relevant diff against an explicit base/head.

## Standalone release evidence

Follow [standalone release gates](https://github.com/Combraton/combraton/blob/main/docs/STANDALONE-RELEASES.md). Core no-optional-service tests, protocol conformance, real-adapter integration, comparative outcomes and UI usability are separate evidence classes. The [benchmarks repository](https://github.com/Combraton/benchmarks) owns cross-product scenarios and results, not this service's normative contract. No runtime benchmark has been implemented or run.

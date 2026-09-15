# Verification — Protocol v0.1.0

This repository publishes the Protocol 0.1 contracts (six profiles and the local bindings), their schemas, and a conformance suite with a reference provider, an independent provider and third-party example clients. The release record is [docs/release/0.1](release/0.1/README.md). A result is a conformance claim only for the fixtures it lists as passing; `skipped` and `unsupported` outcomes are coverage limits.

**Two supported ways to verify v0.1.0:**
- **Git checkout** of tag `v0.1.0`. Use the commands in this document; `release_inventory.py --verify` lists files from Git.
- **Extracted source bundle** `combraton-protocol-0.1.0-source.tar.gz`, which has no Git metadata:
  1. Check the archive against the release's `SHA256SUMS`: `shasum -a 256 -c SHA256SUMS --ignore-missing` on macOS, or `sha256sum -c --ignore-missing SHA256SUMS` on Linux.
  2. From the extracted directory, check every file against `BUNDLE-SHA256SUMS`: `shasum -a 256 -c BUNDLE-SHA256SUMS` (or `sha256sum -c BUNDLE-SHA256SUMS`).
  3. Run `python3 scripts/release_inventory.py --verify`, which uses file-system mode automatically there, followed by the documentation and conformance commands below.

  A maintainer runs the same path end to end with `python3 scripts/release.py verify-bundle --archive <file> --work <empty dir> --full`. The compatibility commands that build the pinned M5 release need a Git checkout.

The **normative inventory** (`docs/release/0.1/inventory.json`) covers the files consumers pin: specs, schemas, fixtures, vectors and the license. `BUNDLE-SHA256SUMS` covers every file of the distributed bundle, including the runner, reference provider, independent provider and third-party examples.

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

## Conformance suite

Toolchain: Rust `1.97.1`, pinned by `rust-toolchain.toml`; `rustup` installs it on first use. Dependencies are locked in `Cargo.lock`. From the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
cargo test --workspace --locked
./target/debug/combraton-conformance version
./target/debug/combraton-conformance self-test
./target/debug/combraton-conformance check-fixtures
python3 scripts/release_inventory.py --verify
python3 scripts/check_operations.py
./target/debug/combraton-conformance run --participant conformance/participants/reference-provider.json
./target/debug/combraton-conformance check-mutants --participant conformance/participants/reference-provider.json
./target/debug/combraton-conformance run --participant conformance/participants/reference-provider-unix.json --out conformance/results/reference-unix
./target/debug/combraton-conformance check-mutants --participant conformance/participants/reference-provider-unix.json --out conformance/results/reference-unix
./target/debug/combraton-conformance run --participant conformance/participants/independent-python-core.json --out conformance/results/independent-python-core
python3 conformance/scripts/peer_user_check.py --require --out conformance/results/peer-user
python3 conformance/scripts/repeat_fixture.py --participant conformance/participants/reference-provider-unix.json --fixture socket.subscription-recheck-race-regression --runs 20 --out conformance/results/race-repeat/correct
python3 conformance/scripts/repeat_fixture.py --participant conformance/participants/reference-provider-unix.json --fixture socket.subscription-recheck-race-regression --mutant recheck-outside-lock --runs 20 --out conformance/results/race-repeat/recheck-outside-lock
python3 conformance/scripts/peer_user_check.py --require --mutant skip-peer-check --expect accepted --out conformance/results/peer-user-mutant
# Compatibility (Git checkout only): the accepted M5 build and fixture set
python3 conformance/scripts/build_pinned.py m5
./target/debug/combraton-conformance run --participant conformance/participants/reference-provider.json --fixtures target/pinned-m5/src/conformance/fixtures --out conformance/results/compat/m5-fixtures-reference
./target/debug/combraton-conformance run --participant conformance/participants/pinned/m5-reference-provider.json --filter compat. --out conformance/results/compat/pinned-m5-provider
```

The two peer-user commands need passwordless `sudo`, as on GitHub-hosted runners. Without `--require` they exit 77 with outcome `unsupported` on hosts that lack it.

What each command establishes:

| Command | Exit 0 means | Does not mean |
|---|---|---|
| `cargo test` | The reference provider's hand-written parser and canonical encoder agree with `conformance/vectors/encoding.json`; runner pattern matching works. Reference-only checks also pass: its capability revision follows CORE §17.1 when only an evidence source or `observed_at` changes, and it refuses constructed cursors past its head. | Portable conformance: these tests know the reference's private cursor format and store |
| `self-test` | The runner's strict parser and RFC 8785 canonicalization agree with the vectors | That the vectors are correct; see the cross-checks below |
| `check-fixtures` | Every fixture validates against `conformance/schemas/fixture.schema.json`, IDs are unique, requirement IDs exist in the matrix, and negative fixtures declare mutants | That fixtures are semantically right |
| `run` | The named participant passed every applicable fixture over its binding (stdio or Unix socket); `unsupported` and `skipped` fixtures are reported separately and not run. A result manifest with fixture digests and per-case transcripts is written to `conformance/results/<participant>/` (not committed). | Conformance for any profile without fixtures; real-adapter behavior |
| `run` on `independent-python-core.json` | The spec-only Python implementation passes every fixture applicable to what it claims: Core with grants, events, capabilities and effects; `execution/1` with its optional features and context revalidation; `evidence/1`, `context/1` with single-provider `context.claims`, `knowledge/1` and `verification/1`; `core.feature_dependencies`; the declared test controls; over stdio | That it or the reference is correct where fixtures are silent; see its divergence logs |
| `check-mutants` | Each of the reference provider's deliberately broken mutants fails every fixture that declares it, and every mutant is declared by at least one fixture. On the Unix-socket participant it also runs the third-party client mutants (`client_kills`), each at its stated step and reason. | That no other wrong implementation can pass. Mutants share the reference author's assumptions. |
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

- **Bindings.** Both the stdio and Unix-socket forms are exercised. The socket participant runs every applicable fixture after automatic authentication. Root, through `sudo`, is the only other OS user tested (decision 006).
- **Implementations.**
  - The reference provider is written by the fixture author.
  - The independent Python provider is written from the documents only, and serves stdio only.
  - Two third-party clients are written from the contracts only: an execution kernel (S-A) and an Evidence publisher (S-B). They are the only mixed-implementation compositions, both in client roles.
  - Composition fixtures otherwise run every named participant from the descriptor under test.
- **Not established:** Execution-provider interoperability between implementations, backpressure and fairness across implementations, real adapters, and memory or retrieval quality.
- **Test control.** States reached through launch configuration (restart, generation advancement, scripts, store faults) are test-environment control, not product operations.
- **Integrations.** No PIO, CBR or benchmark integration has run.
- **Remaining gaps.** The accepted limitations and deferred gaps are listed in the [release record](release/0.1/README.md).

For a change, report the command, exit status, environment, tested revision, real versus simulated dependencies, evidence location and untested limitations. Preserve the producer exit code when displaying shortened logs. Review the relevant diff against an explicit base/head.

## Standalone release evidence

Follow [standalone release gates](https://github.com/Combraton/combraton/blob/main/docs/STANDALONE-RELEASES.md). Core no-optional-service tests, protocol conformance, real-adapter integration, comparative outcomes and UI usability are separate evidence classes. The [benchmarks repository](https://github.com/Combraton/benchmarks) owns cross-product scenarios and results, not this service's normative contract. No runtime benchmark has been implemented or run.

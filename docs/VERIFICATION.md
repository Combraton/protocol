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
```

What each command establishes:

| Command | Exit 0 means | Does not mean |
|---|---|---|
| `cargo test` | The reference provider's hand-written parser and canonical encoder agree with `conformance/vectors/encoding.json`; runner pattern matching works | Any provider behavior over the wire |
| `self-test` | The runner's strict parser and RFC 8785 canonicalization agree with the vectors | That the vectors are correct; see the cross-checks below |
| `check-fixtures` | Every fixture validates against `conformance/schemas/fixture.schema.json`, IDs are unique, requirement IDs exist in the matrix, and negative fixtures declare mutants | That fixtures are semantically right |
| `run` | The named participant passed every applicable fixture over the stdio binding. A result manifest with fixture digests and per-case transcripts is written to `conformance/results/<participant>/` (not committed). | Conformance for any profile without fixtures; real-adapter behavior; the Unix-socket binding |
| `check-mutants` | Each of the reference provider's deliberately broken mutants fails every fixture that declares it, and every mutant is declared by at least one fixture | That no other wrong implementation can pass. Mutants share the reference author's assumptions. |

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

- Only the stdio form of the stream binding is exercised. Unix sockets, peer-credential authentication, grants, events and subscriptions, capability snapshots and effects are M2 work.
- The only provider tested is the reference provider written by the same session that wrote the fixtures. The spec-only independent implementation planned for M2 is the first check against shared assumptions.
- States reached through launch configuration (restart, generation advancement) are test-environment control, not product operations.
- No PIO, CBR or benchmark integration has run.

For a change, report the command, exit status, environment, tested revision, real versus simulated dependencies, evidence location and untested limitations. Preserve the producer exit code when displaying shortened logs. Review the relevant diff against an explicit base/head.

## Standalone release evidence

Follow [standalone release gates](https://github.com/Combraton/combraton/blob/main/docs/STANDALONE-RELEASES.md). Core no-optional-service tests, protocol conformance, real-adapter integration, comparative outcomes and UI usability are separate evidence classes. The [benchmarks repository](https://github.com/Combraton/benchmarks) owns cross-product scenarios and results, not this service's normative contract. No runtime benchmark has been implemented or run.

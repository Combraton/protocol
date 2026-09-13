//! Deliberate violations used to prove that negative fixtures detect wrong providers.
//! Each mutant must be failed by at least one fixture that declares it.

use std::collections::BTreeSet;

pub const ALL: &[(&str, &str)] = &[
    (
        "reexecute-duplicates",
        "skips the deduplication lookup and applies every transmission",
    ),
    (
        "ignore-command-digest",
        "returns the stored result for a bound command ID even when the intent differs",
    ),
    (
        "dedupe-by-transport-id",
        "uses the JSON-RPC request id as the deduplication key",
    ),
    (
        "ignore-generation-window",
        "treats forgotten or never-issued generations as new",
    ),
    (
        "checks-before-dedupe",
        "checks epoch and preconditions before the deduplication lookup",
    ),
    (
        "bind-on-rejection",
        "binds command identities that were rejected at precondition or epoch checks",
    ),
    (
        "lose-dedupe-on-restart",
        "keeps deduplication records only in memory",
    ),
    ("ignore-authority-epoch", "does not check authority_epoch"),
    (
        "partial-preconditions",
        "checks only the primary subject's precondition",
    ),
    (
        "accept-unknown-fields",
        "ignores unevaluated (unknown) fields",
    ),
    (
        "ignore-requires",
        "does not refuse unknown required features or extensions",
    ),
    (
        "skip-digest-verification",
        "never verifies command_digest or its algorithm",
    ),
    (
        "canonical-code-point-order",
        "sorts canonical members by code point instead of UTF-16 code units",
    ),
    (
        "canonical-ascii-escape",
        "escapes non-ASCII characters in the canonical form",
    ),
    ("unbounded-frames", "has no frame limit"),
    (
        "skip-invalid-frames",
        "answers frame-level failures but keeps the connection open",
    ),
    (
        "parse-unterminated",
        "parses an unterminated trailing frame at end of input",
    ),
    (
        "process-notifications",
        "executes requests sent as notifications",
    ),
    (
        "accept-unsupported-profile",
        "negotiates profiles declared unsupported",
    ),
    (
        "no-negotiation-gate",
        "serves operations before negotiation",
    ),
    (
        "ignore-limits",
        "does not enforce declared payload, string, array or depth limits",
    ),
    (
        "allow-renegotiation",
        "accepts a second negotiation in one session",
    ),
    (
        "ignore-profile-selection",
        "serves operations of profiles the session did not select",
    ),
    (
        "accept-duplicate-members",
        "keeps the last of duplicate JSON members",
    ),
    (
        "lax-numbers",
        "accepts fractions, exponents and integers outside the safe range",
    ),
    (
        "ignore-major-version",
        "selects a profile even when no requested major version is supported",
    ),
    (
        "ignore-negotiation-features",
        "selects a profile even when a required feature is unknown",
    ),
    (
        "ignore-preconditions",
        "applies commands without checking any precondition",
    ),
    (
        "expose-control-endpoint",
        "answers test-control method names on the product endpoint",
    ),
    (
        "close-on-invalid-request",
        "closes the connection after an invalid JSON-RPC request instead of staying open",
    ),
    (
        "strict-off-by-one-limit",
        "rejects a frame whose length equals the limit",
    ),
    (
        "route-by-operation",
        "dispatches on params.operation instead of the method name",
    ),
];

#[derive(Default)]
pub struct Mutants(BTreeSet<String>);

impl Mutants {
    pub fn parse(names: &[String]) -> Result<Self, String> {
        let mut set = BTreeSet::new();
        for name in names {
            if !ALL.iter().any(|(known, _)| known == name) {
                return Err(format!("unknown mutant {name}"));
            }
            set.insert(name.clone());
        }
        Ok(Self(set))
    }

    pub fn on(&self, name: &str) -> bool {
        debug_assert!(
            ALL.iter().any(|(known, _)| *known == name),
            "undeclared mutant {name}"
        );
        self.0.contains(name)
    }
}

//! Deliberate violations used to prove that negative fixtures detect wrong providers.
//! Each mutant must be failed by at least one fixture that declares it.

use std::collections::BTreeSet;

pub const ALL: &[(&str, &str)] = &[
    (
        "events-ignore-subject-read",
        "shows events and snapshot subjects covered by a core.events.read grant without the subject's own read authority",
    ),
    (
        "filtered-always-under-grant",
        "reports filtered true under any grant or kind filter even when nothing was hidden",
    ),
    (
        "issue-validation-before-dedupe",
        "validates grant audience and expiry before deduplication, so bound issues stop replaying",
    ),
    (
        "subscription-survives-authorization-loss",
        "keeps delivering to a subscription after its grant stops authorizing",
    ),
    (
        "grant-state-before-holder",
        "reports another principal's revoked grant as revoked instead of grant_not_found",
    ),
    (
        "expiry-boundary-inclusive",
        "keeps a grant usable at exactly expires_at",
    ),
    (
        "issue-at-now-accepted",
        "issues grants whose expires_at equals the provider clock",
    ),
    (
        "stale-parent-delegates",
        "lets revoked, expired or epoch-stale parents delegate",
    ),
    (
        "anyone-may-delegate",
        "lets any principal delegate from a parent grant",
    ),
    (
        "delegation-ignores-allowed-flag",
        "ignores a parent's delegation.allowed false",
    ),
    (
        "child-may-outlive-parent",
        "lets a delegated grant expire later than its parent or omit expiry",
    ),
    (
        "child-drops-authority-binding",
        "lets a delegated grant drop its parent's authority binding",
    ),
    ("anyone-may-revoke", "lets any principal revoke any grant"),
    (
        "authority-grant-unrestricted",
        "ignores the grant an authority principal names",
    ),
    ("claim-needs-no-right", "does not require core-test.claim"),
    (
        "applied-count-unprotected",
        "does not protect applied_count with core-test.read",
    ),
    (
        "denial-order-scope-first",
        "reports out_of_scope before right_missing",
    ),
    (
        "no-grant-issued-events",
        "records no core.grant.issued events",
    ),
    (
        "revoke-event-target-only",
        "records a revoked event only for the named grant",
    ),
    (
        "accept-cursor-from-other-stream",
        "accepts a well-formed cursor from another stream",
    ),
    (
        "closed-epoch-cursor-refused",
        "refuses a cursor into an earlier epoch past its vouched_through instead of reporting the epoch change",
    ),
    (
        "current-revealed-without-read",
        "includes current revisions in precondition_failed for subjects the principal may not read",
    ),
    (
        "re-revoke-accepted",
        "revokes an already revoked grant again",
    ),
    (
        "grant-precondition-revision-unchecked",
        "accepts issue preconditions other than revision 0 and revoke preconditions at revision 0",
    ),
    (
        "unknown-binding-scope-accepted",
        "issues grants bound to an authority scope the provider does not track",
    ),
    (
        "grant-hidden-from-issuer",
        "answers not_found when a grant's issuer reads it",
    ),
    (
        "ignore-caller-receive-limit",
        "sends responses and notifications larger than the caller's receive limit",
    ),
    (
        "notify-before-response",
        "sends notifications before the response of the command that caused them",
    ),
    (
        "subscription-ignores-kinds",
        "ignores a subscription's kinds filter",
    ),
    (
        "subscription-ignores-grant",
        "ignores the grant's visibility filter for subscriptions",
    ),
    (
        "subscribe-unauthorized",
        "lets any principal subscribe without authorization",
    ),
    (
        "events-read-any-grant",
        "lets any grant read events without core.events.read",
    ),
    (
        "snapshot-unfiltered",
        "includes snapshot subjects the principal may not read",
    ),
    (
        "capability-after-preconditions",
        "checks capabilities after epoch and preconditions",
    ),
    (
        "capability-before-authorization",
        "checks capabilities before authorization",
    ),
    (
        "capability-event-wrong-revision",
        "records capability change events with a revision different from the snapshot",
    ),
    (
        "capability-event-wrong-subject",
        "records capability change events with a subject id other than provider_id",
    ),
    (
        "claim-depends-on-writes",
        "makes authority claims depend on core-test.writes",
    ),
    (
        "duplicate-precondition-subjects-accepted",
        "accepts two preconditions naming the same subject",
    ),
    (
        "skip-authentication",
        "lets unauthenticated socket sessions negotiate and operate",
    ),
    (
        "distinguishable-auth-failure",
        "tells callers whether a credential was unknown or revoked",
    ),
    ("revoked-credential-accepted", "accepts revoked credentials"),
    (
        "echo-credential",
        "echoes the presented credential in authentication errors",
    ),
    (
        "unchecked-socket-directory",
        "listens in a socket directory with group or other permissions",
    ),
    (
        "no-cross-session-delivery",
        "delivers subscription notifications only after the subscriber's own requests",
    ),
    (
        "unknown-method-negotiation-required",
        "answers negotiation_required instead of method_not_found for unknown operations before negotiation",
    ),
    (
        "ignore-idless-garbage",
        "silently ignores id-less objects that are not notifications",
    ),
    (
        "lenient-jsonrpc-shape",
        "accepts extra request members and boolean ids",
    ),
    (
        "accept-lone-surrogates",
        "replaces unpaired surrogate escapes instead of closing the connection",
    ),
    ("accept-noncharacters", "accepts noncharacters in strings"),
    (
        "exit-nonzero-at-end-of-input",
        "exits with a nonzero status at end of input",
    ),
    (
        "frame-limit-never-raised",
        "keeps the pre-negotiation frame limit after negotiation",
    ),
    (
        "limits-off-by-one",
        "refuses values exactly at a declared limit",
    ),
    (
        "ignore-unique-items",
        "does not refuse duplicate requires entries",
    ),
    (
        "preconditions-before-epoch",
        "checks preconditions before the authority epoch",
    ),
    (
        "first-precondition-failure-only",
        "lists only the first failed precondition",
    ),
    (
        "accept-duplicate-profiles",
        "accepts the same profile listed twice in negotiation",
    ),
    (
        "failed-negotiation-blocks-retry",
        "answers already_negotiated after a refused negotiation",
    ),
    ("core-optional", "lets a caller make core optional"),
    (
        "reversed-negotiation-precedence",
        "prefers unsupported_version over unsupported_profile",
    ),
    (
        "no-primary-precondition-check",
        "accepts puts without a primary-subject precondition",
    ),
    ("requires-commands-only", "checks requires only on commands"),
    (
        "correlation-in-digest",
        "includes correlation in the command intent digest",
    ),
    (
        "retain-off-by-one",
        "retains one generation fewer than retain_generations",
    ),
    (
        "ignore-capability-loss",
        "admits commands whose capability is unsupported",
    ),
    (
        "capability-loss-blocks-replay",
        "checks capabilities before deduplication, so bound commands cannot replay after loss",
    ),
    (
        "unknown-capability-as-supported",
        "treats an unknown capability status as supported",
    ),
    (
        "capability-revision-static",
        "never raises the capability revision or records a change event",
    ),
    (
        "events-not-recorded",
        "commits commands without appending events",
    ),
    ("event-sequence-gap", "assigns event sequences with holes"),
    ("drop-caused-by", "does not copy caused_by into events"),
    (
        "cursor-skips-last",
        "returns a next_cursor one position past the last item",
    ),
    (
        "accept-foreign-cursor",
        "treats invalid cursors as the start of the stream",
    ),
    (
        "silent-retention-gap",
        "resumes after discarded events without a gap item",
    ),
    (
        "silent-epoch-change",
        "moves to a new epoch without an epoch_change item",
    ),
    (
        "events-ignore-authorization",
        "serves events without authorization or filtering",
    ),
    (
        "volatile-events",
        "loses recorded events when the provider restarts",
    ),
    (
        "subscription-misses-backlog",
        "starts subscriptions at the current end instead of the requested position",
    ),
    (
        "replay-appends-event",
        "appends a duplicate event when a command is replayed",
    ),
    ("ignore-grants", "performs no authorization at all"),
    (
        "ignore-grant-scope",
        "accepts any valid grant regardless of its rights and resources",
    ),
    ("ignore-revocation", "keeps honoring revoked grants"),
    ("ignore-expiry", "keeps honoring expired grants"),
    (
        "no-revocation-cascade",
        "revokes a grant without revoking grants delegated from it",
    ),
    (
        "allow-delegation-escalation",
        "lets a delegated grant exceed its parent",
    ),
    (
        "leak-existence",
        "answers not_found for unauthorized reads of nonexistent subjects before checking authorization",
    ),
    (
        "global-dedupe-scope",
        "shares deduplication records across principals",
    ),
    (
        "ignore-grant-epoch-binding",
        "keeps honoring grants bound to a superseded authority epoch",
    ),
    (
        "deny-replay-after-revocation",
        "authorizes before deduplication, so revoked principals cannot replay their own commands",
    ),
    (
        "accept-any-holder",
        "lets any principal act under a grant held by another",
    ),
    (
        "ignore-audience",
        "issues grants whose audience is another provider",
    ),
    (
        "grant-visible-to-all",
        "shows grant records to principals who are neither holder, issuer nor authority",
    ),
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

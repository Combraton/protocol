//! Deliberate violations used to prove that negative fixtures detect wrong providers.
//! Each mutant must be failed by at least one fixture that declares it.

use std::collections::BTreeSet;

pub const ALL: &[(&str, &str)] = &[
    (
        "chunk-limit-ignores-overhead",
        "declares a chunk limit that ignores base64 and envelope overhead",
    ),
    (
        "append-overwrites-received",
        "accepts an append at an already received offset, overwriting bytes",
    ),
    (
        "release-without-authority",
        "lets any principal release a hold it does not own",
    ),
    (
        "duplicate-chunk-accepted",
        "accepts an identical chunk resent under a new command with a stale precondition",
    ),
    ("coverage-optional", "accepts a descriptor without coverage"),
    (
        "locator-as-identity",
        "serves the bytes of the first artifact sharing the requested artifact's locator",
    ),
    (
        "seal-unverified",
        "seals without verifying the received size and digest",
    ),
    (
        "seal-twice-appends-event",
        "gives an already sealed artifact a new revision and event on a second seal",
    ),
    (
        "locator-credentials-accepted",
        "accepts a locator that carries credentials",
    ),
    (
        "producer-principal-from-payload",
        "records a producer principal named in the payload instead of the session principal",
    ),
    (
        "terminal-output-complete-accepted",
        "accepts terminal output declared as a complete tool trace",
    ),
    (
        "fetch-leaks-existence",
        "answers not_found for a nonexistent artifact before authorization",
    ),
    (
        "mismatched-reference-serves-bytes",
        "serves bytes for a reference whose digest differs from the artifact's",
    ),
    (
        "query-returns-unreadable",
        "lists artifacts the reader may not read",
    ),
    (
        "corrupted-bytes-served",
        "serves stored bytes that no longer match their digest",
    ),
    (
        "manifest-incomplete-reported-complete",
        "reports a manifest complete although a required child is not present",
    ),
    (
        "withheld-child-reported-missing",
        "evaluates children the reader may not read instead of withholding them",
    ),
    (
        "manifest-children-by-digest",
        "resolves a manifest child by digest alone when the named artifact is absent",
    ),
    (
        "purge-bypasses-holds",
        "purges an artifact although active holds remain",
    ),
    (
        "release-holds-as-authorization",
        "releases named holds without checking release authority",
    ),
    (
        "purged-before-confirmation",
        "reports purged before physical deletion is confirmed",
    ),
    (
        "loss-report-unfiltered",
        "shows proof-loss dependencies the reader may not inspect",
    ),
    (
        "purge-ignores-hold-revisions",
        "accepts a purge without revision preconditions on the holds it releases",
    ),
    (
        "notice-budget-per-subscription",
        "restarts the ending-notice budget for each subscription",
    ),
    (
        "room-wait-unbounded",
        "waits without bound for a stalled consumer to make room",
    ),
    (
        "unstarted-work-reported-preparing",
        "reports runtime preparing for queued or refused work that has not started",
    ),
    (
        "refusal-records-delivery-effect",
        "records a prompt delivery effect for an execution it refused",
    ),
    (
        "recovery-host-change-silent",
        "advances the host generation during recovery without an execution.host.changed event",
    ),
    (
        "unavailable-after-binding",
        "answers unavailable for a submit it had already committed and bound",
    ),
    (
        "internal-error-state-without-binding",
        "commits a command's state change without binding its command identity",
    ),
    (
        "correlation-dropped",
        "drops the caller's opaque correlation from the execution record",
    ),
    (
        "output-dropped-silently",
        "discards spooled output without declaring a lost range",
    ),
    (
        "lost-range-without-bytes",
        "declares a spool discard without its known byte count",
    ),
    (
        "consumer-too-slow-to-older-consumers",
        "sends consumer_too_slow to a session that did not negotiate core.events.backpressure",
    ),
    (
        "notice-waits-indefinitely",
        "waits without bound for a non-reading consumer to accept the ending notice",
    ),
    (
        "unbounded-pending-output",
        "keeps queueing output for a consumer that stopped reading",
    ),
    (
        "semantic-events-dropped-under-pressure",
        "drops notifications instead of closing a connection whose pending output is over its bound",
    ),
    (
        "cancels-on-session-close",
        "cancels running executions when one socket session closes while the provider keeps running",
    ),
    (
        "executor-ticks-only-on-requests",
        "advances the scripted executor and its timeouts only when a request arrives, never while idle",
    ),
    (
        "steer-claims-delivery",
        "reports a steering message acknowledged as soon as it is recorded, without evidence",
    ),
    (
        "steer-unsupported-accepted",
        "records steering as live when the adapter cannot steer",
    ),
    (
        "steer-ack-implies-behavior",
        "treats a steering acknowledgment as observed behavior",
    ),
    (
        "global-action-namespace",
        "answers a native action request found in any execution",
    ),
    (
        "requires-action-without-identity",
        "reports requires_action without the action ID and owner",
    ),
    (
        "actions-lost-on-restart",
        "drops pending native action requests when the provider restarts",
    ),
    (
        "stale-controller-accepted",
        "accepts mutating execution commands carrying a superseded controller epoch",
    ),
    (
        "agent-commit-as-receipt",
        "reports an agent-reported commit as the checkpoint head",
    ),
    (
        "incomplete-checkpoint-complete",
        "declares a checkpoint complete although some state was not probed",
    ),
    (
        "admits-unenforceable-ceiling",
        "admits a hard budget ceiling the adapter cannot enforce",
    ),
    (
        "refunds-on-timeout",
        "releases a budget reservation and resolves liability when the execution deadline passes",
    ),
    (
        "required-binding-admitted",
        "admits an execution whose required-before-start context binding is unsatisfied",
    ),
    (
        "digest-mismatch-satisfies",
        "treats a held packet with a different digest as satisfying a binding",
    ),
    (
        "no-late-state",
        "reports a packet delivered after its dependent boundary as delivered, not late",
    ),
    (
        "detected-offered-as-usable",
        "offers every detected installation as usable",
    ),
    (
        "discovery-unknown-as-yes",
        "reports unknown discovery facts as positive",
    ),
    (
        "fresh-labeled-resumed",
        "labels a fresh continuation as resumed",
    ),
    (
        "non-repeatable-retried",
        "retries a non-repeatable effect after an unknown attempt outcome",
    ),
    (
        "idempotent-retry-new-key",
        "retries an idempotent effect under a new idempotency key",
    ),
    (
        "read-never-retried",
        "gives up on a read effect after one unknown attempt",
    ),
    (
        "abort-marks-effect-failed",
        "marks an effect failed when a wait for it is aborted",
    ),
    (
        "capacity-ignored",
        "admits executions beyond the executor's capacity",
    ),
    (
        "inactivity-marks-exited",
        "treats a passed inactivity timeout as proof the execution exited",
    ),
    (
        "reconciliation-timeout-resolves",
        "treats a passed reconciliation timeout as proof of non-delivery",
    ),
    (
        "cancels-on-disconnect",
        "cancels running executions when a session closes",
    ),
    (
        "respawn-on-restart",
        "respawns running executions with a new prompt effect after a restart",
    ),
    (
        "feature-operations-ungated",
        "serves optional execution feature operations that were not negotiated",
    ),
    (
        "feature-fields-accepted",
        "accepts submit fields of optional features that were not negotiated",
    ),
    (
        "recovery-ignores-cancellation",
        "resumes dispatch during recovery although cancellation was requested",
    ),
    (
        "recovery-ignores-revocation",
        "resumes dispatch during recovery although the submitter's grant no longer authorizes",
    ),
    (
        "recovery-ignores-deadline",
        "resumes dispatch during recovery after the delivery or execution deadline passed",
    ),
    (
        "recovery-trusts-damaged-journal",
        "treats a missing dispatch marker as proof of non-dispatch when journal continuity is lost",
    ),
    (
        "stale-dispatcher-sends",
        "lets a dispatcher from an older host generation send after recovery",
    ),
    (
        "failed-before-delivery-reopened",
        "reopens a delivery already declared failed before delivery",
    ),
    (
        "reconciliation-keeps-ambiguous",
        "leaves the current delivery ambiguous after reconciliation resolves it",
    ),
    (
        "reconciliation-erases-ambiguity",
        "drops the earlier ambiguous determination from history when reconciliation resolves it",
    ),
    (
        "pending-forever",
        "leaves delivery pending after its evidence wait ends",
    ),
    (
        "execution-selected-implicitly",
        "selects execution/1 for a caller that did not request it",
    ),
    (
        "effect-refs-on-core-operations",
        "returns effect references for Core and core-test operations that record no effects",
    ),
    (
        "replay-effect-refs-differ",
        "returns different effect references on replay",
    ),
    (
        "effect-visible-across-principals",
        "lets any principal read an effect whose target it may not read",
    ),
    (
        "execution-dependency-unchecked",
        "selects execution/1 without the Core features it requires",
    ),
    (
        "admits-weaker-enforcement",
        "admits a restriction the adapter cannot enforce at the required level",
    ),
    (
        "unknown-predicate-supported",
        "admits a submit whose required adapter predicate is unknown or missing",
    ),
    (
        "predecessor-dropped",
        "does not record a retry's predecessor",
    ),
    (
        "effect-recorded-after-dispatch",
        "records the prompt submission effect at dispatch instead of in the submit transaction",
    ),
    (
        "cancel-reports-cancelled",
        "reports a cancellation as cancelled when it is only requested",
    ),
    (
        "ambiguous-dispatch-resent",
        "resumes dispatch after a restart although a write-ahead dispatch marker shows dispatch may have begun",
    ),
    (
        "ambiguity-overwritten",
        "skips the ambiguous delivery observation after a crash during dispatch",
    ),
    (
        "echo-always-acknowledged",
        "treats an echo as acknowledged delivery whatever the adapter says",
    ),
    (
        "bytes-written-acknowledged",
        "labels bytes written to a terminal as acknowledged delivery",
    ),
    (
        "old-attempt-finalizes",
        "lets a completion from a superseded host generation finalize the execution",
    ),
    (
        "last-completion-wins",
        "lets conflicting content under a recorded completion ID replace it",
    ),
    (
        "evaluation-from-exit",
        "derives an evaluation from exit status zero",
    ),
    (
        "timeouts-collapsed",
        "reports every timeout when any one of them passes",
    ),
    (
        "deadline-marks-effect-failed",
        "marks the delivery effect failed when the execution deadline passes",
    ),
    (
        "reconcile-resubmits",
        "creates a new delivery when asked to reconcile",
    ),
    (
        "unknown-effect-not-found",
        "answers not_found for a recorded effect whose outcome is unknown",
    ),
    (
        "clock-file-start-unchecked",
        "starts with an arbitrary instant when the clock file is malformed",
    ),
    (
        "recheck-outside-lock",
        "re-checks idle subscriptions and reads their events without holding the processing lock",
    ),
    (
        "clock-file-ignored",
        "keeps the clock file's initial instant for the whole process",
    ),
    (
        "clock-follows-backward-time",
        "lets the clock file move virtual time backward",
    ),
    (
        "clock-malformed-resets",
        "resets virtual time to the launch instant when the clock file is malformed",
    ),
    (
        "issue-binding-before-expiry",
        "checks an issue's binding scope before its expiry",
    ),
    (
        "issuing-rules-before-validity",
        "applies the issuing rules before the audience, expiry and binding-scope checks",
    ),
    (
        "idle-subscriptions-not-rechecked",
        "re-checks subscription authorization only after requests on the subscriber's own connection",
    ),
    (
        "skip-peer-check",
        "accepts Unix-socket connections from other operating-system users",
    ),
    (
        "current-epoch-always-disclosed",
        "includes current_epoch in stale_authority_epoch for principals that may not read the authority subject",
    ),
    (
        "re-revoke-after-preconditions",
        "decides re-revocation after preconditions instead of at step 6",
    ),
    (
        "revoked-before-issuer-check",
        "tells a principal that is neither issuer nor authority that a grant is revoked",
    ),
    (
        "cascade-rerevokes-descendants",
        "revokes already revoked descendants again",
    ),
    (
        "stdio-authenticate-accepted",
        "accepts core.authenticate on a stdio session",
    ),
    (
        "authorization-needs-grants-feature",
        "skips authorization when core.grants was not negotiated",
    ),
    (
        "capabilities-protected",
        "requires a grant for core.capabilities",
    ),
    (
        "grant-field-evaluated-on-unprotected",
        "evaluates a grant field on unprotected operations",
    ),
    (
        "authority-events-unrestricted-under-grant",
        "shows an authority every event even when it reads under a grant",
    ),
    (
        "authority-events-ignore-resources",
        "shows core-test.authority events to core-test.read without a covering resource",
    ),
    (
        "grant-events-visible-to-readers",
        "shows every grant event to any holder of core.events.read",
    ),
    (
        "grant-events-holder-only",
        "hides grant events from the grant's issuer",
    ),
    (
        "capability-events-ignore-resources",
        "shows capability events without a resource covering core.capabilities",
    ),
    (
        "capability-events-need-test-read",
        "hides capability events unless the grant also has core-test.read",
    ),
    (
        "filtered-ignores-snapshot",
        "does not report snapshot subjects hidden by authorization in filtered",
    ),
    (
        "filtered-counts-beyond-range",
        "reports filtered for hidden events after the range a read covered",
    ),
    (
        "cursor-stops-at-last-item",
        "leaves next_cursor at the last item when trailing hidden events were covered",
    ),
    (
        "snapshot-grant-state-only",
        "reports a grant's snapshot state as its state string instead of its record",
    ),
    (
        "subscription-reauth-epoch-only",
        "ends subscriptions only for epoch-stale grants, not revoked or expired ones",
    ),
    (
        "gap-hides-epoch-change",
        "starts a retention gap inside a closed epoch instead of reporting the epoch change first",
    ),
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

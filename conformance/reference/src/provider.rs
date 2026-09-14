//! Core command path (CORE sections 3-12) and the conformance-only core-test/1 profile.

use std::collections::{BTreeMap, HashMap};

use jsonschema::Validator;
use serde_json::{Value, json};

use crate::grants;
use crate::json::{CanonicalFlaws, canonical, sha256_digest};
use crate::mutants::Mutants;
use crate::store::Store;

pub const PROVIDER_NAME: &str = "combraton-reference-provider";
pub const AUTHORITY_KIND: &str = "core-test.authority";
pub const AUTHORITY_ID: &str = "core-test";

#[derive(Clone, Copy)]
pub struct Limits {
    pub max_frame_bytes: i64,
    pub max_payload_bytes: i64,
    pub max_string_bytes: i64,
    pub max_array_items: i64,
    pub max_depth: i64,
}

impl Limits {
    fn to_json(self) -> Value {
        json!({
            "max_frame_bytes": self.max_frame_bytes,
            "max_payload_bytes": self.max_payload_bytes,
            "max_string_bytes": self.max_string_bytes,
            "max_array_items": self.max_array_items,
            "max_depth": self.max_depth,
        })
    }
}

struct Operation {
    name: &'static str,
    profile: &'static str,
    command: bool,
}

const OPERATIONS: &[Operation] = &[
    Operation {
        name: "core.describe",
        profile: "core",
        command: false,
    },
    Operation {
        name: "core.negotiate",
        profile: "core",
        command: false,
    },
    Operation {
        name: "core.authenticate",
        profile: "core",
        command: false,
    },
    Operation {
        name: "core.grant.issue",
        profile: "core",
        command: true,
    },
    Operation {
        name: "core.grant.revoke",
        profile: "core",
        command: true,
    },
    Operation {
        name: "core.grant.get",
        profile: "core",
        command: false,
    },
    Operation {
        name: "core.capabilities",
        profile: "core",
        command: false,
    },
    Operation {
        name: "core.events.read",
        profile: "core",
        command: false,
    },
    Operation {
        name: "core.events.subscribe",
        profile: "core",
        command: false,
    },
    Operation {
        name: "core.events.unsubscribe",
        profile: "core",
        command: false,
    },
    Operation {
        name: "core-test.authority.claim",
        profile: "core-test",
        command: true,
    },
    Operation {
        name: "core-test.subject.put",
        profile: "core-test",
        command: true,
    },
    Operation {
        name: "core-test.subject.get",
        profile: "core-test",
        command: false,
    },
    Operation {
        name: "core-test.subject.applied_count",
        profile: "core-test",
        command: false,
    },
];

struct SupportedProfile {
    name: &'static str,
    majors: &'static [i64],
    features: &'static [&'static str],
    depends_on: &'static [&'static str],
}

const SUPPORTED: &[SupportedProfile] = &[
    SupportedProfile {
        name: "core",
        majors: &[1],
        features: &["core.grants", "core.events", "core.capabilities"],
        depends_on: &[],
    },
    SupportedProfile {
        name: "core-test",
        majors: &[1],
        features: &[],
        depends_on: &["core"],
    },
];
const DECLARED_UNSUPPORTED: &[&str] = &["coordination", "remote-trust"];

pub struct Reject {
    pub code: &'static str,
    pub details: Value,
}

fn reject(code: &'static str, details: Value) -> Reject {
    Reject { code, details }
}

pub fn retry_class(code: &str) -> &'static str {
    match code {
        "negotiation_required" | "profile_not_negotiated" => "after_renegotiate",
        "dedupe_history_unavailable"
        | "stale_authority_epoch"
        | "precondition_failed"
        | "internal_error" => "after_reconcile",
        "unavailable" | "overloaded" => "same_command",
        "capability_unavailable" => "after_reconcile",
        _ => "no",
    }
}

pub fn jsonrpc_code(code: &str) -> i64 {
    match code {
        "parse_error" | "invalid_utf8" => -32700,
        "frame_too_large" => -32010,
        "invalid_request" => -32600,
        "method_not_found" => -32601,
        "overloaded" => -32011,
        _ => 1,
    }
}

pub fn error_response(id: Value, code: &str, details: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": jsonrpc_code(code),
            "message": code,
            "data": {"code": code, "retry": retry_class(code), "details": details},
        }
    })
}

/// A credential accepted on shared transports (CORE section 18); only its digest is kept.
pub struct Credential {
    pub principal: String,
    pub digest: String,
    pub revoked: bool,
}

/// Serializes command and query processing across sessions of one process (CORE section 10).
static PROCESSING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Session principal and provider-local authority configuration (CORE section 15.1).
#[derive(Clone)]
pub struct Identity {
    /// Current capability predicates (CORE section 17).
    pub capabilities: Value,
    pub principal: String,
    pub provider_id: String,
    pub authorities: Vec<String>,
    pub clock: std::sync::Arc<crate::clock::Clock>,
}

pub struct Provider {
    store: Store,
    mutants: Mutants,
    identity: Identity,
    limits: Limits,
    validators: std::sync::Arc<HashMap<&'static str, Validator>>,
    requires_authentication: bool,
    authenticated: bool,
    credentials: std::sync::Arc<Vec<Credential>>,
    authentication_failures: u32,
    /// Selected profile name -> negotiated features.
    selected: Option<BTreeMap<String, Vec<String>>>,
    subscriptions: Vec<Subscription>,
    next_subscription: u64,
    /// Mutant `failed-negotiation-blocks-retry` only.
    negotiation_refused: bool,
    /// The caller's `receive_limits.max_frame_bytes` (STREAM section 1).
    caller_frame_limit: usize,
}

/// Room left for the JSON-RPC wrapper and a request id of up to 128 escaped code points.
const RESPONSE_OVERHEAD: usize = 1024;

#[derive(Clone)]
struct Subscription {
    id: String,
    position: (i64, i64),
    kinds: Option<Vec<String>>,
    grant: Option<Value>,
    grant_id: Option<String>,
}

impl Provider {
    pub fn new(
        store: Store,
        mutants: Mutants,
        identity: Identity,
        limits: Limits,
        validators: std::sync::Arc<HashMap<&'static str, Validator>>,
        requires_authentication: bool,
        credentials: std::sync::Arc<Vec<Credential>>,
    ) -> Self {
        Self {
            store,
            mutants,
            identity,
            limits,
            validators,
            requires_authentication,
            authenticated: false,
            credentials,
            authentication_failures: 0,
            selected: None,
            subscriptions: Vec::new(),
            next_subscription: 0,
            negotiation_refused: false,
            caller_frame_limit: 1_048_576,
        }
    }

    pub fn operation_names() -> impl Iterator<Item = &'static str> {
        OPERATIONS.iter().map(|op| op.name)
    }

    pub fn frame_limit(&self) -> usize {
        self.limits.max_frame_bytes as usize
    }

    pub fn negotiated(&self) -> bool {
        self.selected.is_some()
    }

    /// Handle one request. `id` is the validated JSON-RPC id.
    pub fn handle(&mut self, id: Value, method: &str, params: Value) -> Value {
        let _guard = match PROCESSING.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                // Test signal (decision 007): this request found the lock held and will wait.
                crate::barriers::signal(crate::barriers::LOCK_CONTENDED);
                PROCESSING
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
            }
        };
        let response = match self.process(&id, method, params) {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err(Reject { code, details }) => error_response(id.clone(), code, details),
        };
        if self.frame_fits(&response, 0) {
            response
        } else {
            error_response(id, "internal_error", json!({}))
        }
    }

    /// Whether `value`, plus `overhead` bytes, fits the caller's receive limit (CORE section 4.2).
    fn frame_fits(&self, value: &Value, overhead: usize) -> bool {
        self.mutants.on("ignore-caller-receive-limit")
            || crate::json::encode_frame(value).len() - 1 + overhead <= self.caller_frame_limit
    }

    fn process(&mut self, id: &Value, method: &str, params: Value) -> Result<Value, Reject> {
        let route = if self.mutants.on("route-by-operation") {
            params
                .get("operation")
                .and_then(Value::as_str)
                .unwrap_or(method)
                .to_string()
        } else {
            method.to_string()
        };
        if self.mutants.on("unknown-method-negotiation-required")
            && self.selected.is_none()
            && !OPERATIONS.iter().any(|op| op.name == route)
        {
            return Err(reject("negotiation_required", json!({})));
        }
        // Step 1: operation known; negotiated; profile selected.
        let operation = OPERATIONS
            .iter()
            .find(|op| op.name == route)
            .ok_or_else(|| reject("method_not_found", json!({"operation": route})))?;
        if self.requires_authentication
            && !self.authenticated
            && !matches!(operation.name, "core.describe" | "core.authenticate")
            && !self.mutants.on("skip-authentication")
        {
            return Err(reject("authentication_required", json!({})));
        }
        let pre_negotiation = matches!(
            operation.name,
            "core.describe" | "core.negotiate" | "core.authenticate"
        );
        if !pre_negotiation {
            match &self.selected {
                None if !self.mutants.on("no-negotiation-gate") => {
                    return Err(reject("negotiation_required", json!({})));
                }
                Some(selected)
                    if !selected.contains_key(operation.profile)
                        && !self.mutants.on("ignore-profile-selection") =>
                {
                    return Err(reject(
                        "profile_not_negotiated",
                        json!({"profile": operation.profile}),
                    ));
                }
                _ => {}
            }
        }

        // Step 2: limits, method/operation agreement, schema, envelope semantics.
        if !self.mutants.on("ignore-limits") {
            self.check_limits(&params)?;
        }
        if !self.mutants.on("route-by-operation")
            && params.get("operation").and_then(Value::as_str) != Some(method)
        {
            return Err(reject(
                "invalid_envelope",
                json!({"path": "/operation", "reason": "operation does not equal the method name"}),
            ));
        }
        self.check_schema(operation.name, &params)?;
        self.check_envelope_semantics(operation, &params)?;

        // Step 3: required features and extensions, including the feature an operation belongs to.
        if !self.mutants.on("ignore-requires") {
            if operation.command || !self.mutants.on("requires-commands-only") {
                self.check_requires(&params)?;
            }
            let feature = if operation.name.starts_with("core.grant.") {
                Some("core.grants")
            } else if operation.name.starts_with("core.events.") {
                Some("core.events")
            } else if operation.name == "core.capabilities" {
                Some("core.capabilities")
            } else {
                None
            };
            if let Some(feature) = feature {
                let negotiated = self
                    .selected
                    .as_ref()
                    .and_then(|selected| selected.get("core"))
                    .is_some_and(|features| features.iter().any(|f| f == feature));
                if !negotiated {
                    return Err(reject(
                        "unsupported_required_feature",
                        json!({"features": [feature]}),
                    ));
                }
            }
        }

        if !operation.command {
            if self.mutants.on("leak-existence")
                && matches!(
                    operation.name,
                    "core-test.subject.get" | "core-test.subject.applied_count"
                )
            {
                let subject = &params["payload"]["subject"];
                let exists = self
                    .store
                    .subject(
                        subject["kind"].as_str().unwrap_or_default(),
                        subject["id"].as_str().unwrap_or_default(),
                    )
                    .map_err(storage)?
                    .is_some();
                if !exists {
                    return Err(reject("not_found", json!({})));
                }
            }
            // Step 6 for queries.
            self.authorize(operation.name, &params)?;
            return self.query(operation.name, &params);
        }

        // Step 4: digest.
        let digest = params["command_digest"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        if !self.mutants.on("skip-digest-verification") {
            let (algorithm, _) = digest.split_once(':').unwrap_or(("", ""));
            if algorithm != "sha256" {
                return Err(reject(
                    "unsupported_digest_algorithm",
                    json!({"algorithm": algorithm, "supported": ["sha256"]}),
                ));
            }
            let expected = self.intent_digest(&params);
            if expected != digest {
                return Err(reject("digest_mismatch", json!({"expected": expected})));
            }
        }

        let scope = if self.mutants.on("global-dedupe-scope") {
            "global".to_string()
        } else {
            self.identity.principal.clone()
        };
        let key = if self.mutants.on("dedupe-by-transport-id") {
            format!("rpc:{id}")
        } else {
            params["command_id"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        };
        let generation = params["dedupe_generation"].as_i64().unwrap_or_default();

        if self.mutants.on("capability-loss-blocks-replay") {
            self.check_capabilities(operation.name)?;
        }
        if self.mutants.on("checks-before-dedupe") {
            self.check_epoch_and_preconditions(operation.name, &params)?;
        }

        if self.mutants.on("deny-replay-after-revocation") {
            self.authorize(operation.name, &params)?;
        }

        // Step 5: deduplication.
        let window = self.store.window().map_err(storage)?;
        if !self.mutants.on("ignore-generation-window") && generation > window.current {
            return Err(reject(
                "invalid_envelope",
                json!({"path": "/dedupe_generation", "reason": "generation was never issued"}),
            ));
        }
        if !self.mutants.on("reexecute-duplicates") {
            if let Some(stored) = self.store.find_command(&scope, &key).map_err(storage)? {
                if stored.digest == digest || self.mutants.on("ignore-command-digest") {
                    if self.mutants.on("replay-appends-event")
                        && let Ok(stored_value) = serde_json::from_str::<Value>(&stored.response)
                        && stored_value.get("acknowledgment").is_some()
                    {
                        let ack = &stored_value["acknowledgment"];
                        let record = json!({
                            "stream": self.store.stream_id().map_err(storage)?,
                            "origin": "command",
                            "type": "core-test.subject.changed",
                            "subject": ack["subject"],
                            "revision": ack["revision"],
                            "operation_ref": ack["operation_ref"],
                            "command_id": ack["command_id"],
                            "caused_by": [],
                            "recorded_at": self.identity.clock.now(),
                            "payload": stored_value["outcome"],
                        });
                        self.store
                            .append_standalone_event(record, false)
                            .map_err(storage)?;
                    }
                    return replay(&stored.response);
                }
                return Err(reject(
                    "idempotency_conflict",
                    json!({"command_id": params["command_id"]}),
                ));
            }
            if !self.mutants.on("ignore-generation-window") && generation < window.oldest {
                return Err(reject(
                    "dedupe_history_unavailable",
                    json!({"oldest_retained": window.oldest}),
                ));
            }
        }

        if self.mutants.on("capability-before-authorization") {
            self.check_capabilities(operation.name)?;
        }
        // Step 6: authorization.
        if !self.mutants.on("deny-replay-after-revocation") {
            self.authorize(operation.name, &params)?;
        }

        // Step 7: capabilities, then epoch and preconditions.
        if !self.mutants.on("capability-loss-blocks-replay")
            && !self.mutants.on("capability-before-authorization")
            && !self.mutants.on("capability-after-preconditions")
        {
            self.check_capabilities(operation.name)?;
        }
        if !self.mutants.on("checks-before-dedupe")
            && let Err(rejection) = self.check_epoch_and_preconditions(operation.name, &params)
        {
            if self.mutants.on("bind-on-rejection") {
                let stored =
                    json!({"error": {"code": rejection.code, "details": rejection.details}})
                        .to_string();
                self.store
                    .bind_rejection(&scope, &key, generation, &digest, &stored)
                    .map_err(storage)?;
            }
            return Err(rejection);
        }
        if operation.name == "core.grant.revoke" && self.mutants.on("re-revoke-after-preconditions")
        {
            let id = params["subject"]["id"].as_str().unwrap_or_default();
            if self
                .store
                .grant(id)
                .map_err(storage)?
                .is_some_and(|(_, grant)| grant["state"] == "revoked")
            {
                return Err(reject("permission_denied", json!({"reason": "revoked"})));
            }
        }
        if self.mutants.on("capability-after-preconditions") {
            self.check_capabilities(operation.name)?;
        }

        // Steps 8-9: commit and acknowledge.
        let subject = params["subject"].clone();
        let subject_id = subject["id"].as_str().unwrap_or_default().to_string();
        let command_id = params["command_id"].clone();
        let principal = self.identity.principal.clone();
        let revoke_ids = if operation.name == "core.grant.revoke" {
            self.revocation_set(&subject_id)?
        } else {
            Vec::new()
        };
        let operation_name = operation.name;
        let payload = params["payload"].clone();
        let stream = self.store.stream_id().map_err(storage)?;
        let recorded_at = self.identity.clock.now();
        let caused_by = if self.mutants.on("drop-caused-by") {
            json!([])
        } else {
            params
                .get("caused_by")
                .cloned()
                .unwrap_or_else(|| json!([]))
        };
        let record_events = !self.mutants.on("events-not-recorded");
        let sequence_gap = self.mutants.on("event-sequence-gap");
        let no_issued_events = self.mutants.on("no-grant-issued-events");
        let revoke_target_only = self.mutants.on("revoke-event-target-only");
        let response = self
            .store
            .commit_with(&scope, &key, generation, &digest, |tx, sequence| {
                let (revision, outcome, events) = apply_change(
                    tx,
                    operation_name,
                    &subject,
                    &payload,
                    &principal,
                    &revoke_ids,
                )?;
                if record_events {
                    for (event_type, event_subject, event_revision, event_payload) in events {
                        if (event_type == "core.grant.issued" && no_issued_events)
                            || (event_type == "core.grant.revoked"
                                && revoke_target_only
                                && event_subject != subject)
                        {
                            continue;
                        }
                        let record = json!({
                            "stream": stream,
                            "origin": "command",
                            "type": event_type,
                            "subject": event_subject,
                            "revision": event_revision,
                            "operation_ref": format!("op-{sequence}"),
                            "command_id": command_id,
                            "caused_by": caused_by,
                            "recorded_at": recorded_at,
                            "payload": event_payload,
                        });
                        crate::store::append_event(tx, record, sequence_gap)?;
                    }
                }
                Ok(json!({
                    "acknowledgment": {
                        "command_id": command_id,
                        "command_digest": digest,
                        "operation_ref": format!("op-{sequence}"),
                        "subject": subject,
                        "revision": revision,
                        "effect_refs": [],
                    },
                    "outcome": outcome,
                })
                .to_string())
            })
            .map_err(storage)?;
        let mut result: Value =
            serde_json::from_str(&response).map_err(|_| reject("internal_error", json!({})))?;
        result["replay"] = json!(false);
        Ok(result)
    }

    /// The target grant plus, unless mutated, every grant delegated from it.
    fn revocation_set(&self, target: &str) -> Result<Vec<String>, Reject> {
        let mut ids = vec![target.to_string()];
        if self.mutants.on("no-revocation-cascade") {
            return Ok(ids);
        }
        let all = self.store.all_grants().map_err(storage)?;
        let mut index = 0;
        while index < ids.len() {
            let parent = ids[index].clone();
            for (_, grant) in &all {
                if grant["parent"] == parent
                    && (grant["state"] == "active"
                        || self.mutants.on("cascade-rerevokes-descendants"))
                {
                    let id = grant["id"].as_str().unwrap_or_default().to_string();
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
            }
            index += 1;
        }
        Ok(ids)
    }

    /// CORE section 18.2. Failures are indistinguishable and never echo the credential.
    fn authenticate(&mut self, payload: &Value) -> Result<Value, Reject> {
        if !self.requires_authentication && self.mutants.on("stdio-authenticate-accepted") {
            return Ok(json!({"principal": self.identity.principal}));
        }
        if !self.requires_authentication || self.authenticated {
            return Err(reject("already_authenticated", json!({})));
        }
        let credential = payload["credential"].as_str().unwrap_or_default();
        let digest = sha256_digest(credential.as_bytes());
        let mut found: Option<&Credential> = None;
        for candidate in self.credentials.iter() {
            let equal = candidate.digest.len() == digest.len()
                && candidate
                    .digest
                    .bytes()
                    .zip(digest.bytes())
                    .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                    == 0;
            if equal {
                found = Some(candidate);
            }
        }
        match found {
            Some(entry) if !entry.revoked || self.mutants.on("revoked-credential-accepted") => {
                self.identity.principal = entry.principal.clone();
                self.authenticated = true;
                Ok(json!({"principal": entry.principal}))
            }
            other => {
                self.authentication_failures += 1;
                let mut details = json!({});
                if self.mutants.on("distinguishable-auth-failure") {
                    details =
                        json!({"reason": if other.is_some() { "revoked" } else { "unknown" }});
                }
                if self.mutants.on("echo-credential") {
                    details = json!({"credential": credential});
                }
                Err(reject("authentication_failed", details))
            }
        }
    }

    fn check_capabilities(&self, operation: &str) -> Result<(), Reject> {
        let dependent = operation == "core-test.subject.put"
            || (operation == "core-test.authority.claim"
                && self.mutants.on("claim-depends-on-writes"));
        if !dependent || self.mutants.on("ignore-capability-loss") {
            return Ok(());
        }
        let status = self
            .identity
            .capabilities
            .as_array()
            .into_iter()
            .flatten()
            .find(|predicate| predicate["name"] == "core-test.writes")
            .and_then(|predicate| predicate["status"].as_str())
            .unwrap_or("unknown");
        let admitted = status == "supported"
            || (status == "unknown" && self.mutants.on("unknown-capability-as-supported"));
        if admitted {
            Ok(())
        } else {
            Err(reject(
                "capability_unavailable",
                json!({"capability": "core-test.writes", "status": status}),
            ))
        }
    }

    fn grants_negotiated(&self) -> bool {
        self.selected
            .as_ref()
            .and_then(|selected| selected.get("core"))
            .is_some_and(|features| features.iter().any(|f| f == "core.grants"))
    }

    fn is_authority(&self) -> bool {
        self.identity.authorities.contains(&self.identity.principal)
    }

    /// A grant the session principal may act under right now (CORE section 15.5).
    fn usable_grant(&self, id: &str) -> Result<Value, Reject> {
        self.usable_grant_checked(id, true)
    }

    fn usable_grant_checked(&self, id: &str, check_holder: bool) -> Result<Value, Reject> {
        let denied = |reason: &str| reject("permission_denied", json!({"reason": reason}));
        let Some((_, grant)) = self.store.grant(id).map_err(storage)? else {
            return Err(denied("grant_not_found"));
        };
        let state_first = self.mutants.on("grant-state-before-holder");
        if state_first && grant["state"] != "active" && !self.mutants.on("ignore-revocation") {
            return Err(denied("revoked"));
        }
        if check_holder
            && grant["holder"] != self.identity.principal.as_str()
            && !self.mutants.on("accept-any-holder")
        {
            return Err(denied("grant_not_found"));
        }
        if grant["state"] != "active" && !self.mutants.on("ignore-revocation") {
            return Err(denied("revoked"));
        }
        let now = self.identity.clock.now();
        if let Some(expiry) = grant.get("expires_at").and_then(Value::as_str)
            && !self.mutants.on("ignore-expiry")
            && (expiry < now.as_str()
                || (expiry == now.as_str() && !self.mutants.on("expiry-boundary-inclusive")))
        {
            return Err(denied("expired"));
        }
        if let Some(binding) = grant.get("authority_binding")
            && !self.mutants.on("ignore-grant-epoch-binding")
        {
            let current = if binding["scope"] == AUTHORITY_ID {
                self.store
                    .revision(AUTHORITY_KIND, AUTHORITY_ID)
                    .map_err(storage)?
            } else {
                -1
            };
            if binding["epoch"].as_i64() != Some(current) {
                return Err(denied("authority_epoch_stale"));
            }
        }
        Ok(grant)
    }

    /// Mutant `subscription-reauth-epoch-only`: only the authority binding can end a grant.
    fn usable_grant_epoch_only(&self, id: &str) -> bool {
        let Ok(Some((_, grant))) = self.store.grant(id) else {
            return false;
        };
        let Some(binding) = grant.get("authority_binding") else {
            return true;
        };
        let current = self
            .store
            .revision(AUTHORITY_KIND, AUTHORITY_ID)
            .unwrap_or(-1);
        binding["epoch"].as_i64() == Some(current)
    }

    /// Issue validation (CORE section 15.3), part of step 6 so bound issues still replay.
    fn validate_issue(&self, payload: &Value) -> Result<(), Reject> {
        let invalid = |path: &str, reason: &str| {
            Err(reject(
                "invalid_envelope",
                json!({"path": path, "reason": reason}),
            ))
        };
        if payload["audience"] != self.identity.provider_id.as_str()
            && !self.mutants.on("ignore-audience")
        {
            return invalid("/payload/audience", "audience is not this provider");
        }
        let binding_check = || {
            if let Some(binding) = payload.get("authority_binding")
                && binding["scope"] != AUTHORITY_ID
                && !self.mutants.on("unknown-binding-scope-accepted")
            {
                return invalid(
                    "/payload/authority_binding/scope",
                    "not an authority scope of this provider",
                );
            }
            Ok(())
        };
        if self.mutants.on("issue-binding-before-expiry") {
            binding_check()?;
        }
        if let Some(expiry) = payload.get("expires_at").and_then(Value::as_str) {
            let now = self.identity.clock.now();
            let too_early = if self.mutants.on("issue-at-now-accepted") {
                expiry < now.as_str()
            } else {
                expiry <= now.as_str()
            };
            if too_early {
                return invalid("/payload/expires_at", "grant would already be expired");
            }
        }
        binding_check()
    }

    /// Issuing rules for `core.grant.issue` (CORE section 15.3).
    fn authorize_issuing(&self, params: &Value) -> Result<(), Reject> {
        let denied = |reason: &str| Err(reject("permission_denied", json!({"reason": reason})));
        let payload = &params["payload"];
        match payload.get("parent").and_then(Value::as_str) {
            None if self.is_authority() => Ok(()),
            None => denied("not_authority"),
            Some(parent_id) => {
                let parent = if self.mutants.on("stale-parent-delegates") {
                    match self.store.grant(parent_id).map_err(storage)? {
                        Some((_, grant)) if grant["holder"] == self.identity.principal.as_str() => {
                            grant
                        }
                        _ => return denied("grant_not_found"),
                    }
                } else {
                    self.usable_grant_checked(parent_id, !self.mutants.on("anyone-may-delegate"))?
                };
                let allowed = (parent["delegation"]["allowed"].as_bool().unwrap_or(false)
                    || self.mutants.on("delegation-ignores-allowed-flag"))
                    && parent["delegation"]["max_depth"].as_i64().unwrap_or(0) >= 1;
                let mut child = payload.clone();
                if self.mutants.on("child-may-outlive-parent") {
                    child["expires_at"] = parent.get("expires_at").cloned().unwrap_or(Value::Null);
                    if child["expires_at"].is_null() {
                        child.as_object_mut().map(|m| m.remove("expires_at"));
                    }
                }
                if self.mutants.on("child-drops-authority-binding")
                    && let Some(binding) = parent.get("authority_binding")
                {
                    child["authority_binding"] = binding.clone();
                }
                if !self.mutants.on("allow-delegation-escalation")
                    && !(allowed && grants::within_parent(&parent, &child))
                {
                    return denied("delegation_exceeded");
                }
                Ok(())
            }
        }
    }

    /// Step 6 (CORE section 15.5).
    fn authorize(&self, operation: &str, params: &Value) -> Result<(), Reject> {
        let validate_now =
            operation == "core.grant.issue" && !self.mutants.on("issue-validation-before-dedupe");
        if validate_now && !self.mutants.on("issuing-rules-before-validity") {
            self.validate_issue(&params["payload"])?;
        }
        if validate_now && self.mutants.on("issuing-rules-before-validity") {
            self.authorize_issuing(params)?;
            return self.validate_issue(&params["payload"]);
        }
        if self.mutants.on("ignore-grants")
            || (self.mutants.on("authorization-needs-grants-feature") && !self.grants_negotiated())
        {
            return Ok(());
        }
        let denied = |reason: &str| Err(reject("permission_denied", json!({"reason": reason})));
        match operation {
            "core.grant.issue" => self.authorize_issuing(params),
            "core.grant.revoke" => {
                let id = params["subject"]["id"].as_str().unwrap_or_default();
                let record = self.store.grant(id).map_err(storage)?;
                let issuer = record
                    .as_ref()
                    .is_some_and(|(_, grant)| grant["issuer"] == self.identity.principal.as_str());
                let already_revoked = record
                    .as_ref()
                    .is_some_and(|(_, grant)| grant["state"] == "revoked")
                    && !self.mutants.on("re-revoke-accepted")
                    && !self.mutants.on("re-revoke-after-preconditions");
                if already_revoked && self.mutants.on("revoked-before-issuer-check") {
                    return denied("revoked");
                }
                if !(self.is_authority() || issuer || self.mutants.on("anyone-may-revoke")) {
                    return denied("not_authority");
                }
                if already_revoked {
                    return denied("revoked");
                }
                Ok(())
            }
            _ => {
                let Some(mut needed) = grants::required_rights(operation, params) else {
                    let grant_id = params.get("grant").and_then(Value::as_str);
                    if operation == "core.capabilities"
                        && self.mutants.on("capabilities-protected")
                        && grant_id.is_none()
                        && !self.is_authority()
                    {
                        return denied("grant_required");
                    }
                    if let Some(id) = grant_id
                        && self.mutants.on("grant-field-evaluated-on-unprotected")
                    {
                        self.usable_grant(id)?;
                    }
                    return Ok(());
                };
                if (operation == "core-test.authority.claim"
                    && self.mutants.on("claim-needs-no-right"))
                    || (operation == "core-test.subject.applied_count"
                        && self.mutants.on("applied-count-unprotected"))
                {
                    needed.clear();
                }
                if needed.is_empty() {
                    return Ok(());
                }
                match params.get("grant").and_then(Value::as_str) {
                    Some(_)
                        if self.is_authority()
                            && self.mutants.on("authority-grant-unrestricted") =>
                    {
                        Ok(())
                    }
                    None if self.is_authority() => Ok(()),
                    None => denied("grant_required"),
                    Some(grant_id) => {
                        let grant = self.usable_grant(grant_id)?;
                        if self.mutants.on("ignore-grant-scope") {
                            return Ok(());
                        }
                        let scope_first = self.mutants.on("denial-order-scope-first");
                        let rights = |grant: &Value| -> Option<&'static str> {
                            needed.iter().find_map(|(right, subject)| {
                                match grants::grant_covers(grant, right, subject) {
                                    Err("right_missing") => Some("right_missing"),
                                    _ => None,
                                }
                            })
                        };
                        let scopes = |grant: &Value| -> Option<&'static str> {
                            needed.iter().find_map(|(right, subject)| {
                                match grants::grant_covers(grant, right, subject) {
                                    Err("out_of_scope") => Some("out_of_scope"),
                                    _ => None,
                                }
                            })
                        };
                        let first = if scope_first {
                            scopes(&grant).or_else(|| rights(&grant))
                        } else {
                            rights(&grant).or_else(|| scopes(&grant))
                        };
                        match first {
                            Some(reason) => denied(reason),
                            None => Ok(()),
                        }
                    }
                }
            }
        }
    }

    fn intent_digest(&self, params: &Value) -> String {
        let requires: Vec<&str> = params["requires"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let mut extensions = serde_json::Map::new();
        if let Some(all) = params.get("extensions").and_then(Value::as_object) {
            for (key, value) in all {
                if requires.contains(&key.as_str()) {
                    extensions.insert(key.clone(), value.clone());
                }
            }
        }
        let mut intent = json!({
            "operation": params["operation"],
            "subject": params["subject"],
            "preconditions": params["preconditions"],
            "requires": params["requires"],
            "payload": params["payload"],
            "extensions": extensions,
        });
        if self.mutants.on("correlation-in-digest")
            && let Some(correlation) = params.get("correlation")
        {
            intent["correlation"] = correlation.clone();
        }
        let flaws = CanonicalFlaws {
            code_point_order: self.mutants.on("canonical-code-point-order"),
            ascii_escape: self.mutants.on("canonical-ascii-escape"),
        };
        sha256_digest(&canonical(&intent, flaws))
    }

    fn check_limits(&self, params: &Value) -> Result<(), Reject> {
        fn walk(value: &Value, depth: i64, max: &mut (i64, i64, i64)) {
            if value.is_object() || value.is_array() {
                max.0 = max.0.max(depth);
            }
            match value {
                Value::Object(map) => {
                    for (key, member) in map {
                        max.1 = max.1.max(key.len() as i64);
                        walk(member, depth + 1, max);
                    }
                }
                Value::Array(items) => {
                    max.2 = max.2.max(items.len() as i64);
                    for item in items {
                        walk(item, depth + 1, max);
                    }
                }
                Value::String(text) => max.1 = max.1.max(text.len() as i64),
                _ => {}
            }
        }
        let mut max = (0, 0, 0);
        walk(params, 1, &mut max);
        let exceeded = |limit: &str, maximum: i64| {
            Err(reject(
                "limit_exceeded",
                json!({"limit": limit, "maximum": maximum}),
            ))
        };
        let over = |value: i64, limit: i64| {
            if self.mutants.on("limits-off-by-one") {
                value >= limit
            } else {
                value > limit
            }
        };
        if over(max.0, self.limits.max_depth) {
            return exceeded("max_depth", self.limits.max_depth);
        }
        if over(max.1, self.limits.max_string_bytes) {
            return exceeded("max_string_bytes", self.limits.max_string_bytes);
        }
        if over(max.2, self.limits.max_array_items) {
            return exceeded("max_array_items", self.limits.max_array_items);
        }
        if let Some(payload) = params.get("payload")
            && canonical(payload, CanonicalFlaws::default()).len() as i64
                > self.limits.max_payload_bytes
        {
            return exceeded("max_payload_bytes", self.limits.max_payload_bytes);
        }
        Ok(())
    }

    fn check_schema(&self, operation: &str, params: &Value) -> Result<(), Reject> {
        let validator = &self.validators[operation];
        let ignore_unknown = self.mutants.on("accept-unknown-fields");
        // Mutant `ignore-unique-items` validates a copy with duplicate requires entries removed.
        let mut deduplicated = params.clone();
        if self.mutants.on("ignore-unique-items")
            && let Some(requires) = deduplicated
                .get_mut("requires")
                .and_then(Value::as_array_mut)
        {
            let mut seen = Vec::new();
            requires.retain(|entry| {
                let fresh = !seen.contains(entry);
                seen.push(entry.clone());
                fresh
            });
        }
        let first = validator.iter_errors(&deduplicated).find(|error| {
            let keyword = error.kind().keyword();
            !(ignore_unknown && matches!(keyword, "unevaluatedProperties" | "additionalProperties"))
        });
        match first {
            None => Ok(()),
            Some(error) => Err(reject(
                "invalid_envelope",
                json!({"path": error.instance_path().to_string(), "reason": error.to_string()}),
            )),
        }
    }

    fn check_envelope_semantics(
        &self,
        operation: &Operation,
        params: &Value,
    ) -> Result<(), Reject> {
        let invalid = |path: &str, reason: &str| {
            Err(reject(
                "invalid_envelope",
                json!({"path": path, "reason": reason}),
            ))
        };
        if params.get("grant").is_some() && !self.mutants.on("accept-unknown-fields") {
            let negotiated = self
                .selected
                .as_ref()
                .and_then(|selected| selected.get("core"))
                .is_some_and(|features| features.iter().any(|f| f == "core.grants"));
            if !negotiated {
                return invalid("/grant", "feature core.grants was not negotiated");
            }
        }
        if operation.name == "core.grant.issue" && self.mutants.on("issue-validation-before-dedupe")
        {
            self.validate_issue(&params["payload"])?;
        }
        let extensions = params.get("extensions").and_then(Value::as_object);
        for entry in params
            .get("requires")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let name = entry.as_str().unwrap_or_default();
            if name.contains('/')
                && !self.mutants.on("ignore-requires")
                && !extensions.is_some_and(|map| map.contains_key(name))
            {
                return invalid(
                    "/requires",
                    "required extension is not present in extensions",
                );
            }
        }
        if !operation.command {
            return Ok(());
        }
        if let Some((algorithm, hex)) = params["command_digest"]
            .as_str()
            .and_then(|d| d.split_once(':'))
        {
            let expected = match algorithm {
                "sha256" => Some(64),
                "sha512" => Some(128),
                _ => None,
            };
            if !self.mutants.on("skip-digest-verification")
                && expected.is_some_and(|length| hex.len() != length)
            {
                return invalid(
                    "/command_digest",
                    "digest length does not match its algorithm",
                );
            }
        }
        let preconditions = params["preconditions"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let mut seen = Vec::new();
        for entry in &preconditions {
            if seen.contains(&entry["subject"])
                && !self.mutants.on("duplicate-precondition-subjects-accepted")
            {
                return invalid("/preconditions", "duplicate precondition subject");
            }
            seen.push(entry["subject"].clone());
        }
        // The schemas allow one precondition; its revision is fixed by the operation (CORE section 15.3).
        let revision = preconditions
            .first()
            .and_then(|entry| entry["revision"].as_i64());
        let revision_ok = match operation.name {
            "core.grant.issue" => revision == Some(0),
            "core.grant.revoke" => revision.is_some_and(|revision| revision >= 1),
            _ => true,
        };
        if !revision_ok && !self.mutants.on("grant-precondition-revision-unchecked") {
            return invalid(
                "/preconditions/0/revision",
                "issue needs revision 0 and revoke at least 1",
            );
        }
        if !seen.contains(&params["subject"]) && !self.mutants.on("no-primary-precondition-check") {
            return invalid("/preconditions", "the primary subject needs a precondition");
        }
        Ok(())
    }

    fn check_requires(&self, params: &Value) -> Result<(), Reject> {
        let mut unsupported = Vec::new();
        for entry in params
            .get("requires")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let name = entry.as_str().unwrap_or_default();
            let understood = if name.contains('/') {
                false // The reference provider understands no extensions.
            } else {
                let profile = name.split('.').next().unwrap_or_default();
                self.selected
                    .as_ref()
                    .and_then(|selected| selected.get(profile))
                    .is_some_and(|features| features.iter().any(|feature| feature == name))
            };
            if !understood {
                unsupported.push(name.to_string());
            }
        }
        if unsupported.is_empty() {
            Ok(())
        } else {
            Err(reject(
                "unsupported_required_feature",
                json!({"features": unsupported}),
            ))
        }
    }

    fn check_epoch_and_preconditions(&self, operation: &str, params: &Value) -> Result<(), Reject> {
        if self.mutants.on("preconditions-before-epoch") {
            self.check_preconditions_only(params)?;
        }
        let epoch = self
            .store
            .revision(AUTHORITY_KIND, AUTHORITY_ID)
            .map_err(storage)?;
        if operation == "core-test.subject.put" && !self.mutants.on("ignore-authority-epoch") {
            let asserted = params["authority_epoch"].as_i64().unwrap_or_default();
            if asserted < epoch {
                let authority = json!({"kind": AUTHORITY_KIND, "id": AUTHORITY_ID});
                let details = if self.may_read(&authority, params)?
                    || self.mutants.on("current-epoch-always-disclosed")
                {
                    json!({"current_epoch": epoch})
                } else {
                    json!({})
                };
                return Err(reject("stale_authority_epoch", details));
            }
            if asserted > epoch {
                return Err(reject("unknown_authority_epoch", json!({})));
            }
        }
        self.check_preconditions_only(params)
    }

    /// Whether the principal may read `subject` in this command's authorization context (CORE section 7).
    fn may_read(&self, subject: &Value, params: &Value) -> Result<bool, Reject> {
        let grant_operation = params["operation"]
            .as_str()
            .is_some_and(|name| name.starts_with("core.grant."));
        match params.get("grant").and_then(Value::as_str) {
            _ if grant_operation && self.is_authority() => Ok(true),
            _ if grant_operation => self.visible(
                subject,
                &None,
                &Some(json!({"rights": [], "resources": []})),
            ),
            Some(id) => match self.usable_grant(id) {
                Ok(grant) => self.visible(subject, &None, &Some(grant)),
                Err(_) => Ok(false),
            },
            None if self.is_authority() => Ok(true),
            None => self.visible(
                subject,
                &None,
                &Some(json!({"rights": [], "resources": []})),
            ),
        }
    }

    fn check_preconditions_only(&self, params: &Value) -> Result<(), Reject> {
        let mut failed = Vec::new();
        for entry in params["preconditions"].as_array().into_iter().flatten() {
            if self.mutants.on("first-precondition-failure-only") && !failed.is_empty() {
                break;
            }
            if self.mutants.on("ignore-preconditions") {
                break;
            }
            if self.mutants.on("partial-preconditions") && entry["subject"] != params["subject"] {
                continue;
            }
            let kind = entry["subject"]["kind"].as_str().unwrap_or_default();
            let id = entry["subject"]["id"].as_str().unwrap_or_default();
            let current = self.store.revision(kind, id).map_err(storage)?;
            let expected = entry["revision"].as_i64().unwrap_or_default();
            if current != expected {
                let mut failure = json!({"subject": entry["subject"], "expected": expected});
                if self.may_read(&entry["subject"], params)?
                    || self.mutants.on("current-revealed-without-read")
                {
                    failure["current"] = json!(current);
                }
                failed.push(failure);
            }
        }
        if failed.is_empty() {
            Ok(())
        } else {
            Err(reject("precondition_failed", json!({"failed": failed})))
        }
    }

    fn query(&mut self, operation: &str, params: &Value) -> Result<Value, Reject> {
        match operation {
            "core.describe" => self.describe(),
            "core.negotiate" => self.negotiate(&params["payload"]),
            "core.authenticate" => self.authenticate(&params["payload"]),
            "core.events.read" => self.events_read(params),
            "core.capabilities" => Ok(json!({
                "revision": self.store.capability_revision().map_err(storage)?,
                "predicates": self.identity.capabilities,
            })),
            "core.events.subscribe" => self.events_subscribe(params),
            "core.events.unsubscribe" => {
                let id = params["payload"]["subscription"]
                    .as_str()
                    .unwrap_or_default();
                let before = self.subscriptions.len();
                self.subscriptions.retain(|sub| sub.id != id);
                if self.subscriptions.len() == before {
                    Err(reject("not_found", json!({})))
                } else {
                    Ok(json!({}))
                }
            }
            "core.grant.get" => {
                let id = params["payload"]["grant"].as_str().unwrap_or_default();
                let found = self.store.grant(id).map_err(storage)?;
                match found {
                    Some((revision, grant))
                        if self.is_authority()
                            || grant["holder"] == self.identity.principal.as_str()
                            || (grant["issuer"] == self.identity.principal.as_str()
                                && !self.mutants.on("grant-hidden-from-issuer"))
                            || self.mutants.on("grant-visible-to-all") =>
                    {
                        Ok(json!({"grant": grant, "revision": revision}))
                    }
                    _ => Err(reject("not_found", json!({}))),
                }
            }
            "core-test.subject.get" => {
                let subject = &params["payload"]["subject"];
                let found = self
                    .store
                    .subject(
                        subject["kind"].as_str().unwrap_or_default(),
                        subject["id"].as_str().unwrap_or_default(),
                    )
                    .map_err(storage)?;
                match found {
                    Some((revision, value, _)) => {
                        Ok(json!({"subject": subject, "revision": revision, "value": value}))
                    }
                    None => Err(reject("not_found", json!({}))),
                }
            }
            "core-test.subject.applied_count" => {
                let subject = &params["payload"]["subject"];
                let found = self
                    .store
                    .subject(
                        subject["kind"].as_str().unwrap_or_default(),
                        subject["id"].as_str().unwrap_or_default(),
                    )
                    .map_err(storage)?;
                Ok(
                    json!({"subject": subject, "applied_count": found.map_or(0, |(_, _, count)| count)}),
                )
            }
            _ => Err(reject("method_not_found", json!({"operation": operation}))),
        }
    }

    fn window_json(&self) -> Result<Value, Reject> {
        let window = self.store.window().map_err(storage)?;
        Ok(json!({"oldest_retained": window.oldest, "current": window.current}))
    }

    fn describe(&self) -> Result<Value, Reject> {
        let profiles: Vec<Value> = SUPPORTED
            .iter()
            .map(|p| json!({"name": p.name, "majors": p.majors, "features": p.features, "depends_on": p.depends_on}))
            .collect();
        let unsupported: Vec<Value> = DECLARED_UNSUPPORTED
            .iter()
            .map(|name| json!({"name": name, "reason": "not_in_release"}))
            .collect();
        Ok(json!({
            "provider": {"name": PROVIDER_NAME, "version": env!("CARGO_PKG_VERSION")},
            "profiles": profiles,
            "unsupported_profiles": unsupported,
            "limits": self.limits.to_json(),
            "dedupe_window": self.window_json()?,
            "unknown_extensions": "drop",
        }))
    }

    fn negotiate(&mut self, payload: &Value) -> Result<Value, Reject> {
        if (self.selected.is_some() || self.negotiation_refused)
            && !self.mutants.on("allow-renegotiation")
        {
            return Err(reject("already_negotiated", json!({})));
        }
        let mut seen_names = Vec::new();
        for (index, request) in payload["profiles"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            if seen_names.contains(&request["name"])
                && !self.mutants.on("accept-duplicate-profiles")
            {
                return Err(reject(
                    "invalid_envelope",
                    json!({"path": format!("/payload/profiles/{index}/name"), "reason": "profile listed twice"}),
                ));
            }
            seen_names.push(request["name"].clone());
        }
        let result = self.negotiate_profiles(payload);
        if result.is_err() && self.mutants.on("failed-negotiation-blocks-retry") {
            self.negotiation_refused = true;
        }
        result
    }

    fn negotiate_profiles(&mut self, payload: &Value) -> Result<Value, Reject> {
        let accept_unsupported = self.mutants.on("accept-unsupported-profile");
        let ignore_major = self.mutants.on("ignore-major-version");
        let ignore_features = self.mutants.on("ignore-negotiation-features");
        let mut requests: Vec<Value> = payload["profiles"].as_array().cloned().unwrap_or_default();
        if !requests.iter().any(|r| r["name"] == "core") {
            requests.insert(0, json!({"name": "core", "majors": [1], "required": true, "required_features": [], "optional_features": []}));
        }
        let mut selected: BTreeMap<String, (i64, Vec<String>, bool)> = BTreeMap::new();
        let mut unselected = Vec::new();
        let mut unsatisfied = Vec::new();
        for request in &requests {
            let name = request["name"].as_str().unwrap_or_default().to_string();
            let required = request["required"].as_bool().unwrap_or(false)
                || (name == "core" && !self.mutants.on("core-optional"));
            let mut fail = |reason: &str, feature: Option<&str>| {
                let mut item = json!({"profile": name, "reason": reason});
                if let Some(feature) = feature {
                    item["feature"] = json!(feature);
                }
                if required {
                    unsatisfied.push(item)
                } else {
                    unselected.push(item)
                }
            };
            let supported = SUPPORTED.iter().find(|p| p.name == name);
            let (majors, features): (&[i64], &[&str]) = match supported {
                Some(p) => (p.majors, p.features),
                None if accept_unsupported && DECLARED_UNSUPPORTED.contains(&name.as_str()) => {
                    (&[1], &[])
                }
                None if DECLARED_UNSUPPORTED.contains(&name.as_str()) => {
                    fail("declared_unsupported", None);
                    continue;
                }
                None => {
                    fail("unknown_profile", None);
                    continue;
                }
            };
            let common = request["majors"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_i64)
                .filter(|m| majors.contains(m))
                .max()
                .or(if ignore_major {
                    majors.first().copied()
                } else {
                    None
                });
            let Some(major) = common else {
                fail("no_common_major", None);
                continue;
            };
            let missing: Vec<String> = request["required_features"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter(|f| !features.contains(f))
                .map(String::from)
                .collect();
            if !missing.is_empty() && !ignore_features {
                for feature in &missing {
                    fail("unknown_feature", Some(feature));
                }
                continue;
            }
            let mut chosen: Vec<String> = request["required_features"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect();
            for feature in request["optional_features"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                if features.contains(&feature) {
                    chosen.push(feature.to_string());
                } else {
                    unselected.push(
                        json!({"profile": name, "feature": feature, "reason": "unknown_feature"}),
                    );
                }
            }
            selected.insert(name, (major, chosen, required));
        }
        // Dependencies.
        let names: Vec<String> = selected.keys().cloned().collect();
        for name in names {
            let depends = SUPPORTED
                .iter()
                .find(|p| p.name == name)
                .map_or(&[][..], |p| p.depends_on);
            if depends
                .iter()
                .any(|dependency| !selected.contains_key(*dependency))
            {
                let (_, _, required) = selected.remove(&name).unwrap();
                let item = json!({"profile": name, "reason": "dependency_not_selected"});
                if required {
                    unsatisfied.push(item)
                } else {
                    unselected.push(item)
                }
            }
        }
        if !unsatisfied.is_empty() {
            let reasons: Vec<&str> = unsatisfied
                .iter()
                .filter_map(|item| item["reason"].as_str())
                .collect();
            let reversed = self.mutants.on("reversed-negotiation-precedence");
            let code = if reversed && reasons.contains(&"no_common_major") {
                "unsupported_version"
            } else if reasons.iter().any(|r| {
                matches!(
                    *r,
                    "unknown_profile" | "declared_unsupported" | "dependency_not_selected"
                )
            }) {
                "unsupported_profile"
            } else if reasons.contains(&"no_common_major") {
                "unsupported_version"
            } else {
                "unsupported_required_feature"
            };
            return Err(reject(code, json!({"unsatisfied": unsatisfied})));
        }
        let selected_json: Vec<Value> = selected.iter().map(|(name, (major, features, _))| json!({"name": name, "major": major, "features": features})).collect();
        self.selected = Some(
            selected
                .into_iter()
                .map(|(name, (_, features, _))| (name, features))
                .collect(),
        );
        self.caller_frame_limit = payload["receive_limits"]["max_frame_bytes"]
            .as_u64()
            .map_or(1_048_576, |limit| limit as usize);
        Ok(json!({
            "selected": selected_json,
            "unselected": unselected,
            "limits": self.limits.to_json(),
            "dedupe_window": self.window_json()?,
        }))
    }
}

impl Provider {
    /// Who may read events and which subjects they see (CORE section 16.6).
    fn event_access(&self, params: &Value) -> Result<Option<Value>, Reject> {
        if self.mutants.on("events-ignore-authorization") {
            return Ok(None);
        }
        match params.get("grant").and_then(Value::as_str) {
            Some(id) => {
                let grant = self.usable_grant(id)?;
                if self.is_authority()
                    && self.mutants.on("authority-events-unrestricted-under-grant")
                {
                    return Ok(None);
                }
                let can_read = grant["rights"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|right| right == "core.events.read");
                if can_read || self.mutants.on("events-read-any-grant") {
                    Ok(Some(grant))
                } else {
                    Err(reject(
                        "permission_denied",
                        json!({"reason": "right_missing"}),
                    ))
                }
            }
            None if self.is_authority() => Ok(None),
            None => Err(reject(
                "permission_denied",
                json!({"reason": "grant_required"}),
            )),
        }
    }

    fn head(&self) -> Result<(i64, i64), Reject> {
        let epoch = self.store.stream_epoch().map_err(storage)?;
        Ok((epoch, self.store.assigned_through(epoch).map_err(storage)?))
    }

    fn start_position(&self, payload: &Value) -> Result<(i64, i64), Reject> {
        match (
            payload.get("cursor").and_then(Value::as_str),
            payload.get("from").and_then(Value::as_str),
        ) {
            (Some(cursor), _) => match self.parse_cursor(cursor) {
                Ok(position) => Ok(position),
                Err(_) if self.mutants.on("accept-foreign-cursor") => Ok((1, 0)),
                Err(reason) => Err(reject("invalid_cursor", json!({"reason": reason}))),
            },
            (None, Some("now")) => self.head(),
            _ => Ok((1, 0)),
        }
    }

    fn parse_cursor(&self, cursor: &str) -> Result<(i64, i64), &'static str> {
        let parts: Vec<&str> = cursor.split(':').collect();
        if parts.len() != 4 || parts[0] != "ev1" {
            return Err("malformed");
        }
        let stream = self.store.stream_id().map_err(|_| "unavailable")?;
        if parts[1] != stream && !self.mutants.on("accept-cursor-from-other-stream") {
            return Err("foreign_stream");
        }
        let epoch: i64 = parts[2].parse().map_err(|_| "malformed")?;
        let sequence: i64 = parts[3].parse().map_err(|_| "malformed")?;
        let (current, head) = self.head().map_err(|_| "unavailable")?;
        let limit = if epoch == current {
            head
        } else if (1..current).contains(&epoch) {
            // A closed epoch: a position past vouched_through is answered with the epoch change.
            if self.mutants.on("closed-epoch-cursor-refused") {
                self.store
                    .vouched_through(epoch)
                    .map_err(|_| "unavailable")?
                    .unwrap_or(0)
            } else {
                i64::MAX
            }
        } else {
            return Err("beyond_end");
        };
        if !(0..=limit).contains(&sequence) {
            return Err("beyond_end");
        }
        Ok((epoch, sequence))
    }

    fn cursor(&self, position: (i64, i64), advanced: bool) -> Result<String, Reject> {
        let stream = self.store.stream_id().map_err(storage)?;
        let skew = i64::from(advanced && self.mutants.on("cursor-skips-last"));
        Ok(format!("ev1:{stream}:{}:{}", position.0, position.1 + skew))
    }

    /// Visibility of a subject in events and snapshots (CORE section 16.6).
    fn visible(
        &self,
        subject: &Value,
        kinds: &Option<Vec<String>>,
        grant: &Option<Value>,
    ) -> Result<bool, Reject> {
        let kind_ok = kinds
            .as_ref()
            .is_none_or(|kinds| kinds.iter().any(|kind| subject["kind"] == kind.as_str()));
        let Some(grant) = grant else {
            return Ok(kind_ok);
        };
        let covers = grant["resources"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|resource| grants::resource_covers_subject(resource, subject));
        if self.mutants.on("events-ignore-subject-read") {
            return Ok(kind_ok && covers);
        }
        let kind = subject["kind"].as_str().unwrap_or_default();
        let test_read = grant["rights"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|right| right == "core-test.read");
        let readable = if kind.starts_with("core-test.") {
            let covered = covers
                || (kind == AUTHORITY_KIND && self.mutants.on("authority-events-ignore-resources"));
            covered && test_read
        } else if kind == "core.grant" {
            let id = subject["id"].as_str().unwrap_or_default();
            self.mutants.on("grant-events-visible-to-readers")
                || self
                    .store
                    .grant(id)
                    .map_err(storage)?
                    .is_some_and(|(_, record)| {
                        record["holder"] == self.identity.principal.as_str()
                            || (record["issuer"] == self.identity.principal.as_str()
                                && !self.mutants.on("grant-events-holder-only"))
                    })
        } else if kind == "core.capabilities" {
            (covers || self.mutants.on("capability-events-ignore-resources"))
                && (test_read || !self.mutants.on("capability-events-need-test-read"))
        } else {
            false
        };
        Ok(kind_ok && readable)
    }

    /// Visible snapshot subjects and whether any subject was hidden.
    fn snapshot_subjects(
        &self,
        kinds: &Option<Vec<String>>,
        grant: &Option<Value>,
    ) -> Result<(Vec<Value>, bool), Reject> {
        let grant = if self.mutants.on("snapshot-unfiltered") {
            &None
        } else {
            grant
        };
        let mut subjects = Vec::new();
        let mut hidden = false;
        let mut consider =
            |this: &Self, subject: Value, revision: i64, state: Value| -> Result<(), Reject> {
                if this.visible(&subject, kinds, grant)? {
                    subjects
                        .push(json!({"subject": subject, "revision": revision, "state": state}));
                } else {
                    hidden = true;
                }
                Ok(())
            };
        for (kind, id, revision, value) in self.store.all_subjects().map_err(storage)? {
            let state = if kind == AUTHORITY_KIND {
                json!({"epoch": revision})
            } else {
                json!({"value": value})
            };
            consider(self, json!({"kind": kind, "id": id}), revision, state)?;
        }
        consider(
            self,
            json!({"kind": "core.capabilities", "id": self.identity.provider_id}),
            self.store.capability_revision().map_err(storage)?,
            json!({"predicates": self.identity.capabilities}),
        )?;
        for (revision, record) in self.store.all_grants().map_err(storage)? {
            consider(
                self,
                json!({"kind": "core.grant", "id": record["id"]}),
                revision,
                if self.mutants.on("snapshot-grant-state-only") {
                    json!({"state": record["state"]})
                } else {
                    json!({"grant": record})
                },
            )?;
        }
        Ok((subjects, hidden))
    }

    /// Ordered stream items after `start` (CORE section 16.4).
    fn collect_items(
        &self,
        start: (i64, i64),
        limit: usize,
        kinds: &Option<Vec<String>>,
        grant: &Option<Value>,
    ) -> Result<Collected, Reject> {
        let current = self.store.stream_epoch().map_err(storage)?;
        let discarded = self.store.discarded_through().map_err(storage)?;
        let mut filtered =
            self.mutants.on("filtered-always-under-grant") && (kinds.is_some() || grant.is_some());
        let mut position = start;
        let mut items = Vec::new();
        // A cursor at the vouched end of a closed epoch hears about the epoch change before any gap.
        while discarded > position
            && position.0 < current
            && !self.mutants.on("gap-hides-epoch-change")
        {
            let vouched = self
                .store
                .vouched_through(position.0)
                .map_err(storage)?
                .unwrap_or(0);
            if position.1 < vouched {
                break;
            }
            if !self.mutants.on("silent-epoch-change") {
                items.push(json!({"epoch_change": {
                    "from_epoch": position.0,
                    "to_epoch": position.0 + 1,
                    "vouched_through": vouched,
                }}));
            }
            position = (position.0 + 1, 0);
        }
        if discarded > position && !self.mutants.on("silent-retention-gap") {
            let head = self.head()?;
            let as_of = json!({"epoch": head.0, "sequence": head.1});
            let (subjects, hidden) = self.snapshot_subjects(kinds, grant)?;
            filtered |= hidden && !self.mutants.on("filtered-ignores-snapshot");
            items.push(json!({"gap": {
                "kind": "retention",
                "from": {"epoch": position.0.max(1), "sequence": position.1 + 1},
                "to": as_of,
                "snapshot": {"as_of": as_of, "subjects": subjects},
            }}));
            return Ok((items, head, filtered));
        }
        let mut after_last_item = position;
        while items.len() < limit {
            if position.0 < current {
                let vouched = self
                    .store
                    .vouched_through(position.0)
                    .map_err(storage)?
                    .unwrap_or(0);
                match self
                    .store
                    .next_event(position.0, position.1)
                    .map_err(storage)?
                {
                    Some((sequence, event)) if sequence <= vouched => {
                        position = (position.0, sequence);
                        if self.visible(&event["subject"], kinds, grant)? {
                            items.push(json!({"event": event}));
                            after_last_item = position;
                        } else {
                            filtered = true;
                        }
                    }
                    _ => {
                        if !self.mutants.on("silent-epoch-change") {
                            items.push(json!({"epoch_change": {
                                "from_epoch": position.0,
                                "to_epoch": position.0 + 1,
                                "vouched_through": vouched,
                            }}));
                        }
                        position = (position.0 + 1, 0);
                        after_last_item = position;
                    }
                }
            } else {
                match self
                    .store
                    .next_event(current, position.1)
                    .map_err(storage)?
                {
                    Some((sequence, event)) => {
                        position = (current, sequence);
                        if self.visible(&event["subject"], kinds, grant)? {
                            items.push(json!({"event": event}));
                            after_last_item = position;
                        } else {
                            filtered = true;
                        }
                    }
                    None => break,
                }
            }
        }
        if items.len() == limit && self.mutants.on("filtered-counts-beyond-range") {
            filtered |= self.collect_items(position, usize::MAX, kinds, grant)?.2;
        }
        if self.mutants.on("cursor-stops-at-last-item") && !items.is_empty() {
            position = after_last_item;
        }
        Ok((items, position, filtered))
    }

    fn kinds(payload: &Value) -> Option<Vec<String>> {
        payload.get("kinds").and_then(Value::as_array).map(|kinds| {
            kinds
                .iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
    }

    fn events_read(&self, params: &Value) -> Result<Value, Reject> {
        let grant = self.event_access(params)?;
        let payload = &params["payload"];
        let start = self.start_position(payload)?;
        let mut take = payload["limit"].as_u64().unwrap_or(100) as usize;
        let kinds = Self::kinds(payload);
        loop {
            let (items, position, filtered) = self.collect_items(start, take, &kinds, &grant)?;
            let count = items.len();
            let result = json!({
                "stream": {"id": self.store.stream_id().map_err(storage)?, "epoch": self.store.stream_epoch().map_err(storage)?},
                "next_cursor": self.cursor(position, count > 0)?,
                "items": items,
                "filtered": filtered,
            });
            if self.frame_fits(&result, RESPONSE_OVERHEAD) {
                return Ok(result);
            }
            if count <= 1 {
                return Err(reject("internal_error", json!({})));
            }
            take = count / 2;
        }
    }

    fn events_subscribe(&mut self, params: &Value) -> Result<Value, Reject> {
        let grant = if self.mutants.on("subscribe-unauthorized") {
            None
        } else {
            self.event_access(params)?
        };
        let grant_id = params
            .get("grant")
            .and_then(Value::as_str)
            .map(String::from);
        let payload = &params["payload"];
        let position = if self.mutants.on("subscription-misses-backlog") {
            self.start_position(payload)?;
            self.head()?
        } else {
            self.start_position(payload)?
        };
        self.next_subscription += 1;
        let id = format!("sub-{}", self.next_subscription);
        self.subscriptions.push(Subscription {
            id: id.clone(),
            position,
            kinds: if self.mutants.on("subscription-ignores-kinds") {
                None
            } else {
                Self::kinds(payload)
            },
            grant: if self.mutants.on("subscription-ignores-grant") {
                None
            } else {
                grant.clone()
            },
            grant_id: if grant.is_some() { grant_id } else { None },
        });
        Ok(json!({
            "subscription": id,
            "stream": {"id": self.store.stream_id().map_err(storage)?, "epoch": self.store.stream_epoch().map_err(storage)?},
        }))
    }

    /// Notification frames owed to this session's subscriptions, sent after each response and,
    /// on shared transports, whenever the connection is idle (`idle`).
    ///
    /// The re-authorization and the event read run under the processing lock, so no command
    /// can commit between them: an item committed after a revocation is never delivered.
    pub fn drain_notifications(&mut self, idle: bool) -> Vec<Value> {
        let _guard = (!self.mutants.on("recheck-outside-lock")).then(|| {
            PROCESSING
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        });
        let mut frames = Vec::new();
        let mut subscriptions = std::mem::take(&mut self.subscriptions);
        subscriptions.retain(|subscription| {
            let Some(grant_id) = &subscription.grant_id else { return true };
            if idle && self.mutants.on("idle-subscriptions-not-rechecked") {
                return true;
            }
            let still_authorized = if self.mutants.on("subscription-reauth-epoch-only") {
                self.usable_grant_epoch_only(grant_id)
            } else {
                self.usable_grant(grant_id).is_ok()
            };
            if self.mutants.on("subscription-survives-authorization-loss") || still_authorized {
                // Test barrier (decision 007): between re-authorization and reading events.
                crate::barriers::pause(crate::barriers::RECHECK_AFTER_AUTHORIZATION);
                return true;
            }
            frames.push(json!({
                "jsonrpc": "2.0",
                "method": "core.events.notify",
                "params": {"subscription": subscription.id, "items": [], "next_cursor": self.cursor(subscription.position, false).unwrap_or_default(), "ended": {"reason": "authorization_lost"}},
            }));
            false
        });
        let mut too_large = Vec::new();
        for subscription in &mut subscriptions {
            let mut take = 100;
            while let Ok((items, position, _)) = self.collect_items(
                subscription.position,
                take,
                &subscription.kinds,
                &subscription.grant,
            ) {
                let count = items.len();
                if count == 0 {
                    subscription.position = position;
                    break;
                }
                let Ok(next_cursor) = self.cursor(position, true) else {
                    break;
                };
                let frame = json!({
                    "jsonrpc": "2.0",
                    "method": "core.events.notify",
                    "params": {"subscription": subscription.id, "items": items, "next_cursor": next_cursor},
                });
                if !self.frame_fits(&frame, 0) {
                    if count > 1 {
                        take = count / 2;
                        continue;
                    }
                    frames.push(json!({
                        "jsonrpc": "2.0",
                        "method": "core.events.notify",
                        "params": {"subscription": subscription.id, "items": [], "next_cursor": self.cursor(subscription.position, false).unwrap_or_default(), "ended": {"reason": "item_too_large"}},
                    }));
                    too_large.push(subscription.id.clone());
                    break;
                }
                subscription.position = position;
                frames.push(frame);
                take = 100;
            }
        }
        subscriptions.retain(|subscription| !too_large.contains(&subscription.id));
        self.subscriptions = subscriptions;
        frames
    }
}

fn replay(stored: &str) -> Result<Value, Reject> {
    let mut value: Value =
        serde_json::from_str(stored).map_err(|_| reject("internal_error", json!({})))?;
    if let Some(error) = value.get("error") {
        let code = error["code"].as_str().unwrap_or("internal_error");
        let code: &'static str = Box::leak(code.to_string().into_boxed_str());
        return Err(reject(code, error["details"].clone()));
    }
    value["replay"] = json!(true);
    Ok(value)
}

fn storage(error: rusqlite::Error) -> Reject {
    eprintln!("storage error: {error}");
    reject("unavailable", json!({}))
}

/// Stream items read, the position after them, and whether filtering hid anything.
type Collected = (Vec<Value>, (i64, i64), bool);

/// One event produced by a command: (type, subject, revision, payload).
type EventDraft = (&'static str, Value, i64, Value);

/// Apply one accepted command inside the owner transaction; returns (revision, outcome, events).
fn apply_change(
    tx: &rusqlite::Transaction,
    operation: &str,
    subject: &Value,
    payload: &Value,
    principal: &str,
    revoke_ids: &[String],
) -> rusqlite::Result<(i64, Value, Vec<EventDraft>)> {
    use rusqlite::{OptionalExtension, params};
    let kind = subject["kind"].as_str().unwrap_or_default();
    let id = subject["id"].as_str().unwrap_or_default();
    match operation {
        "core.grant.issue" => {
            let mut record = payload.clone();
            record["id"] = json!(id);
            record["issuer"] = json!(principal);
            record["state"] = json!("active");
            tx.execute(
                "INSERT INTO grants VALUES (?1, 1, ?2)",
                params![id, record.to_string()],
            )?;
            let events = vec![(
                "core.grant.issued",
                subject.clone(),
                1,
                json!({"grant": record}),
            )];
            Ok((1, json!({"grant": record}), events))
        }
        "core.grant.revoke" => {
            let mut target_revision = 0;
            let mut events = Vec::new();
            for grant_id in revoke_ids {
                let row: Option<(i64, String)> = tx
                    .query_row(
                        "SELECT revision, record FROM grants WHERE id=?1",
                        [grant_id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                let Some((revision, record)) = row else {
                    continue;
                };
                let mut record: Value = serde_json::from_str(&record).unwrap_or(Value::Null);
                record["state"] = json!("revoked");
                tx.execute(
                    "UPDATE grants SET revision=?1, record=?2 WHERE id=?3",
                    params![revision + 1, record.to_string(), grant_id],
                )?;
                if grant_id == id {
                    target_revision = revision + 1;
                }
                events.push((
                    "core.grant.revoked",
                    json!({"kind": "core.grant", "id": grant_id}),
                    revision + 1,
                    json!({"state": "revoked"}),
                ));
            }
            Ok((target_revision, json!({"revoked": revoke_ids}), events))
        }
        _ => {
            let value = payload["value"].as_str().unwrap_or_default();
            let revision: i64 = tx
                .query_row(
                    "SELECT revision FROM subjects WHERE kind=?1 AND id=?2",
                    params![kind, id],
                    |r| r.get(0),
                )
                .optional()?
                .unwrap_or(0)
                + 1;
            tx.execute(
                "INSERT INTO subjects VALUES (?1, ?2, ?3, ?4, 1)
                 ON CONFLICT (kind, id) DO UPDATE SET revision=excluded.revision, value=excluded.value,
                 applied_count=applied_count+1",
                params![kind, id, revision, value],
            )?;
            let (outcome, event_type) = if operation == "core-test.authority.claim" {
                (json!({"epoch": revision}), "core-test.authority.claimed")
            } else {
                (json!({"value": value}), "core-test.subject.changed")
            };
            let events = vec![(event_type, subject.clone(), revision, outcome.clone())];
            Ok((revision, outcome, events))
        }
    }
}

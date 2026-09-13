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
        features: &["core.grants"],
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

/// Session principal and provider-local authority configuration (CORE section 15.1).
pub struct Identity {
    pub principal: String,
    pub provider_id: String,
    pub authorities: Vec<String>,
    pub fixed_clock: Option<String>,
}

pub struct Provider {
    store: Store,
    mutants: Mutants,
    identity: Identity,
    limits: Limits,
    validators: HashMap<&'static str, Validator>,
    /// Selected profile name -> negotiated features.
    selected: Option<BTreeMap<String, Vec<String>>>,
}

impl Provider {
    pub fn new(
        store: Store,
        mutants: Mutants,
        identity: Identity,
        limits: Limits,
        validators: HashMap<&'static str, Validator>,
    ) -> Self {
        Self {
            store,
            mutants,
            identity,
            limits,
            validators,
            selected: None,
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
        match self.process(&id, method, params) {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err(Reject { code, details }) => error_response(id, code, details),
        }
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
        if self.mutants.on("expose-control-endpoint") && route.starts_with("control.") {
            return Ok(json!({}));
        }
        // Step 1: operation known; negotiated; profile selected.
        let operation = OPERATIONS
            .iter()
            .find(|op| op.name == route)
            .ok_or_else(|| reject("method_not_found", json!({"operation": route})))?;
        let pre_negotiation = matches!(operation.name, "core.describe" | "core.negotiate");
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

        // Step 3: required features and extensions.
        if !self.mutants.on("ignore-requires") {
            self.check_requires(&params)?;
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

        // Step 6: authorization.
        if !self.mutants.on("deny-replay-after-revocation") {
            self.authorize(operation.name, &params)?;
        }

        // Step 7: epoch and preconditions.
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
        let response = self
            .store
            .commit_with(&scope, &key, generation, &digest, |tx, sequence| {
                let (revision, outcome) = apply_change(
                    tx,
                    operation_name,
                    &subject,
                    &payload,
                    &principal,
                    &revoke_ids,
                )?;
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
                if grant["parent"] == parent && grant["state"] == "active" {
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

    fn is_authority(&self) -> bool {
        self.identity.authorities.contains(&self.identity.principal)
    }

    /// A grant the session principal may act under right now (CORE section 15.5).
    fn usable_grant(&self, id: &str) -> Result<Value, Reject> {
        let denied = |reason: &str| reject("permission_denied", json!({"reason": reason}));
        let Some((_, grant)) = self.store.grant(id).map_err(storage)? else {
            return Err(denied("grant_not_found"));
        };
        if grant["holder"] != self.identity.principal.as_str()
            && !self.mutants.on("accept-any-holder")
        {
            return Err(denied("grant_not_found"));
        }
        if grant["state"] != "active" && !self.mutants.on("ignore-revocation") {
            return Err(denied("revoked"));
        }
        if let Some(expiry) = grant.get("expires_at").and_then(Value::as_str)
            && !self.mutants.on("ignore-expiry")
            && expiry <= grants::now(self.identity.fixed_clock.as_deref()).as_str()
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

    /// Step 6 (CORE section 15.5).
    fn authorize(&self, operation: &str, params: &Value) -> Result<(), Reject> {
        if self.mutants.on("ignore-grants") {
            return Ok(());
        }
        let denied = |reason: &str| Err(reject("permission_denied", json!({"reason": reason})));
        match operation {
            "core.grant.issue" => {
                let payload = &params["payload"];
                match payload.get("parent").and_then(Value::as_str) {
                    None if self.is_authority() => Ok(()),
                    None => denied("not_authority"),
                    Some(parent_id) => {
                        let parent = self.usable_grant(parent_id)?;
                        let allowed = parent["delegation"]["allowed"].as_bool().unwrap_or(false)
                            && parent["delegation"]["max_depth"].as_i64().unwrap_or(0) >= 1;
                        if !self.mutants.on("allow-delegation-escalation")
                            && !(allowed && grants::within_parent(&parent, payload))
                        {
                            return denied("delegation_exceeded");
                        }
                        Ok(())
                    }
                }
            }
            "core.grant.revoke" => {
                if self.is_authority() {
                    return Ok(());
                }
                let id = params["subject"]["id"].as_str().unwrap_or_default();
                let issuer = self
                    .store
                    .grant(id)
                    .map_err(storage)?
                    .is_some_and(|(_, grant)| grant["issuer"] == self.identity.principal.as_str());
                if issuer {
                    Ok(())
                } else {
                    denied("not_authority")
                }
            }
            _ => {
                let Some(needed) = grants::required_rights(operation, params) else {
                    return Ok(());
                };
                match params.get("grant").and_then(Value::as_str) {
                    None if self.is_authority() => Ok(()),
                    None => denied("grant_required"),
                    Some(grant_id) => {
                        let grant = self.usable_grant(grant_id)?;
                        if self.mutants.on("ignore-grant-scope") {
                            return Ok(());
                        }
                        for (right, subject) in &needed {
                            if let Err(reason) = grants::grant_covers(&grant, right, subject) {
                                return denied(reason);
                            }
                        }
                        Ok(())
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
        let intent = json!({
            "operation": params["operation"],
            "subject": params["subject"],
            "preconditions": params["preconditions"],
            "requires": params["requires"],
            "payload": params["payload"],
            "extensions": extensions,
        });
        let flaws = CanonicalFlaws {
            code_point_order: self.mutants.on("canonical-code-point-order"),
            ascii_escape: self.mutants.on("canonical-ascii-escape"),
        };
        sha256_digest(&canonical(&intent, flaws))
    }

    fn check_limits(&self, params: &Value) -> Result<(), Reject> {
        fn walk(value: &Value, depth: i64, max: &mut (i64, i64, i64)) {
            max.0 = max.0.max(depth);
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
        if max.0 > self.limits.max_depth {
            return exceeded("max_depth", self.limits.max_depth);
        }
        if max.1 > self.limits.max_string_bytes {
            return exceeded("max_string_bytes", self.limits.max_string_bytes);
        }
        if max.2 > self.limits.max_array_items {
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
        let first = validator.iter_errors(params).find(|error| {
            !(ignore_unknown
                && matches!(
                    error.kind().keyword(),
                    "unevaluatedProperties" | "additionalProperties"
                ))
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
        if operation.name == "core.grant.issue" {
            let payload = &params["payload"];
            if payload["audience"] != self.identity.provider_id.as_str()
                && !self.mutants.on("ignore-audience")
            {
                return invalid("/payload/audience", "audience is not this provider");
            }
            if let Some(expiry) = payload.get("expires_at").and_then(Value::as_str)
                && expiry <= grants::now(self.identity.fixed_clock.as_deref()).as_str()
            {
                return invalid("/payload/expires_at", "grant would already be expired");
            }
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
            if seen.contains(&entry["subject"]) {
                return invalid("/preconditions", "duplicate precondition subject");
            }
            seen.push(entry["subject"].clone());
        }
        if !seen.contains(&params["subject"]) {
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
        let epoch = self
            .store
            .revision(AUTHORITY_KIND, AUTHORITY_ID)
            .map_err(storage)?;
        if operation == "core-test.subject.put" && !self.mutants.on("ignore-authority-epoch") {
            let asserted = params["authority_epoch"].as_i64().unwrap_or_default();
            if asserted < epoch {
                return Err(reject(
                    "stale_authority_epoch",
                    json!({"current_epoch": epoch}),
                ));
            }
            if asserted > epoch {
                return Err(reject("unknown_authority_epoch", json!({})));
            }
        }
        let mut failed = Vec::new();
        for entry in params["preconditions"].as_array().into_iter().flatten() {
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
                failed.push(
                    json!({"subject": entry["subject"], "expected": expected, "current": current}),
                );
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
            "core.grant.get" => {
                let id = params["payload"]["grant"].as_str().unwrap_or_default();
                let found = self.store.grant(id).map_err(storage)?;
                match found {
                    Some((revision, grant))
                        if self.is_authority()
                            || grant["holder"] == self.identity.principal.as_str()
                            || grant["issuer"] == self.identity.principal.as_str()
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
        if self.selected.is_some() && !self.mutants.on("allow-renegotiation") {
            return Err(reject("already_negotiated", json!({})));
        }
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
            let required = request["required"].as_bool().unwrap_or(false);
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
            let code = if reasons.iter().any(|r| {
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
        Ok(json!({
            "selected": selected_json,
            "unselected": unselected,
            "limits": self.limits.to_json(),
            "dedupe_window": self.window_json()?,
        }))
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

/// Apply one accepted command inside the owner transaction; returns (revision, outcome).
fn apply_change(
    tx: &rusqlite::Transaction,
    operation: &str,
    subject: &Value,
    payload: &Value,
    principal: &str,
    revoke_ids: &[String],
) -> rusqlite::Result<(i64, Value)> {
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
            Ok((1, json!({"grant": record})))
        }
        "core.grant.revoke" => {
            let mut target_revision = 0;
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
            }
            Ok((target_revision, json!({"revoked": revoke_ids})))
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
            let outcome = if operation == "core-test.authority.claim" {
                json!({"epoch": revision})
            } else {
                json!({"value": value})
            };
            Ok((revision, outcome))
        }
    }
}

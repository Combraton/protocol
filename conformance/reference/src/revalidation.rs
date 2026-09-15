//! Context revalidation by the reference executor (EXECUTION section 13.1, owner decision M4-Q4).
//!
//! The executor checks only typed conditions it can observe against the basis it holds, reports
//! each as `match`, `mismatch` or `unavailable`, and applies the result by obligation at each
//! boundary. It fetches packet facts and bytes only through public connections under the
//! per-audience grants the binding names, and verifies the digest (CONTEXT section 7).

use serde_json::{Value, json};

use crate::mutants::Mutants;
use crate::peer::Peer;

/// The basis the executor observes for an execution: the host's configured basis, overridden
/// by what the execution's own script observed.
pub fn observed_basis(executor: &Value, record: &Value) -> Value {
    let mut basis = executor["observed_basis"].clone();
    if !basis.is_object() {
        basis = json!({});
    }
    if let Some(overrides) = record["observed_basis"].as_object() {
        for (key, value) in overrides {
            if key == "repositories" {
                for (repository, state) in value.as_object().into_iter().flatten() {
                    basis["repositories"][repository] = state.clone();
                }
            } else {
                basis[key] = value.clone();
            }
        }
    }
    basis
}

/// The executor as a check sees it at `now`: its configured basis, the scheduled `basis_changes`
/// that have taken effect, and host observations recorded by executions (EXECUTION 15 controls).
pub fn host_view(
    tx: &rusqlite::Transaction,
    executor: &Value,
    now: &str,
) -> rusqlite::Result<Value> {
    let mut view = executor.clone();
    let mut updates: Vec<(String, Value)> = executor["basis_changes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["at"].as_str().is_some_and(|at| at <= now))
        .map(|c| {
            (
                c["at"].as_str().unwrap_or_default().to_string(),
                c["observed_basis"].clone(),
            )
        })
        .collect();
    if let Some((_, value)) =
        crate::evidence::load(tx, HOST_OBSERVATIONS, crate::execution::host_id(executor))?
    {
        for entry in value["updates"].as_array().into_iter().flatten() {
            updates.push((
                entry["at"].as_str().unwrap_or_default().to_string(),
                entry["observed_basis"].clone(),
            ));
        }
    }
    updates.sort_by(|a, b| a.0.cmp(&b.0));
    let mut basis = if executor["observed_basis"].is_object() {
        executor["observed_basis"].clone()
    } else {
        json!({})
    };
    for (_, update) in updates {
        merge_basis(&mut basis, &update);
    }
    view["observed_basis"] = basis;
    Ok(view)
}

pub const HOST_OBSERVATIONS: &str = "execution.host_observation";

fn merge_basis(basis: &mut Value, update: &Value) {
    for (key, value) in update.as_object().into_iter().flatten() {
        if key == "repositories" {
            for (repository, state) in value.as_object().into_iter().flatten() {
                basis["repositories"][repository] = state.clone();
            }
        } else {
            basis[key] = value.clone();
        }
    }
}

/// Record a host-level observation an execution made (script step `observe_host_basis`).
pub fn record_host_observation(
    tx: &rusqlite::Transaction,
    executor: &Value,
    basis: &Value,
    now: &str,
) -> rusqlite::Result<()> {
    let host = crate::execution::host_id(executor);
    let (revision, mut value) =
        crate::evidence::load(tx, HOST_OBSERVATIONS, host)?.unwrap_or((0, json!({"updates": []})));
    if let Some(list) = value["updates"].as_array_mut() {
        list.push(json!({"at": now, "observed_basis": basis}));
    }
    tx.execute(
        "INSERT INTO subjects VALUES (?1, ?2, ?3, ?4, 1)
         ON CONFLICT (kind, id) DO UPDATE SET revision=excluded.revision, value=excluded.value",
        rusqlite::params![HOST_OBSERVATIONS, host, revision + 1, value.to_string()],
    )?;
    Ok(())
}

fn peer_for<'a>(executor: &'a Value, provider: &str) -> Option<&'a Value> {
    executor["peers"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["provider_id"] == provider)
}

/// Read the bound revision's result facts at the context provider under the binding's grant.
fn read_facts(executor: &Value, binding: &Value, claims: bool) -> Result<Value, String> {
    let reference = &binding["packet"];
    let context = &binding["fetch"]["context"];
    let provider = context["provider"].as_str().unwrap_or_default();
    let peer = peer_for(executor, provider)
        .ok_or_else(|| format!("no connection to context provider {provider}"))?;
    // Claim revalidation reads claim facts, which need `context.claims` (EXECUTION section 13.3).
    let optional: Vec<&str> = if claims {
        vec!["context.claims"]
    } else {
        Vec::new()
    };
    let profiles = json!([
        {"name": "core", "majors": [1], "required": true, "required_features": ["core.events", "core.grants"], "optional_features": []},
        {"name": "context", "majors": [1], "required": true, "required_features": [], "optional_features": optional},
    ]);
    let mut client =
        Peer::connect(peer, profiles).map_err(|f| format!("context provider: {}", f.describe()))?;
    client
        .query(
            "context.packet.inspect",
            json!({"packet": reference["packet"]["id"], "revision": reference["revision"], "max_bytes": 1}),
            context["grant"].as_str(),
        )
        .map_err(|f| format!("context provider: {}", f.describe()))
}

/// Fetch the bound artifact's bytes at its evidence provider and verify the digest.
fn fetch_bytes(executor: &Value, binding: &Value, mutants: &Mutants) -> Result<(), String> {
    let artifact = &binding["packet"]["artifact"];
    let provider = artifact["provider"].as_str().unwrap_or_default();
    let evidence = &binding["fetch"]["evidence"];
    if evidence["provider"] != provider {
        return Err("the evidence grant is for another provider than the packet's artifact".into());
    }
    let peer = peer_for(executor, provider)
        .ok_or_else(|| format!("no connection to evidence provider {provider}"))?;
    let mut client = Peer::connect(peer, crate::peer::evidence_profiles(true))
        .map_err(|f| format!("evidence provider: {}", f.describe()))?;
    let mut bytes = Vec::new();
    loop {
        let chunk = client
            .query(
                "evidence.fetch",
                json!({"artifact": artifact["artifact"], "digest": artifact["digest"], "offset": bytes.len(), "max_bytes": 65_536}),
                evidence["grant"].as_str(),
            )
            .map_err(|f| format!("evidence provider: {}", f.describe()))?;
        if chunk["availability"]["state"] != "available" {
            return Err(format!(
                "artifact availability is {}",
                chunk["availability"]["state"]
            ));
        }
        let data =
            crate::evidence::decode_base64(chunk["data_base64"].as_str().unwrap_or_default())
                .unwrap_or_default();
        bytes.extend_from_slice(&data);
        if data.is_empty() || chunk["next_offset"].as_u64() >= chunk["size"].as_u64() {
            break;
        }
    }
    // The digest of the bytes actually assembled, against the immutable binding; the provider's
    // advertised digest is not evidence of what was received.
    let digest = crate::json::sha256_digest(&bytes);
    if digest != artifact["digest"].as_str().unwrap_or_default()
        && !mutants.on("fetched-digest-unchecked")
    {
        return Err(format!(
            "fetched bytes have digest {digest}, not the bound digest"
        ));
    }
    Ok(())
}

/// Whether the executor holds the bound packet's exact bytes.
fn holds(executor: &Value, binding: &mut Value, mutants: &Mutants) -> (bool, Option<String>) {
    let digest = binding["packet"]["artifact"]["digest"].clone();
    if !digest.is_null() && binding["held_digest"] == digest {
        return (true, None);
    }
    if binding["fetch"].get("evidence").is_some() {
        return match fetch_bytes(executor, binding, mutants) {
            Ok(()) => {
                binding["held_digest"] = digest;
                (true, None)
            }
            Err(reason) => (false, Some(reason)),
        };
    }
    // Without fetch grants, the executor holds only packets its host was given (EXECUTION 15).
    let held = executor["context_packets"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|p| {
            p["reference"] == binding["packet"]
                || (p["ref"] == binding["packet"]["ref"]
                    && p["digest"] == binding["packet"]["digest"])
        });
    (held, None)
}

/// Check one binding at a boundary: typed conditions against the observed basis, and holding.
pub fn check(
    executor: &Value,
    record: &Value,
    binding: &mut Value,
    boundary: &str,
    now: &str,
    mutants: &Mutants,
) -> Value {
    let basis = observed_basis(executor, record);
    let mut results = Vec::new();
    let mut refusal: Option<String> = None;
    // With a context grant, every check re-reads the bound revision's facts: whether it is still
    // the request's current revision is observable there, and so are unmet required items.
    if binding["fetch"].get("context").is_some() {
        let require_current =
            binding["require_current"] == true && !mutants.on("current-requirement-ignored");
        let claims = record["context"]["claim_revalidation"] == true;
        match read_facts(executor, binding, claims) {
            Err(reason) => {
                let result = if mutants.on("context-outage-fails-open") {
                    "match"
                } else {
                    "unavailable"
                };
                let evidence = reason.chars().take(256).collect::<String>();
                results.push(
                    json!({"condition_id": "packet.facts", "result": result, "evidence": evidence}),
                );
                if require_current {
                    results.push(json!({"condition_id": "packet.current", "result": result, "evidence": evidence}));
                }
            }
            Ok(facts) => {
                if facts["reference"] != binding["packet"]
                    && !mutants.on("packet-reference-unchecked")
                {
                    refusal = Some(
                        "the context provider's packet reference differs from the binding".into(),
                    );
                }
                let unmet: Vec<String> = facts["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|i| i["obligation"] != "advisory" && i["result"] == "unmet")
                    .filter_map(|i| i["item_id"].as_str().map(String::from))
                    .collect();
                if !unmet.is_empty() && refusal.is_none() && !mutants.on("unmet-packet-satisfies") {
                    refusal = Some(format!(
                        "the packet reports required items unmet: {}",
                        unmet.join(", ")
                    ));
                }
                // A correction after this revision was prepared invalidates its required items,
                // whether or not the revision is pinned (CONTEXT section 8).
                let required: Vec<&Value> = facts["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|i| i["obligation"] != "advisory")
                    .map(|i| &i["item_id"])
                    .collect();
                let listed = |member: &str, with_claim: bool| -> Vec<String> {
                    facts[member]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|i| required.contains(&&i["item_id"]))
                        .filter(|i| i.get("claim").is_some() == with_claim)
                        .filter_map(|i| i["item_id"].as_str().map(String::from))
                        .collect()
                };
                let invalidated = listed("invalidated_items", false);
                let enforce_claims = claims && !mutants.on("claim-revalidation-ignored");
                let claim_invalidated = if enforce_claims {
                    listed("invalidated_items", true)
                } else {
                    Vec::new()
                };
                let unverified = if enforce_claims {
                    listed("unverified_items", true)
                } else {
                    Vec::new()
                };
                let mut entry = json!({"condition_id": "packet.facts", "evidence": "context provider result facts"});
                let corrected = !invalidated.is_empty() && !mutants.on("pinned-ignores-correction");
                if corrected || !claim_invalidated.is_empty() {
                    entry["result"] = json!("mismatch");
                    let mut observed = Vec::new();
                    if corrected {
                        observed.push(format!("corrected: {}", invalidated.join(", ")));
                    }
                    if !claim_invalidated.is_empty() {
                        observed.push(format!(
                            "claim invalidated: {}",
                            claim_invalidated.join(", ")
                        ));
                    }
                    entry["observed"] = json!(observed.join("; "));
                } else if !unverified.is_empty() {
                    entry["result"] = json!("unavailable");
                    entry["evidence"] = json!(format!(
                        "claim knowledge not established for: {}",
                        unverified.join(", ")
                    ));
                } else {
                    entry["result"] = json!("match");
                }
                results.push(entry);
                let current = facts["current"] == true;
                if !current && mutants.on("newer-revision-substituted") {
                    // Mutant: silently rebinds to the newest revision.
                    binding["packet"]["revision"] = facts["superseded_by"]["revision"].clone();
                } else if require_current || (!current && mutants.on("supersession-always-stale")) {
                    let mut entry = json!({"condition_id": "packet.current", "result": if current { "match" } else { "mismatch" },
                        "evidence": "context provider result facts"});
                    if !current {
                        entry["observed"] = json!(format!(
                            "superseded by revision {}",
                            facts["superseded_by"]["revision"]
                        ));
                    }
                    results.push(entry);
                }
            }
        }
    }
    let (mut held, mut fetch_failure) = holds(executor, binding, mutants);
    if let Some(reason) = refusal {
        held = false;
        fetch_failure = Some(reason);
        if let Some(map) = binding.as_object_mut() {
            map.remove("held_digest");
        }
    }
    for condition in binding["conditions"].as_array().into_iter().flatten() {
        let repository = condition["repository"].as_str().unwrap_or_default();
        let observed = match condition["kind"].as_str() {
            Some("repository_tree") => basis["repositories"][repository]["tree"].clone(),
            Some("dirty_snapshot") => basis["repositories"][repository]["dirty_snapshot"].clone(),
            Some("environment_digest") => basis["environment_digest"].clone(),
            // An authority revision is not something the executor observes.
            _ => Value::Null,
        };
        let mut entry = json!({"condition_id": condition["condition_id"]});
        if observed.is_null() {
            let result = if mutants.on("unavailable-treated-as-fresh") {
                "match"
            } else if mutants.on("unavailable-reported-stale") {
                "mismatch"
            } else {
                "unavailable"
            };
            entry["result"] = json!(result);
            entry["evidence"] = json!("the executor cannot observe this condition");
        } else {
            let matched = observed == condition["expected"];
            entry["result"] = json!(if matched { "match" } else { "mismatch" });
            entry["observed"] = observed;
            entry["evidence"] = json!("executor basis");
        }
        results.push(entry);
    }
    let state = if results.iter().any(|r| r["result"] == "mismatch") {
        "stale"
    } else if held && results.iter().all(|r| r["result"] == "match") {
        "current"
    } else {
        "unknown"
    };
    let mut record_entry = json!({
        "binding_id": binding["binding_id"],
        "boundary": boundary,
        "checked_at": now,
        "observed_basis": basis,
        "held": held,
        "results": results,
        "state": state,
    });
    if let Some(reason) = fetch_failure {
        record_entry["fetch"] = json!(reason.chars().take(256).collect::<String>());
    }
    if boundary == "transition" {
        record_entry["transition"] = binding["transition"].clone();
    }
    binding["revalidation"] = json!(state);
    record_entry
}

/// The M3 binding state implied by a check (EXECUTION section 13).
pub fn m3_state(binding: &Value, held: bool) -> &'static str {
    match (
        held && binding["revalidation"] == "current",
        binding["obligation"].as_str(),
    ) {
        (true, _) => "satisfied",
        (false, Some("advisory")) => "gap",
        _ => "unsatisfied",
    }
}

/// The queue reason for a required binding that is not current.
pub fn block_reason(check: &Value) -> &'static str {
    match check["state"].as_str() {
        Some("stale") => "context_binding_stale",
        _ if check["held"] == false => "context_binding_unsatisfied",
        _ => "context_binding_unknown",
    }
}

/// Whether a check differs from the last recorded check for the same binding and boundary.
pub fn is_new(context: &Value, check: &Value) -> bool {
    let same =
        |c: &&Value| c["binding_id"] == check["binding_id"] && c["boundary"] == check["boundary"];
    match context["checks"]
        .as_array()
        .and_then(|list| list.iter().rev().find(same))
    {
        None => true,
        Some(last) => {
            last["state"] != check["state"]
                || last["results"] != check["results"]
                || last["held"] != check["held"]
        }
    }
}

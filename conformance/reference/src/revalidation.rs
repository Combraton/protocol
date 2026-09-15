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

fn peer_for<'a>(executor: &'a Value, provider: &str) -> Option<&'a Value> {
    executor["peers"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["provider_id"] == provider)
}

/// Read the bound revision's result facts at the context provider under the binding's grant.
fn read_facts(executor: &Value, binding: &Value) -> Result<Value, String> {
    let reference = &binding["packet"];
    let context = &binding["fetch"]["context"];
    let provider = context["provider"].as_str().unwrap_or_default();
    let peer = peer_for(executor, provider)
        .ok_or_else(|| format!("no connection to context provider {provider}"))?;
    let profiles = json!([
        {"name": "core", "majors": [1], "required": true, "required_features": ["core.events", "core.grants"], "optional_features": []},
        {"name": "context", "majors": [1], "required": true, "required_features": [], "optional_features": []},
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
fn fetch_bytes(executor: &Value, binding: &Value) -> Result<(), String> {
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
    let digest = crate::json::sha256_digest(&bytes);
    if digest != artifact["digest"].as_str().unwrap_or_default() {
        return Err(format!(
            "fetched bytes have digest {digest}, not the bound digest"
        ));
    }
    Ok(())
}

/// Whether the executor holds the bound packet's exact bytes.
fn holds(executor: &Value, binding: &mut Value) -> (bool, Option<String>) {
    let digest = binding["packet"]["artifact"]["digest"].clone();
    if !digest.is_null() && binding["held_digest"] == digest {
        return (true, None);
    }
    if binding["fetch"].get("evidence").is_some() {
        return match fetch_bytes(executor, binding) {
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
        match read_facts(executor, binding) {
            Err(reason) => {
                let fail_open = mutants.on("context-outage-fails-open");
                results.push(json!({"condition_id": "packet.current",
                    "result": if fail_open { "match" } else { "unavailable" },
                    "evidence": reason.chars().take(256).collect::<String>()}));
            }
            Ok(facts) => {
                if facts["reference"] != binding["packet"]
                    && !mutants.on("packet-reference-unchecked")
                {
                    refusal = Some(
                        "the context provider's packet reference differs from the binding".into(),
                    );
                }
                let current = facts["current"] == true;
                let mut entry = json!({"condition_id": "packet.current", "result": if current { "match" } else { "mismatch" },
                    "evidence": "context provider result facts"});
                if !current {
                    entry["observed"] = json!("superseded");
                }
                results.push(entry);
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
            }
        }
    }
    let (mut held, mut fetch_failure) = holds(executor, binding);
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

//! Reference provider for `verification/1` (VERIFICATION draft, owner decisions M5-Q5, Q6, Q9).
//!
//! Jobs and receipts are ordinary subjects. Contracts are read from this provider's own evidence
//! store, and every receipt is sealed there as one artifact (`receipt.<id>`). The scripted
//! evaluator (`verifier` in launch configuration) is test environment, never protocol.

use rusqlite::Transaction;
use serde_json::{Value, json};

use crate::execution::{Applied, Draft, provider_event};
use crate::json::{CanonicalFlaws, canonical, sha256_digest};
use crate::mutants::Mutants;
use crate::store::Store;

pub const JOB: &str = "verification.job";
pub const RECEIPT: &str = "verification.receipt";
pub const CONTRACT_FORMAT: &str = "combraton-verification-contract/1";
pub const RECEIPT_FORMAT: &str = "combraton-verification-receipt/1";
pub const RECEIPT_MEDIA_TYPE: &str = "application/vnd.combraton.verification-receipt+json";

pub type Refusal = (&'static str, Value);

/// What commands and reads need besides the store.
pub struct Context<'a> {
    pub now: String,
    pub principal: &'a str,
    pub provider_id: &'a str,
    pub capabilities: &'a Value,
    pub evidence_config: &'a Value,
    pub order: i64,
    pub mutants: &'a Mutants,
}

fn subject(kind: &str, id: &str) -> Value {
    json!({"kind": kind, "id": id})
}

fn load(tx: &Transaction, kind: &str, id: &str) -> rusqlite::Result<Option<(i64, Value)>> {
    crate::evidence::load(tx, kind, id)
}

fn save(
    tx: &Transaction,
    kind: &str,
    id: &str,
    revision: i64,
    record: &Value,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO subjects VALUES (?1, ?2, ?3, ?4, 1)
         ON CONFLICT (kind, id) DO UPDATE SET revision=excluded.revision, value=excluded.value,
         applied_count=applied_count+1",
        rusqlite::params![kind, id, revision, record.to_string()],
    )?;
    Ok(())
}

/// The capability predicate naming one evaluator version (VERIFICATION section 4).
pub fn predicate_name(evaluator: &Value) -> String {
    format!(
        "verification.evaluator.{}.v{}",
        evaluator["id"].as_str().unwrap_or_default(),
        evaluator["version"].as_str().unwrap_or_default()
    )
}

/// Capability predicates for the scripted evaluators in launch configuration.
pub fn predicates(config: &Value) -> Vec<Value> {
    config["evaluators"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|e| {
            json!({
                "name": predicate_name(e),
                "status": e["status"].as_str().unwrap_or("supported"),
                "evidence": {"source": "launch-configuration"},
            })
        })
        .collect()
}

fn status(capabilities: &Value, name: &str) -> String {
    capabilities
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["name"] == name)
        .and_then(|p| p["status"].as_str())
        .unwrap_or("unsupported")
        .to_string()
}

// ---------------------------------------------------------------------------------------------
// Step 2.

pub fn validate(operation: &str, params: &Value) -> Result<(), (String, &'static str)> {
    let payload = &params["payload"];
    if operation == "verification.receipt.record" {
        for (index, property) in payload["properties"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            let needs = matches!(
                property["result"].as_str(),
                Some("not_evaluated" | "indeterminate")
            );
            if needs != property.get("reason").is_some() {
                return Err((
                    format!("/payload/properties/{index}/reason"),
                    "a reason is given exactly for not_evaluated and indeterminate",
                ));
            }
        }
        if payload["observed_from"].as_str() > payload["observed_until"].as_str() {
            return Err((
                "/payload/observed_until".to_string(),
                "observed_until is before observed_from",
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Contracts (VERIFICATION section 3).

/// The contract content, or `contract_unavailable` / `artifact_digest_mismatch`.
pub fn read_contract(
    tx: &Transaction,
    reference: &Value,
    provider_id: &str,
    evidence_config: &Value,
    mutants: &Mutants,
) -> rusqlite::Result<Result<Value, Refusal>> {
    let unavailable = |reason: &str| Ok(Err(("contract_unavailable", json!({"reason": reason}))));
    if reference["provider"] != provider_id {
        return unavailable("unreachable");
    }
    let id = reference["artifact"]["id"].as_str().unwrap_or_default();
    let Some((_, record)) = load(tx, crate::evidence::ARTIFACT, id)? else {
        return unavailable("not_found");
    };
    if record["state"] != "sealed" {
        return unavailable("not_sealed");
    }
    let observed = crate::evidence::observed(tx, id, &record, evidence_config, mutants)?;
    if observed["state"] != "available" {
        return unavailable("unavailable");
    }
    if record["descriptor"]["digest"] != reference["digest"] {
        return Ok(Err(("artifact_digest_mismatch", json!({}))));
    }
    let bytes = crate::evidence::sealed_bytes(tx, id)?;
    match serde_json::from_slice::<Value>(&bytes) {
        Ok(contract)
            if contract["format"] == CONTRACT_FORMAT
                && contract["subjects"].is_array()
                && contract["properties"].is_array() =>
        {
            Ok(Ok(contract))
        }
        _ => unavailable("invalid_format"),
    }
}

/// Missing, duplicated or unknown names against the contract's list.
fn listing(expected: &[String], given: &[String]) -> Option<Value> {
    let missing: Vec<&String> = expected.iter().filter(|e| !given.contains(e)).collect();
    let mut duplicated = Vec::new();
    for (index, name) in given.iter().enumerate() {
        if given[..index].contains(name) && !duplicated.contains(name) {
            duplicated.push(name.clone());
        }
    }
    let unknown: Vec<&String> = given.iter().filter(|g| !expected.contains(g)).collect();
    if missing.is_empty() && duplicated.is_empty() && unknown.is_empty() {
        None
    } else {
        Some(json!({"missing": missing, "duplicated": duplicated, "unknown": unknown}))
    }
}

fn names(list: &Value, key: &str) -> Vec<String> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item[key].as_str().map(String::from))
        .collect()
}

fn roles_refusal(contract: &Value, subjects: &Value) -> Option<Refusal> {
    listing(
        &names(&contract["subjects"], "role"),
        &names(subjects, "role"),
    )
    .map(|details| {
        let mut details = details;
        details["path"] = json!("/payload/subjects");
        details["reason"] = json!("subject roles must match the contract's roles exactly");
        ("invalid_envelope", details)
    })
}

// ---------------------------------------------------------------------------------------------
// Step 7.

pub fn check(
    tx: &Transaction,
    operation: &str,
    params: &Value,
    ctx: &Context,
) -> rusqlite::Result<Result<(), Refusal>> {
    let payload = &params["payload"];
    let mutants = ctx.mutants;
    match operation {
        "verification.evaluate_contract" => {
            let name = predicate_name(&payload["evaluator"]);
            let current = status(ctx.capabilities, &name);
            if current != "supported"
                && pinned_substitute(ctx.capabilities, &payload["evaluator"], mutants).is_none()
            {
                return Ok(Err((
                    "capability_unavailable",
                    json!({"capability": name, "status": current}),
                )));
            }
            let contract = match read_contract(
                tx,
                &payload["contract"],
                ctx.provider_id,
                ctx.evidence_config,
                mutants,
            )? {
                Ok(contract) => contract,
                Err(refusal) => return Ok(Err(refusal)),
            };
            Ok(roles_refusal(&contract, &payload["subjects"]).map_or(Ok(()), Err))
        }
        "verification.receipt.record" => {
            let contract = match read_contract(
                tx,
                &payload["contract"],
                ctx.provider_id,
                ctx.evidence_config,
                mutants,
            )? {
                Ok(contract) => contract,
                Err(refusal) => return Ok(Err(refusal)),
            };
            if let Some(refusal) = roles_refusal(&contract, &payload["subjects"]) {
                return Ok(Err(refusal));
            }
            if !mutants.on("receipt-listing-incomplete-accepted")
                && let Some(mut details) = listing(
                    &names(&contract["properties"], "property_id"),
                    &names(&payload["properties"], "property_id"),
                )
            {
                details["path"] = json!("/payload/properties");
                details["reason"] = json!("every contract property is listed exactly once");
                return Ok(Err(("invalid_envelope", details)));
            }
            Ok(Ok(()))
        }
        _ => Ok(Ok(())),
    }
}

/// Mutant `evaluator-version-substituted`: another supported version of the same evaluator.
fn pinned_substitute(capabilities: &Value, evaluator: &Value, mutants: &Mutants) -> Option<Value> {
    if !mutants.on("evaluator-version-substituted") {
        return None;
    }
    let prefix = format!(
        "verification.evaluator.{}.v",
        evaluator["id"].as_str().unwrap_or_default()
    );
    capabilities.as_array().into_iter().flatten().find_map(|p| {
        let name = p["name"].as_str()?;
        (p["status"] == "supported" && name.starts_with(&prefix))
            .then(|| json!({"id": evaluator["id"], "version": &name[prefix.len()..]}))
    })
}

// ---------------------------------------------------------------------------------------------
// Receipts.

fn seal_receipt(
    tx: &Transaction,
    id: &str,
    content: &Value,
    provider_id: &str,
    now: &str,
) -> rusqlite::Result<(Value, Vec<Draft>)> {
    let bytes = canonical(content, CanonicalFlaws::default());
    let artifact_id = format!("receipt.{id}");
    let drafts = crate::evidence::publish_artifact(
        tx,
        &crate::evidence::Publication {
            id: &artifact_id,
            provider: provider_id,
            producer_id: "reference-verifier",
            source: json!({"kind": RECEIPT, "id": id}),
            scope: "verification",
            retention_class: "verification-receipt",
            media_type: RECEIPT_MEDIA_TYPE,
            now,
        },
        &bytes,
    )?;
    let artifact = json!({"provider": provider_id, "artifact": {"kind": crate::evidence::ARTIFACT, "id": artifact_id}, "digest": sha256_digest(&bytes)});
    Ok((
        json!({"provider": provider_id, "receipt": id, "artifact": artifact}),
        drafts,
    ))
}

fn store_receipt(
    tx: &Transaction,
    id: &str,
    content: &Value,
    ctx_provider: &str,
    now: &str,
    event: &'static str,
    job: Option<&str>,
) -> rusqlite::Result<(Value, Vec<Draft>)> {
    let (reference, mut drafts) = seal_receipt(tx, id, content, ctx_provider, now)?;
    let record = json!({"reference": reference, "recorded_at": now});
    save(tx, RECEIPT, id, 1, &record)?;
    let mut payload = json!({"reference": reference});
    if let Some(job) = job {
        payload["job"] = json!(job);
    }
    drafts.push((event, subject(RECEIPT, id), 1, payload));
    Ok((reference, drafts))
}

fn content_of(tx: &Transaction, reference: &Value) -> rusqlite::Result<Option<Value>> {
    let id = reference["artifact"]["artifact"]["id"]
        .as_str()
        .unwrap_or_default();
    let bytes = crate::evidence::sealed_bytes(tx, id)?;
    Ok(serde_json::from_slice(&bytes).ok())
}

// ---------------------------------------------------------------------------------------------
// Commands.

pub fn apply(
    tx: &Transaction,
    operation: &str,
    id: &str,
    params: &Value,
    ctx: &Context,
) -> rusqlite::Result<Applied> {
    let payload = &params["payload"];
    let mutants = ctx.mutants;
    match operation {
        "verification.evaluate_contract" => {
            let name = predicate_name(&payload["evaluator"]);
            let evaluator = if status(ctx.capabilities, &name) == "supported" {
                payload["evaluator"].clone()
            } else {
                pinned_substitute(ctx.capabilities, &payload["evaluator"], mutants)
                    .unwrap_or_else(|| payload["evaluator"].clone())
            };
            let job = json!({
                "contract": payload["contract"],
                "subjects": payload["subjects"],
                "environment": payload.get("environment").cloned().unwrap_or(json!({"anchors": {}})),
                "evaluator": evaluator,
                "state": "queued",
                "position": 0,
                "recorded": [],
                "submitted_at": ctx.now,
                "order": ctx.order,
            });
            save(tx, JOB, id, 1, &job)?;
            let events: Vec<Draft> = vec![(
                "verification.job.changed",
                subject(JOB, id),
                1,
                json!({"state": "queued", "evaluator": evaluator}),
            )];
            let mut outcome = json!({"job": subject(JOB, id), "state": "queued"});
            if mutants.on("results-in-outcome") {
                outcome["properties"] = json!([]);
            }
            Ok((1, outcome, events, Vec::new()))
        }
        "verification.receipt.record" => {
            let content = json!({
                "format": RECEIPT_FORMAT,
                "receipt": id,
                "subjects": payload["subjects"],
                "contract": payload["contract"],
                "evaluator": {"id": payload["evaluator"]["id"], "version": payload["evaluator"]["version"], "principal": ctx.principal},
                "environment": payload["environment"],
                "inputs": payload["inputs"],
                "properties": payload["properties"],
                "observed_from": payload["observed_from"],
                "observed_until": payload["observed_until"],
                "valid_until": payload["valid_until"],
                "scope": payload["scope"],
                "job": Value::Null,
                "provenance": payload["provenance"],
            });
            let (reference, drafts) = store_receipt(
                tx,
                id,
                &content,
                ctx.provider_id,
                &ctx.now,
                "verification.receipt.recorded",
                None,
            )?;
            if mutants.on("failure-marks-execution")
                && payload["properties"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|p| p["result"] == "fail")
            {
                for source in payload["provenance"].as_array().into_iter().flatten() {
                    if source["kind"] == crate::execution::KIND
                        && let Some(execution) = source["id"].as_str()
                        && let Some((revision, mut record)) =
                            load(tx, crate::execution::KIND, execution)?
                    {
                        record["cancellation"] = json!({"receipt": {"state": "cancel_requested", "operation_ref": "op-verification"}, "outcome": "cancelled"});
                        save(tx, crate::execution::KIND, execution, revision + 1, &record)?;
                    }
                }
            }
            Ok((1, json!({"reference": reference}), drafts, Vec::new()))
        }
        _ => Ok((0, json!({}), Vec::new(), Vec::new())),
    }
}

// ---------------------------------------------------------------------------------------------
// The scripted evaluator (VERIFICATION section 11).

fn script_for<'a>(config: &'a Value, job: &str) -> Option<&'a Value> {
    config["scripts"]
        .get(job)
        .or_else(|| config.get("default_script"))
}

pub fn tick(
    store: &mut Store,
    now: &str,
    capabilities: &Value,
    config: &Value,
    provider_id: &str,
    mutants: &Mutants,
) -> rusqlite::Result<()> {
    let stream = store.stream_id()?;
    let jobs: Vec<String> = {
        let tx = store.transaction()?;
        let mut statement = tx.prepare("SELECT id FROM subjects WHERE kind=?1 ORDER BY id")?;
        let rows = statement.query_map([JOB], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for id in jobs {
        let tx = store.transaction()?;
        let Some((mut revision, mut job)) = load(&tx, JOB, &id)? else {
            continue;
        };
        if job["state"] == "completed" {
            continue;
        }
        let mut drafts: Vec<Draft> = Vec::new();
        let before = job.clone();
        advance(
            &tx,
            &id,
            &mut job,
            now,
            capabilities,
            config,
            provider_id,
            mutants,
            &mut drafts,
            &mut revision,
        )?;
        if job != before {
            save(&tx, JOB, &id, revision, &job)?;
        }
        for draft in drafts {
            provider_event(&tx, &stream, now, draft)?;
        }
        tx.commit()?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn advance(
    tx: &Transaction,
    id: &str,
    job: &mut Value,
    now: &str,
    capabilities: &Value,
    config: &Value,
    provider_id: &str,
    mutants: &Mutants,
    drafts: &mut Vec<Draft>,
    revision: &mut i64,
) -> rusqlite::Result<()> {
    let job_subject = subject(JOB, id);
    // A lost pinned evaluator ends the job; recorded results stay (VERIFICATION section 4).
    if status(capabilities, &predicate_name(&job["evaluator"])) != "supported" {
        if mutants.on("lost-evaluator-switches-version")
            && let Some(other) = pinned_substitute_any(capabilities, &job["evaluator"])
        {
            job["evaluator"] = other;
        } else {
            return complete(
                tx,
                id,
                job,
                now,
                provider_id,
                true,
                mutants,
                drafts,
                revision,
            );
        }
    }
    let steps = script_for(config, id)
        .cloned()
        .unwrap_or_else(|| json!([{"complete": true}]));
    loop {
        let position = job["position"].as_u64().unwrap_or(0) as usize;
        let Some(step) = steps.get(position) else {
            return Ok(());
        };
        if let Some(instant) = step["wait_until"].as_str()
            && now < instant
        {
            return Ok(());
        }
        if job["state"] == "queued" {
            job["state"] = json!("running");
            job["started_at"] = json!(now);
            *revision += 1;
            drafts.push((
                "verification.job.changed",
                job_subject.clone(),
                *revision,
                json!({"state": "running", "evaluator": job["evaluator"]}),
            ));
        }
        job["position"] = json!(position + 1);
        if step.get("property").is_some() {
            let p = &step["property"];
            let mut entry = json!({"property_id": p["property_id"], "result": p["result"]});
            if let Some(reason) = p.get("reason") {
                entry["reason"] = reason.clone();
            }
            if let Some(list) = job["recorded"].as_array_mut() {
                list.push(entry.clone());
            }
            *revision += 1;
            drafts.push((
                "verification.property.recorded",
                job_subject.clone(),
                *revision,
                entry,
            ));
        } else if let Some(environment) = step.get("observe") {
            job["observed_environment"] = environment.clone();
        } else if let Some(until) = step.get("valid_until") {
            job["valid_until"] = until.clone();
        } else if step.get("complete").is_some() {
            return complete(
                tx,
                id,
                job,
                now,
                provider_id,
                false,
                mutants,
                drafts,
                revision,
            );
        }
    }
}

fn pinned_substitute_any(capabilities: &Value, evaluator: &Value) -> Option<Value> {
    let prefix = format!(
        "verification.evaluator.{}.v",
        evaluator["id"].as_str().unwrap_or_default()
    );
    capabilities.as_array().into_iter().flatten().find_map(|p| {
        let name = p["name"].as_str()?;
        (p["status"] == "supported" && name.starts_with(&prefix))
            .then(|| json!({"id": evaluator["id"], "version": &name[prefix.len()..]}))
    })
}

#[allow(clippy::too_many_arguments)]
fn complete(
    tx: &Transaction,
    id: &str,
    job: &mut Value,
    now: &str,
    provider_id: &str,
    lost: bool,
    mutants: &Mutants,
    drafts: &mut Vec<Draft>,
    revision: &mut i64,
) -> rusqlite::Result<()> {
    let contract = read_contract(
        tx,
        &job["contract"],
        provider_id,
        &Value::Null,
        &Mutants::default(),
    )?
    .unwrap_or(Value::Null);
    let recorded: Vec<Value> = job["recorded"].as_array().cloned().unwrap_or_default();
    let mut properties = Vec::new();
    for property in contract["properties"].as_array().into_iter().flatten() {
        let found = recorded
            .iter()
            .rev()
            .find(|r| r["property_id"] == property["property_id"]);
        let mut entry = match found {
            Some(r) if !(lost && mutants.on("recorded-results-rewritten")) => r.clone(),
            _ if lost && mutants.on("lost-evaluator-reports-pass") => {
                json!({"property_id": property["property_id"], "result": "pass"})
            }
            _ if lost => {
                json!({"property_id": property["property_id"], "result": "indeterminate", "reason": "evaluator_unavailable"})
            }
            _ => {
                json!({"property_id": property["property_id"], "result": "not_evaluated", "reason": "not_reported"})
            }
        };
        entry["evidence"] = json!([]);
        properties.push(entry);
    }
    let content = json!({
        "format": RECEIPT_FORMAT,
        "receipt": id,
        "subjects": job["subjects"],
        "contract": job["contract"],
        "evaluator": {"id": job["evaluator"]["id"], "version": job["evaluator"]["version"], "principal": provider_id},
        "environment": job.get("observed_environment").cloned().unwrap_or_else(|| job["environment"].clone()),
        "inputs": [],
        "properties": properties,
        "observed_from": job.get("started_at").cloned().unwrap_or_else(|| job["submitted_at"].clone()),
        "observed_until": now,
        "valid_until": job.get("valid_until").cloned().unwrap_or(Value::Null),
        "scope": contract["outcome"].as_str().unwrap_or("contract evaluation"),
        "job": id,
        "provenance": [],
    });
    let (reference, receipt_drafts) = store_receipt(
        tx,
        id,
        &content,
        provider_id,
        now,
        "verification.receipt.issued",
        Some(id),
    )?;
    drafts.extend(receipt_drafts);
    job["state"] = json!("completed");
    job["receipt"] = reference;
    *revision += 1;
    drafts.push((
        "verification.job.changed",
        subject(JOB, id),
        *revision,
        json!({"state": "completed", "evaluator": job["evaluator"]}),
    ));
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Queries.

pub fn inspect_job(tx: &Transaction, id: &str) -> rusqlite::Result<Option<Value>> {
    Ok(load(tx, JOB, id)?.map(|(_, job)| {
        let recorded: Vec<Value> = job["recorded"].as_array().into_iter().flatten().cloned().collect();
        let mut out = json!({"job": subject(JOB, id), "state": job["state"], "contract": job["contract"], "subjects": job["subjects"], "evaluator": job["evaluator"], "recorded": recorded});
        if job["receipt"].is_object() {
            out["receipt"] = job["receipt"].clone();
        }
        out
    }))
}

pub fn inspect_receipt(tx: &Transaction, id: &str) -> rusqlite::Result<Option<Value>> {
    let Some((_, record)) = load(tx, RECEIPT, id)? else {
        return Ok(None);
    };
    let content = content_of(tx, &record["reference"])?.unwrap_or(Value::Null);
    Ok(Some(
        json!({"reference": record["reference"], "content": content}),
    ))
}

/// `verification.receipt.assess` (VERIFICATION section 7).
pub fn assess(
    tx: &Transaction,
    payload: &Value,
    ctx: &Context,
) -> rusqlite::Result<Result<Value, Refusal>> {
    let mutants = ctx.mutants;
    let id = payload["receipt"]["receipt"].as_str().unwrap_or_default();
    let Some((_, record)) = load(tx, RECEIPT, id)? else {
        return Ok(Err(("not_found", json!({}))));
    };
    let caller_selected = payload.get("at").is_some();
    let at = payload["at"]
        .as_str()
        .map(String::from)
        .unwrap_or_else(|| ctx.now.clone());
    let mut checks = serde_json::Map::new();
    let set = |checks: &mut serde_json::Map<String, Value>,
               name: &str,
               status: &str,
               reasons: Vec<&str>| {
        checks.insert(
            name.to_string(),
            json!({"status": status, "reasons": reasons}),
        );
    };
    // Receipt.
    let mapped = &record["reference"];
    let mut content = Value::Null;
    if payload["receipt"] != *mapped {
        set(
            &mut checks,
            "receipt",
            "failed",
            vec!["receipt_reference_mismatch"],
        );
    } else {
        let artifact_id = mapped["artifact"]["artifact"]["id"]
            .as_str()
            .unwrap_or_default();
        let readable = match load(tx, crate::evidence::ARTIFACT, artifact_id)? {
            Some((_, artifact)) => {
                let observed = crate::evidence::observed(
                    tx,
                    artifact_id,
                    &artifact,
                    ctx.evidence_config,
                    mutants,
                )?;
                observed["state"] == "available" || mutants.on("receipt-bytes-unchecked")
            }
            None => false,
        };
        if readable {
            content = content_of(tx, mapped)?.unwrap_or(Value::Null);
            set(&mut checks, "receipt", "passed", vec![]);
        } else {
            set(
                &mut checks,
                "receipt",
                "unverifiable",
                vec!["receipt_unavailable"],
            );
        }
    }
    // Contract.
    let contract = read_contract(
        tx,
        &payload["contract"],
        ctx.provider_id,
        ctx.evidence_config,
        mutants,
    )?
    .ok();
    if let Some(contract) = &contract
        && !mutants.on("assess-roles-incomplete-accepted")
        && let Some(refusal) = roles_refusal(contract, &payload["subjects"])
    {
        return Ok(Err(refusal));
    }
    let have_content = content.is_object();
    if contract.is_none() {
        set(
            &mut checks,
            "contract",
            "unverifiable",
            vec!["contract_unavailable"],
        );
    } else if have_content {
        let same = if mutants.on("contract-digest-only") {
            content["contract"]["digest"] == payload["contract"]["digest"]
        } else {
            content["contract"] == payload["contract"]
        };
        set(
            &mut checks,
            "contract",
            if same { "passed" } else { "failed" },
            if same {
                vec![]
            } else {
                vec!["contract_mismatch"]
            },
        );
    } else {
        set(
            &mut checks,
            "contract",
            "unverifiable",
            vec!["receipt_unavailable"],
        );
    }
    if have_content {
        // Subjects.
        let mismatch = !mutants.on("subject-digest-ignored")
            && payload["subjects"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|requested| {
                    !content["subjects"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|s| {
                            s["role"] == requested["role"] && s["digest"] == requested["digest"]
                        })
                });
        set(
            &mut checks,
            "subjects",
            if mismatch { "failed" } else { "passed" },
            if mismatch {
                vec!["subject_mismatch"]
            } else {
                vec![]
            },
        );
        // Environment.
        let mut env_status = "passed";
        let mut env_reasons = vec![];
        if let Some(contract) = &contract {
            for anchor in contract["environment"]["required_anchors"]
                .as_array()
                .into_iter()
                .flatten()
            {
                let name = anchor.as_str().unwrap_or_default();
                let requested = &payload["environment"]["anchors"][name];
                let observed = &content["environment"]["anchors"][name];
                if requested.is_null() || observed.is_null() {
                    if !mutants.on("environment-unverified-satisfied") && env_status == "passed" {
                        env_status = "unverifiable";
                        env_reasons = vec!["environment_unverified"];
                    }
                } else if requested != observed {
                    env_status = "failed";
                    env_reasons = vec!["environment_mismatch"];
                }
            }
        }
        set(&mut checks, "environment", env_status, env_reasons);
        // Evaluator.
        let permitted = contract.as_ref().is_none_or(|c| {
            c["evaluators"]
                .as_array()
                .is_none_or(|list| list.iter().any(|e| *e == content["evaluator"]["id"]))
        });
        set(
            &mut checks,
            "evaluator",
            if permitted { "passed" } else { "failed" },
            if permitted {
                vec![]
            } else {
                vec!["evaluator_not_permitted"]
            },
        );
        // Time.
        let until = content["observed_until"].as_str().unwrap_or_default();
        let mut time_reasons = vec![];
        if at.as_str() < until && !mutants.on("before-observation-ignored") {
            time_reasons.push("before_observation");
        }
        if !mutants.on("stale-ignored") {
            let expired = content["valid_until"]
                .as_str()
                .is_some_and(|v| at.as_str() >= v);
            let too_old = contract
                .as_ref()
                .and_then(|c| c["freshness"]["max_age_seconds"].as_i64())
                .is_some_and(|max| at > crate::execution::add_seconds(until, max));
            if expired || too_old {
                time_reasons.push("stale");
            }
        }
        set(
            &mut checks,
            "time",
            if time_reasons.is_empty() {
                "passed"
            } else {
                "failed"
            },
            time_reasons,
        );
    } else {
        for name in ["subjects", "environment", "evaluator", "time"] {
            set(
                &mut checks,
                name,
                "unverifiable",
                vec!["receipt_unavailable"],
            );
        }
    }
    let mut reasons: Vec<Value> = Vec::new();
    let mut blocking = false;
    for name in [
        "receipt",
        "contract",
        "subjects",
        "environment",
        "evaluator",
        "time",
    ] {
        let check = &checks[name];
        if check["status"] != "passed" {
            blocking = true;
        }
        for reason in check["reasons"].as_array().into_iter().flatten() {
            if !reasons.contains(reason) {
                reasons.push(reason.clone());
            }
        }
    }
    let mut properties = Vec::new();
    if let (Some(contract), true) = (&contract, have_content) {
        for property in contract["properties"].as_array().into_iter().flatten() {
            let result = content["properties"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|p| p["property_id"] == property["property_id"])
                .map(|p| p["result"].clone())
                .unwrap_or(json!("not_evaluated"));
            let (status, property_reasons) = if blocking {
                ("not_satisfied", reasons.clone())
            } else {
                match result.as_str() {
                    Some("pass") => ("satisfied", vec![]),
                    Some("fail") => ("failed", vec![]),
                    Some("not_evaluated") if mutants.on("not-evaluated-satisfied") => {
                        ("satisfied", vec![])
                    }
                    _ => ("not_satisfied", vec![result.clone()]),
                }
            };
            properties.push(json!({"property_id": property["property_id"], "required": property["required"].as_bool().unwrap_or(true), "result": result, "status": status, "reasons": property_reasons}));
        }
    }
    let required: Vec<&Value> = properties
        .iter()
        .filter(|p| p["required"] == true)
        .collect();
    let overall = if properties.is_empty() {
        "not_satisfied"
    } else if required.iter().any(|p| p["status"] == "failed") {
        "failed"
    } else if required.iter().any(|p| p["status"] != "satisfied") {
        "not_satisfied"
    } else {
        "satisfied"
    };
    let present = overall == "satisfied" && (!caller_selected || mutants.on("time-basis-ignored"));
    Ok(Ok(json!({
        "assessed_at": at,
        "time_basis": if caller_selected { "caller_selected" } else { "provider_clock" },
        "present_validity": if present { "established" } else { "not_established" },
        "basis": {"subjects": payload["subjects"], "environment": payload.get("environment").cloned().unwrap_or(Value::Null)},
        "checks": checks,
        "properties": properties,
        "overall": overall,
        "reasons": reasons,
    })))
}

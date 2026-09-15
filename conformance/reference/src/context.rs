//! Reference provider for `context/1` (CONTEXT draft, owner decisions M4-Q2 to M4-Q4).
//!
//! Requests, jobs and packets are Core subjects. Preparation follows the scripted steps in launch
//! configuration `context`; the script is test environment, never protocol. Each packet revision
//! is sealed as an Evidence artifact in this provider's own store (CONTEXT section 1).

use rusqlite::{Transaction, params};
use serde_json::{Value, json};

use crate::evidence;
use crate::execution::{Applied, Draft, provider_event};
use crate::mutants::Mutants;
use crate::store::Store;

pub const REQUEST: &str = "context.request";
pub const JOB: &str = "context.job";
pub const PACKET: &str = "context.packet";
pub const PACKET_MEDIA_TYPE: &str = "application/vnd.combraton.context-packet+json";
pub const PACKET_FORMAT: &str = "combraton-context-packet/1";
/// Packets for requests submitted under `context.claims` (CONTEXT section 14).
pub const CLAIMS_PACKET_FORMAT: &str = "combraton-context-packet/2";
const COMPILER: &str = "combraton-reference-context";

/// Features of the submitting session that change how a request is prepared.
pub struct Session<'a> {
    pub principal: &'a str,
    pub shared_jobs: bool,
    pub updates: bool,
    /// The session negotiated `context.claims` (CONTEXT section 14).
    pub claims: bool,
    pub script: &'a Value,
    pub mutants: &'a Mutants,
}

fn subject(kind: &str, id: &str) -> Value {
    json!({"kind": kind, "id": id})
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
        params![kind, id, revision, record.to_string()],
    )?;
    Ok(())
}

fn ids(tx: &Transaction, kind: &str) -> rusqlite::Result<Vec<String>> {
    let mut statement = tx.prepare("SELECT id FROM subjects WHERE kind=?1 ORDER BY rowid")?;
    let rows = statement.query_map([kind], |r| r.get(0))?;
    rows.collect()
}

/// The script for a job started by `request`.
fn script_for<'a>(config: &'a Value, request: &str) -> &'a Value {
    match config["scripts"].get(request) {
        Some(script) => script,
        None => &config["default_script"],
    }
}

fn required(item: &Value) -> bool {
    item["obligation"] != "advisory"
}

/// Content bytes the scripted sections for the request's required items need (CTX-8).
fn mandatory_size(script: &Value, items: &[Value]) -> i64 {
    script
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|step| step.get("section"))
        .filter(|section| {
            items
                .iter()
                .any(|item| required(item) && section["item_id"] == item["item_id"])
        })
        .map(|section| section["content"].as_str().map_or(0, str::len) as i64)
        .sum()
}

/// Profile checks on a submit payload that the schema cannot express (CONTEXT section 3).
pub fn validate_submit(payload: &Value, mutants: &Mutants) -> Result<(), (String, &'static str)> {
    for (index, item) in payload["items"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let transition = item.get("transition").is_some();
        if transition != (item["obligation"] == "required_before_transition") {
            return Err((
                format!("/payload/items/{index}/transition"),
                "transition is named exactly for required_before_transition",
            ));
        }
    }
    let commit_only = payload["basis"]["repositories"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|r| r["workspace"] != "clean" && r["dirty"].is_null());
    if commit_only
        && payload["basis"]["completeness"] == "complete"
        && !mutants.on("dirty-basis-complete-accepted")
    {
        return Err((
            "/payload/basis/completeness".to_string(),
            "a commit-only basis for a workspace that is not clean is partial",
        ));
    }
    for (index, content) in payload["authority_content"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        if !payload["items"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|i| i["item_id"] == content["item_id"])
        {
            return Err((
                format!("/payload/authority_content/{index}/item_id"),
                "authority content names no item of this request",
            ));
        }
    }
    Ok(())
}

/// Obligation features a submit needs (CTX-9), and `context.claims` for claim checks.
pub fn obligation_features(payload: &Value) -> Vec<String> {
    let mut features: Vec<String> = Vec::new();
    if payload["items"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|item| item["check"]["kind"] == "claim_included")
    {
        features.push("context.claims".to_string());
    }
    for item in payload["items"].as_array().into_iter().flatten() {
        let feature = format!(
            "context.{}",
            item["obligation"].as_str().unwrap_or_default()
        );
        if !features.contains(&feature) {
            features.push(feature);
        }
    }
    features
}

/// `context.request.submit` inside the owner transaction.
pub fn submit(
    tx: &Transaction,
    id: &str,
    payload: &Value,
    session: &Session,
) -> rusqlite::Result<Applied> {
    let mutants = session.mutants;
    let request_subject = subject(REQUEST, id);
    let items: Vec<Value> = payload["items"].as_array().cloned().unwrap_or_default();
    let mut record = json!({
        "consumer": payload["consumer"],
        "basis": payload["basis"],
        "items": items,
        "limits": payload["limits"],
        "fallback": payload.get("fallback").cloned().unwrap_or(json!("proceed_with_gap")),
        "authority_content": payload.get("authority_content").cloned().unwrap_or(json!([])),
        "submitted_by": session.principal,
        "updates": session.updates,
        "claims": session.claims,
        "packets": [],
    });
    if let Some(origin) = payload.get("origin") {
        record["origin"] = origin.clone();
    }
    let capacity = payload["limits"]["output_capacity"]["amount"]
        .as_i64()
        .unwrap_or(0);
    let script = script_for(session.script, id);
    let needed = mandatory_size(script, &items);
    if needed > capacity && !mutants.on("mandatory-items-dropped") {
        record["state"] = json!("refused");
        record["reason"] = json!("budget_insufficient");
        record["needed"] = json!({"units": "bytes", "amount": needed});
        save(tx, REQUEST, id, 1, &record)?;
        let outcome = json!({"request": request_subject, "state": "refused", "reason": "budget_insufficient", "needed": record["needed"]});
        let events: Vec<Draft> = vec![(
            "context.request.changed",
            request_subject,
            1,
            json!({"state": "refused", "reason": "budget_insufficient"}),
        )];
        return Ok((1, outcome, events, Vec::new()));
    }
    let mut job_id = None;
    if session.shared_jobs {
        for other in ids(tx, JOB)? {
            if let Some((revision, mut job)) = evidence::load(tx, JOB, &other)?
                && job["state"] == "running"
                && job["published"] != true
                && (job["principal"] == session.principal
                    || mutants.on("shared-job-across-principals"))
                && job["basis"] == payload["basis"]
                && job["items"] == payload["items"]
            {
                if let Some(list) = job["requests"].as_array_mut() {
                    list.push(json!(id));
                }
                save(tx, JOB, &other, revision + 1, &job)?;
                job_id = Some(other);
                break;
            }
        }
    }
    let job_id = match job_id {
        Some(job) => job,
        None => {
            let job = json!({
                "state": "running",
                "principal": session.principal,
                "requests": [id],
                "basis": payload["basis"],
                "items": payload["items"],
                "limits": payload["limits"],
                "script": script,
                "cursor": 0,
                "spent": 0,
                "sections": [],
                "coverage": [],
                "unmet": {},
                "omissions": [],
                "corrections": {},
                "conditions": [],
            });
            save(tx, JOB, id, 1, &job)?;
            id.to_string()
        }
    };
    record["state"] = json!("preparing");
    record["job"] = json!(job_id);
    save(tx, REQUEST, id, 1, &record)?;
    let job_subject = subject(JOB, &job_id);
    let outcome = json!({"request": request_subject, "state": "preparing", "job": job_subject});
    let events: Vec<Draft> = vec![(
        "context.request.changed",
        request_subject,
        1,
        json!({"state": "preparing", "job": job_subject}),
    )];
    Ok((1, outcome, events, Vec::new()))
}

/// `context.request.cancel`: that request only (CTX-10).
pub fn cancel(tx: &Transaction, id: &str, session: &Session) -> rusqlite::Result<Applied> {
    let request_subject = subject(REQUEST, id);
    let (revision, mut record) = evidence::load(tx, REQUEST, id)?.unwrap_or((0, json!({})));
    let job_id = record["job"].as_str().unwrap_or_default().to_string();
    let job_subject = subject(JOB, &job_id);
    let mut events: Vec<Draft> = Vec::new();
    let (job_revision, mut job) = evidence::load(tx, JOB, &job_id)?.unwrap_or((0, json!({})));
    let mut others: Vec<Value> = job["requests"].as_array().cloned().unwrap_or_default();
    others.retain(|r| r != id);
    let job_continues = !others.is_empty() && !session.mutants.on("cancel-ends-job");
    job["requests"] = json!(others);
    let revision = revision + 1;
    record["state"] = json!("cancelled");
    save(tx, REQUEST, id, revision, &record)?;
    events.push((
        "context.request.cancelled",
        request_subject.clone(),
        revision,
        json!({"job_continues": job_continues}),
    ));
    if !job_continues {
        job["state"] = json!("ended");
        let reason = if others.is_empty() {
            "no_subscribers"
        } else {
            "cancelled"
        };
        job["reason"] = json!(reason);
        events.push((
            "context.job.ended",
            job_subject.clone(),
            job_revision + 1,
            json!({"reason": reason}),
        ));
        for other in others {
            let other = other.as_str().unwrap_or_default();
            if let Some((other_revision, mut other_record)) = evidence::load(tx, REQUEST, other)? {
                other_record["state"] = json!("cancelled");
                save(tx, REQUEST, other, other_revision + 1, &other_record)?;
                events.push((
                    "context.request.changed",
                    subject(REQUEST, other),
                    other_revision + 1,
                    json!({"state": "cancelled", "reason": "job_ended"}),
                ));
            }
        }
    }
    save(tx, JOB, &job_id, job_revision + 1, &job)?;
    let outcome = json!({"request": request_subject, "state": "cancelled", "job": job_subject, "job_continues": job_continues});
    Ok((revision, outcome, events, Vec::new()))
}

/// Whether a request exists and is still being prepared (the target of cancel).
pub fn preparing(tx: &Transaction, id: &str) -> rusqlite::Result<Option<bool>> {
    Ok(evidence::load(tx, REQUEST, id)?.map(|(_, r)| r["state"] == "preparing"))
}

/// Advance every running job against the provider clock, one owner transaction per job. When a
/// packet cannot be sealed at the evidence provider, that job's step rolls back and is retried at
/// a later tick: a packet is never reported before its artifact is sealed.
///
/// `publisher` is `{ provider_id, evidence_provider? }`; without an evidence provider the packets
/// are sealed in this provider's own store.
pub fn tick(
    store: &mut Store,
    now: &str,
    publisher: &Value,
    mutants: &Mutants,
) -> rusqlite::Result<()> {
    let stream = store.stream_id()?;
    let jobs = {
        let tx = store.transaction()?;
        ids(&tx, JOB)?
    };
    for job_id in jobs {
        let tx = store.transaction()?;
        let Some((mut revision, mut job)) = evidence::load(&tx, JOB, &job_id)? else {
            continue;
        };
        if job["state"] != "running" {
            continue;
        }
        let before = job.clone();
        let mut drafts: Vec<Draft> = Vec::new();
        match advance(&tx, now, publisher, mutants, &job_id, &mut job, &mut drafts) {
            // A peer failure travels as a conversion failure so it is told apart from storage errors.
            Err(rusqlite::Error::ToSqlConversionFailure(reason)) => {
                eprintln!("context job {job_id}: {reason}; retrying later");
                continue;
            }
            other => other?,
        }
        if job != before {
            revision += 1;
            save(&tx, JOB, &job_id, revision, &job)?;
        }
        for draft in drafts {
            provider_event(&tx, &stream, now, draft)?;
        }
        tx.commit()?;
    }
    Ok(())
}

fn advance(
    tx: &Transaction,
    now: &str,
    publisher: &Value,
    mutants: &Mutants,
    job_id: &str,
    job: &mut Value,
    drafts: &mut Vec<Draft>,
) -> rusqlite::Result<()> {
    loop {
        let cursor = job["cursor"].as_u64().unwrap_or(0) as usize;
        let Some(step) = job["script"].get(cursor).cloned() else {
            break;
        };
        let (name, argument) = step
            .as_object()
            .and_then(|m| m.iter().next())
            .map(|(k, v)| (k.clone(), v.clone()))
            .unwrap_or_default();
        match name.as_str() {
            "wait_until" => {
                if now < argument.as_str().unwrap_or_default() {
                    break;
                }
            }
            "stall" => break,
            "investigate" => {
                let spent = job["spent"].as_i64().unwrap_or(0) + argument.as_i64().unwrap_or(0);
                job["spent"] = json!(spent);
                if spent
                    > job["limits"]["investigation"]["amount"]
                        .as_i64()
                        .unwrap_or(0)
                {
                    let reason = if mutants.on("limits-collapsed") {
                        "deadline_passed"
                    } else {
                        "investigation_budget_exhausted"
                    };
                    finish_job(tx, now, publisher, mutants, job_id, job, reason, drafts)?;
                    return Ok(());
                }
            }
            "section" => {
                let mut section = argument.clone();
                section["historical"] = json!(false);
                if let Some(reference) = argument.get("claim") {
                    let basis = job["basis"].clone();
                    let wanted = if mutants.on("claim-latest-substituted") {
                        json!({"provider": reference["provider"], "claim": reference["claim"]})
                    } else {
                        reference.clone()
                    };
                    let read = read_claim(tx, publisher, &wanted);
                    match claim_snapshot(read, reference, &basis, mutants) {
                        Ok(snapshot) => {
                            let accepted_binding = snapshot["reliance"]["state"]
                                == "accepted_for_use"
                                && snapshot["reliance"]["permitted_use"] == "binding";
                            if section["label"] == "binding"
                                && !accepted_binding
                                && !mutants.on("label-promotes-claim")
                            {
                                section["label"] = json!("hypothesis");
                            }
                            let result = snapshot["applicability"]["result"].clone();
                            if result == "invalid_for_target"
                                || (result != "applicable"
                                    && mutants.on("unknown-applicability-marked-stale"))
                            {
                                section["historical"] = json!(true);
                                section["label"] = json!("stale");
                            }
                            let mut snapshot = snapshot;
                            if let Some(map) = snapshot.as_object_mut() {
                                map.remove("current_revision");
                            }
                            section["claim"] = snapshot;
                        }
                        Err(reason) => {
                            section["claim_error"] = json!(reason);
                        }
                    }
                }
                if let (Some(item), Some(revision)) = (
                    section["item_id"].as_str(),
                    section["authority_revision"].as_i64(),
                ) && job["corrections"][item]
                    .as_i64()
                    .is_some_and(|corrected| corrected > revision)
                    && !mutants.on("stale-derivation-current")
                {
                    section["historical"] = json!(true);
                    section["label"] = json!("stale");
                }
                if let Some(list) = job["sections"].as_array_mut() {
                    list.push(section);
                }
            }
            "coverage" => {
                if let Some(list) = job["coverage"].as_array_mut() {
                    list.push(argument.clone());
                }
            }
            "unmet" => {
                let item = argument["item_id"].as_str().unwrap_or_default().to_string();
                job["unmet"][item] = argument["reason"].clone();
            }
            "omit" => {
                if let Some(list) = job["omissions"].as_array_mut() {
                    list.push(argument.clone());
                }
            }
            "correction" => {
                let item = argument["item_id"].as_str().unwrap_or_default().to_string();
                let corrected = argument["authority_revision"].as_i64().unwrap_or(0);
                job["corrections"][&item] = json!(corrected);
                if !mutants.on("stale-derivation-current") {
                    for section in job["sections"].as_array_mut().into_iter().flatten() {
                        if section["item_id"] == item.as_str()
                            && section["authority_revision"]
                                .as_i64()
                                .is_some_and(|r| r < corrected)
                        {
                            section["historical"] = json!(true);
                            section["label"] = json!("stale");
                        }
                    }
                }
            }
            "conditions" => {
                job["conditions"] = argument.clone();
            }
            "execute" => {
                // The job's own investigation runs as an ordinary execution at a separate
                // executor, with its origin, and is never enriched with context (CTX-18, CTX-19).
                if !investigate(publisher, mutants, job_id, job, &argument) {
                    break;
                }
            }
            "publish" => {
                if !publish_all(
                    tx, now, publisher, mutants, job_id, job, false, None, drafts,
                )? {
                    break;
                }
            }
            "end" => {
                let reason = argument.as_str().unwrap_or("ended").to_string();
                finish_job(tx, now, publisher, mutants, job_id, job, &reason, drafts)?;
                return Ok(());
            }
            _ => {}
        }
        job["cursor"] = json!(cursor + 1);
    }
    // Deadline: required items still missing are unmet; advisory items degrade (CTX-7).
    for request in job["requests"].as_array().cloned().unwrap_or_default() {
        let request = request.as_str().unwrap_or_default();
        if let Some((_, record)) = evidence::load(tx, REQUEST, request)?
            && record["state"] == "preparing"
            && now >= record["limits"]["deadline"].as_str().unwrap_or(now)
        {
            publish_one(
                tx,
                now,
                publisher,
                mutants,
                job,
                request,
                true,
                Some("deadline_passed"),
                drafts,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn finish_job(
    tx: &Transaction,
    now: &str,
    publisher: &Value,
    mutants: &Mutants,
    job_id: &str,
    job: &mut Value,
    reason: &str,
    drafts: &mut Vec<Draft>,
) -> rusqlite::Result<()> {
    publish_all(
        tx,
        now,
        publisher,
        mutants,
        job_id,
        job,
        true,
        Some(reason),
        drafts,
    )?;
    job["state"] = json!("ended");
    job["reason"] = json!(reason);
    let revision = evidence::load(tx, JOB, job_id)?.map_or(1, |(r, _)| r) + 1;
    drafts.push((
        "context.job.ended",
        subject(JOB, job_id),
        revision,
        json!({"reason": reason}),
    ));
    Ok(())
}

/// Publish for every subscriber. Returns false when a `wait_until_deadline` fallback holds it.
#[allow(clippy::too_many_arguments)]
fn publish_all(
    tx: &Transaction,
    now: &str,
    publisher: &Value,
    mutants: &Mutants,
    _job_id: &str,
    job: &mut Value,
    finishing: bool,
    reason: Option<&str>,
    drafts: &mut Vec<Draft>,
) -> rusqlite::Result<bool> {
    let requests: Vec<String> = job["requests"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(String::from)
        .collect();
    if !finishing {
        for request in &requests {
            if let Some((_, record)) = evidence::load(tx, REQUEST, request)?
                && record["state"] == "preparing"
                && record["fallback"] == "wait_until_deadline"
                && now < record["limits"]["deadline"].as_str().unwrap_or(now)
                && item_results(&record, job, false, None, mutants)
                    .iter()
                    .any(|i| i["obligation"] == "advisory" && i["result"] != "satisfied")
            {
                return Ok(false);
            }
        }
    }
    for request in &requests {
        publish_one(
            tx, now, publisher, mutants, job, request, true, reason, drafts,
        )?;
    }
    job["published"] = json!(true);
    Ok(true)
}

/// Item results for a request from the job's current state.
fn item_results(
    record: &Value,
    job: &Value,
    finishing: bool,
    reason: Option<&str>,
    mutants: &Mutants,
) -> Vec<Value> {
    let included = inclusion(record, job, mutants).0;
    record["items"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| {
            let item_id = item["item_id"].as_str().unwrap_or_default();
            let obligation = if item["obligation"] == "required_before_transition"
                && mutants.on("transition-as-advisory")
            {
                json!("advisory")
            } else {
                item["obligation"].clone()
            };
            let corrected = job["corrections"].get(item_id).is_some();
            let omitted = job["sections"].as_array().into_iter().flatten().any(|s| {
                s["item_id"] == item_id
                    && s["historical"] == false
                    && !included.contains(&s["section_id"])
            });
            let satisfied = job["sections"].as_array().into_iter().flatten().any(|s| {
                s["item_id"] == item_id
                    && (s["historical"] == false || mutants.on("stale-derivation-current"))
                    && included.contains(&s["section_id"])
                    && check_passes(item, s, record, job, mutants)
            });
            let mut result = json!({"item_id": item_id, "obligation": obligation});
            // A scripted unmet reason never overrides a satisfied item (CONTEXT section 12).
            let scripted = job["unmet"]
                .get(item_id)
                .filter(|_| !satisfied || mutants.on("scripted-unmet-overrides-satisfied"));
            if let Some(unmet) = scripted {
                result["result"] = json!("unmet");
                result["reason"] = unmet.clone();
            } else if satisfied {
                result["result"] = json!("satisfied");
            } else if !finishing {
                result["result"] = json!("pending");
            } else {
                let claim_cause = claim_reason(item, job, mutants);
                let cause = if let Some(claim) = claim_cause {
                    claim
                } else if omitted {
                    "output_capacity"
                } else if corrected {
                    "corrected_during_preparation"
                } else {
                    reason.unwrap_or("unavailable")
                };
                if obligation == "advisory" {
                    result["result"] = json!("degraded");
                } else if mutants.on("missing-required-satisfied") {
                    result["result"] = json!("satisfied");
                    return result;
                } else if mutants.on("required-downgraded-to-advisory") {
                    result["obligation"] = json!("advisory");
                    result["result"] = json!("degraded");
                } else {
                    result["result"] = json!("unmet");
                }
                result["reason"] = json!(cause);
            }
            result
        })
        .collect()
}

/// Whether a section meets the item's check (CONTEXT section 3): satisfaction is decided by the
/// check, not by the section's presence.
fn check_passes(
    item: &Value,
    section: &Value,
    record: &Value,
    job: &Value,
    mutants: &Mutants,
) -> bool {
    if mutants.on("check-ignored") {
        return true;
    }
    let check = &item["check"];
    match check["kind"].as_str() {
        Some("source_included") => {
            section["source"]["repository"] == check["repository"]
                && section["source"]["path"] == check["path"]
        }
        Some("evidence_included") => {
            section["citations"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|c| {
                    c["evidence"]["artifact"] == check["evidence"]["artifact"]
                        && c["evidence"]["digest"] == check["evidence"]["digest"]
                })
        }
        Some("claim_included") => {
            section["claim"]["reference"] == check["claim"]
                && claim_shortfall(item, &section["claim"]).is_none()
        }
        Some("authority_content_included") => {
            let item_id = item["item_id"].as_str().unwrap_or_default();
            let supplied = record["authority_content"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|c| c["item_id"] == item_id)
                .map(|c| c["authority_revision"].clone());
            let Some(supplied) = supplied else {
                return false;
            };
            let current = job["corrections"].get(item_id).cloned().unwrap_or(supplied);
            section["authority_revision"] == current || mutants.on("stale-derivation-current")
        }
        _ => false,
    }
}

/// Which sections fit the output capacity, and the omissions that follows (CTX-4, CTX-8).
fn inclusion(record: &Value, job: &Value, mutants: &Mutants) -> (Vec<Value>, Vec<Value>) {
    let mut remaining = record["limits"]["output_capacity"]["amount"]
        .as_i64()
        .unwrap_or(0);
    if mutants.on("limits-collapsed") {
        remaining -= job["spent"].as_i64().unwrap_or(0);
    }
    let items = record["items"].as_array().cloned().unwrap_or_default();
    let mut included = Vec::new();
    let mut omissions: Vec<Value> = job["omissions"].as_array().cloned().unwrap_or_default();
    for section in job["sections"].as_array().into_iter().flatten() {
        // A claim that could not be read or verified is never carried (CONTEXT section 14).
        if section.get("claim_error").is_some() {
            let mut omission =
                json!({"section_id": section["section_id"], "reason": "unavailable"});
            if let Some(item) = section.get("item_id") {
                omission["item_id"] = item.clone();
            }
            omissions.push(omission);
            continue;
        }
        let size = section["content"].as_str().map_or(0, str::len) as i64;
        let mandatory = items
            .iter()
            .any(|i| required(i) && i["item_id"] == section["item_id"]);
        if mandatory && !mutants.on("mandatory-items-dropped") || size <= remaining {
            remaining -= size;
            included.push(section["section_id"].clone());
        } else {
            let mut omission =
                json!({"section_id": section["section_id"], "reason": "output_capacity"});
            if let Some(item) = section.get("item_id") {
                omission["item_id"] = item.clone();
            }
            omissions.push(omission);
        }
    }
    (included, omissions)
}

/// Publish the next packet revision for one request, if it is due one.
#[allow(clippy::too_many_arguments)]
fn publish_one(
    tx: &Transaction,
    now: &str,
    publisher: &Value,
    mutants: &Mutants,
    job: &Value,
    request: &str,
    finishing: bool,
    reason: Option<&str>,
    drafts: &mut Vec<Draft>,
) -> rusqlite::Result<()> {
    let Some((revision, mut record)) = evidence::load(tx, REQUEST, request)? else {
        return Ok(());
    };
    let published = record["packets"].as_array().map_or(0, Vec::len);
    match record["state"].as_str() {
        Some("preparing") => {}
        Some("ready" | "partial" | "unmet") if record["updates"] == true => {}
        _ => return Ok(()),
    }
    let past_deadline = now >= record["limits"]["deadline"].as_str().unwrap_or(now);
    let reason = reason.or(past_deadline.then_some("deadline_passed"));
    let items = item_results(&record, job, finishing, reason, mutants);
    let (included, omissions) = inclusion(&record, job, mutants);
    let state = if items.iter().any(|i| i["result"] == "unmet") {
        "unmet"
    } else if items.iter().any(|i| i["result"] == "degraded") {
        "partial"
    } else {
        "ready"
    };
    let packet_revision = if mutants.on("packet-mutated-in-place") && published > 0 {
        published
    } else {
        published + 1
    };
    let artifact_id = format!("packet.{request}.{}", published + 1);
    let sections: Vec<Value> = job["sections"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| included.contains(&s["section_id"]))
        .map(|s| {
            let mut out = json!({"section_id": s["section_id"], "label": s["label"], "historical": s["historical"],
                "content": s["content"], "citations": s.get("citations").cloned().unwrap_or(json!([]))});
            if let Some(item) = s.get("item_id") {
                out["item_id"] = item.clone();
            }
            if record["claims"] == true && s["claim"].is_object() {
                out["claim"] = s["claim"].clone();
            }
            out
        })
        .collect();
    let coverage: Vec<Value> = if mutants.on("global-coverage-cursor") {
        let frontier = job["coverage"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| c["frontier"].as_str())
            .max()
            .unwrap_or("none");
        vec![json!({"producer": "all", "frontier": frontier, "gaps": []})]
    } else if mutants.on("unobserved-frontier-claimed") {
        job["coverage"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|c| json!({"producer": c["producer"], "frontier": "head", "gaps": []}))
            .collect()
    } else {
        job["coverage"].as_array().cloned().unwrap_or_default()
    };
    let corrections = &job["corrections"];
    let authority: Vec<Value> = record["authority_content"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|content| {
            let item = content["item_id"].as_str().unwrap_or_default();
            let mut revision = corrections.get(item).cloned().unwrap_or(content["authority_revision"].clone());
            if mutants.on("authority-revision-dropped") {
                revision = json!(0);
            }
            json!({"item_id": item, "authority_revision": revision, "evidence": content["evidence"], "coverage": coverage})
        })
        .collect();
    let applicability = json!({"basis": record["basis"], "conditions": job["conditions"]});
    let format = if record["claims"] == true || mutants.on("claims-format-without-negotiation") {
        CLAIMS_PACKET_FORMAT
    } else {
        PACKET_FORMAT
    };
    let mut body = json!({
        "format": format,
        "request": request,
        "revision": packet_revision,
        "sections": sections,
        "items": items,
        "coverage": coverage,
        "omissions": omissions,
        "applicability": applicability,
        "authority": authority,
    });
    if packet_revision > 1 {
        body["supersedes"] = json!({"revision": packet_revision - 1});
    }
    let content = crate::json::canonical(&body, crate::json::CanonicalFlaws::default());
    let digest = crate::json::sha256_digest(&content);
    let local = publisher["provider_id"].as_str().unwrap_or_default();
    let artifact_provider = match publisher.get("evidence_provider") {
        None => {
            drafts.extend(evidence::publish_sealed(
                tx,
                &artifact_id,
                local,
                request,
                &content,
                PACKET_MEDIA_TYPE,
                now,
            )?);
            local.to_string()
        }
        Some(peer) => {
            let descriptor = json!({
                "digest": digest,
                "size": content.len(),
                "media_type": PACKET_MEDIA_TYPE,
                "producer": {"producer_id": "reference-context-provider"},
                "source": {"kind": "context.packet", "id": request},
                "scope": "context",
                "capture": {"captured_at": now, "anchors": []},
                "coverage": {"completeness": "complete"},
                "retention_class": "context-packet",
            });
            match crate::peer::publish_artifact(peer, &artifact_id, descriptor, &content) {
                Ok(provider) => provider,
                Err(_) if mutants.on("packet-reported-before-seal") => {
                    peer["provider_id"].as_str().unwrap_or_default().to_string()
                }
                Err(failure) => {
                    return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                        std::io::Error::other(format!(
                            "packet {artifact_id} not sealed at the evidence provider: {}",
                            failure.describe()
                        )),
                    )));
                }
            }
        }
    };
    let reference = json!({
        "packet": subject(PACKET, request),
        "revision": packet_revision,
        "artifact": {"provider": artifact_provider, "artifact": subject(evidence::ARTIFACT, &artifact_id), "digest": digest},
    });
    let citations: Vec<Value> = job["sections"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|s| s["citations"].as_array().cloned().unwrap_or_default())
        .collect();
    let selected: Vec<Value> = job["sections"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s.get("source").cloned())
        .chain(citations.iter().map(|c| json!({"evidence": c["evidence"]})))
        .collect();
    let facts = json!({
        "reference": reference,
        "request": subject(REQUEST, request),
        "job": subject(JOB, record["job"].as_str().unwrap_or_default()),
        "selected": selected,
        "provenance": {"compiler": COMPILER, "job": subject(JOB, record["job"].as_str().unwrap_or_default()), "basis": record["basis"]},
        "sections": job["sections"].as_array().into_iter().flatten().filter(|s| included.contains(&s["section_id"])).map(|s| {
            let mut out = json!({"section_id": s["section_id"], "label": s["label"], "historical": s["historical"],
                "length": s["content"].as_str().map_or(0, str::len),
                "citations": s["citations"].as_array().into_iter().flatten().map(|c| c["citation_id"].clone()).collect::<Vec<_>>()});
            if let Some(item) = s.get("item_id") {
                out["item_id"] = item.clone();
            }
            if record["claims"] == true && s["claim"].is_object() {
                out["claim"] = s["claim"].clone();
            }
            if mutants.on("unlabeled-section") {
                out.as_object_mut().map(|o| o.remove("label"));
            }
            out
        }).collect::<Vec<_>>(),
        "inclusions": included,
        "omissions": body["omissions"],
        "citations": citations,
        "coverage": coverage,
        "items": body["items"],
        "applicability": applicability,
        "authority": authority,
        "supersedes": body.get("supersedes").cloned().unwrap_or(Value::Null),
        "body": body,
        "content_base64": crate::json::base64(&content),
    });
    if let Some(packets) = record["packets"].as_array_mut() {
        if packet_revision <= published {
            packets[packet_revision - 1] = facts;
        } else {
            packets.push(facts);
        }
    }
    let state_changed = record["state"] != state;
    record["state"] = json!(state);
    let revision = revision + 1;
    save(tx, REQUEST, request, revision, &record)?;
    drafts.push((
        "context.packet.published",
        subject(REQUEST, request),
        revision,
        json!({"reference": reference, "items": items}),
    ));
    if state_changed {
        drafts.push((
            "context.request.changed",
            subject(REQUEST, request),
            revision,
            json!({"state": state}),
        ));
    }
    Ok(())
}

/// `context.request.inspect`.
pub fn inspect_request(
    store: &Store,
    id: &str,
    mutants: &Mutants,
) -> rusqlite::Result<Option<Value>> {
    let tx = store.reader()?;
    let Some((revision, record)) = evidence::load(&tx, REQUEST, id)? else {
        return Ok(None);
    };
    let job = match record["job"].as_str() {
        Some(job) => evidence::load(&tx, JOB, job)?
            .map(|(_, j)| j)
            .unwrap_or(json!({})),
        None => json!({}),
    };
    let items = if record["state"] == "preparing" {
        item_results(&record, &job, false, None, mutants)
    } else {
        record["packets"]
            .as_array()
            .and_then(|p| p.last())
            .map(|p| p["items"].as_array().cloned().unwrap_or_default())
            .unwrap_or_else(|| item_results(&record, &job, false, None, mutants))
    };
    let packets = record["packets"].as_array().cloned().unwrap_or_default();
    let count = packets.len();
    let listed: Vec<Value> = packets
        .iter()
        .enumerate()
        .map(|(index, facts)| {
            let mut entry = json!({"reference": facts["reference"], "current": index + 1 == count || mutants.on("old-revision-relabeled-current")});
            if !facts["supersedes"].is_null() {
                entry["supersedes"] = facts["supersedes"].clone();
            }
            entry
        })
        .collect();
    let mut result = json!({
        "request": subject(REQUEST, id),
        "revision": revision,
        "state": record["state"],
        "consumer": record["consumer"],
        "limits": record["limits"],
        "items": items,
        "packets": listed,
    });
    for key in ["reason", "needed"] {
        if let Some(value) = record.get(key) {
            result[key] = value.clone();
        }
    }
    if let Some(job) = record["job"].as_str() {
        result["job"] = subject(JOB, job);
    }
    Ok(Some(result))
}

/// Facts of one packet revision, if published.
fn facts(
    tx: &Transaction,
    packet: &str,
    revision: i64,
    reader: &PacketReader,
    mutants: &Mutants,
) -> rusqlite::Result<Option<(Value, bool)>> {
    let Some((_, record)) = evidence::load(tx, REQUEST, packet)? else {
        return Ok(None);
    };
    let packets = record["packets"].as_array().cloned().unwrap_or_default();
    if revision < 1 || revision as usize > packets.len() {
        return Ok(None);
    }
    let mut facts = packets[revision as usize - 1].clone();
    if mutants.on("revision-remapped") {
        facts["reference"]["artifact"] = packets
            .last()
            .map(|p| p["reference"]["artifact"].clone())
            .unwrap_or_default();
    }
    let current =
        revision as usize == packets.len() || mutants.on("old-revision-relabeled-current");
    // Read-time facts about later changes; the published facts themselves never change.
    if !current {
        facts["superseded_by"] = json!({"revision": packets.len()});
    }
    let job = match record["job"].as_str() {
        Some(job) => evidence::load(tx, JOB, job)?
            .map(|(_, j)| j)
            .unwrap_or(Value::Null),
        None => Value::Null,
    };
    let invalidated: Vec<Value> = facts["authority"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let item = entry["item_id"].as_str().unwrap_or_default();
            let corrected = job["corrections"][item].as_i64()?;
            (corrected > entry["authority_revision"].as_i64().unwrap_or(0))
                .then(|| json!({"item_id": item, "authority_revision": corrected}))
        })
        .collect();
    facts["invalidated_items"] = json!(invalidated);
    if reader.claims {
        read_time_claims(tx, &record, &mut facts, reader, mutants);
    } else if !mutants.on("claims-without-negotiation") {
        for section in facts["sections"].as_array_mut().into_iter().flatten() {
            if let Some(map) = section.as_object_mut() {
                map.remove("claim");
            }
        }
    }
    Ok(Some((facts, current)))
}

/// How a packet read reaches claims: the session's `context.claims` and the provider's knowledge
/// source (its own store, or `knowledge_provider`).
pub struct PacketReader<'a> {
    pub claims: bool,
    pub publisher: &'a Value,
}

/// Read a claim revision as an ordinary reader: at the configured knowledge provider, or in this
/// provider's own knowledge store. `Err` means the knowledge could not be read.
fn read_claim(
    tx: &Transaction,
    publisher: &Value,
    reference: &Value,
) -> Result<Value, &'static str> {
    let mut payload = json!({"claim": reference["claim"]});
    if let Some(revision) = reference.get("revision") {
        payload["revision"] = revision.clone();
    }
    match publisher.get("knowledge_provider") {
        Some(peer) if peer["provider_id"] == reference["provider"] => {
            let profiles = json!([
                {"name": "core", "majors": [1], "required": true, "required_features": ["core.events", "core.grants"], "optional_features": []},
                {"name": "knowledge", "majors": [1], "required": true, "required_features": [], "optional_features": []},
            ]);
            let mut client =
                crate::peer::Peer::connect(peer, profiles).map_err(|_| "knowledge_unavailable")?;
            client
                .query("knowledge.claim.inspect", payload, peer["grant"].as_str())
                .map_err(|_| "knowledge_unavailable")
        }
        None if publisher["provider_id"] == reference["provider"] => {
            let reader = crate::knowledge::Reader {
                provider_id: publisher["provider_id"].as_str().unwrap_or_default(),
                config: &publisher["knowledge"],
                evidence_config: &publisher["evidence_store"],
                mutants: &Mutants::default(),
            };
            match crate::knowledge::inspect(tx, &payload, &reader) {
                Ok(Some(result)) => Ok(result),
                _ => Err("knowledge_unavailable"),
            }
        }
        _ => Err("knowledge_unavailable"),
    }
}

/// A context basis as a knowledge target (KNOWLEDGE section 8).
fn to_target(basis: &Value) -> Value {
    let repositories: Vec<Value> = basis["repositories"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| {
            let mut out = json!({"id": r["id"], "tree": r["tree"]});
            if r["dirty"].is_object() {
                out["dirty"] = r["dirty"].clone();
            }
            out
        })
        .collect();
    let mut target = json!({"repositories": repositories, "completeness": basis["completeness"]});
    for member in ["environment", "build"] {
        if let Some(value) = basis.get(member) {
            target[member] = value.clone();
        }
    }
    target
}

/// The section claim snapshot, after recomputing the record digest (CONTEXT section 14).
fn claim_snapshot(
    read: Result<Value, &'static str>,
    reference: &Value,
    basis: &Value,
    mutants: &Mutants,
) -> Result<Value, &'static str> {
    let found = read?;
    let computed = crate::json::sha256_digest(&crate::json::canonical(
        &found["record"],
        crate::json::CanonicalFlaws::default(),
    ));
    // Mutant `claim-latest-substituted` checks whatever revision it read against its own digest.
    let latest = mutants.on("claim-latest-substituted");
    let expected = if latest {
        &found["reference"]
    } else {
        reference
    };
    let exact = latest
        || (found["reference"]["digest"] == reference["digest"]
            && found["reference"]["revision"] == reference["revision"]);
    if !mutants.on("claim-digest-unchecked")
        && (computed != expected["digest"].as_str().unwrap_or_default() || !exact)
    {
        return Err("claim_digest_mismatch");
    }
    let target = to_target(basis);
    let canonical = |v: &Value| crate::json::canonical(v, crate::json::CanonicalFlaws::default());
    let applicability = found["applicability"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| canonical(&a["target"]) == canonical(&target))
        .map(|a| json!({"result": a["result"], "evaluation": a["evaluation"]}))
        .unwrap_or_else(|| json!({"result": "unknown"}));
    let conflicts: Vec<Value> = found["conflicts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["state"] == "open")
        .map(|c| json!({"conflict": c["conflict"], "kind": c["kind"], "status": c["status"]}))
        .collect();
    Ok(json!({
        "reference": found["reference"],
        "plane": found["record"]["plane"],
        "reliance": found["reliance"],
        "applicability": applicability,
        "conflicts": conflicts,
        "support": {"class": found["support"]["class"]},
        "current_revision": found["current_revision"],
    }))
}

/// Why a claim snapshot does not satisfy an item's reliance (CONTEXT section 14), if it does not.
fn claim_shortfall(item: &Value, snapshot: &Value) -> Option<&'static str> {
    let rank = |use_: &Value| match use_.as_str() {
        Some("binding") => 4,
        Some("evidence") => 3,
        Some("hypothesis") => 2,
        Some("reference") => 1,
        _ => 0,
    };
    let reliance = &snapshot["reliance"];
    match item["reliance"].as_str() {
        Some("binding" | "evidence") => {
            if reliance["state"] != "accepted_for_use"
                || rank(&reliance["permitted_use"]) < rank(&item["reliance"])
            {
                Some("not_accepted")
            } else if snapshot["applicability"]["result"] == "invalid_for_target" {
                Some("invalid_for_target")
            } else if snapshot["applicability"]["result"] != "applicable" {
                Some("applicability_not_established")
            } else {
                None
            }
        }
        _ if reliance["state"] == "rejected" => Some("not_accepted"),
        _ => None,
    }
}

/// The unmet reason a claim check gives an item, when it has a claim section.
fn claim_reason(item: &Value, job: &Value, mutants: &Mutants) -> Option<&'static str> {
    if item["check"]["kind"] != "claim_included" {
        return None;
    }
    let section = job["sections"].as_array().into_iter().flatten().find(|s| {
        s["item_id"] == item["item_id"] && s.get("claim").or_else(|| s.get("claim_error")).is_some()
    })?;
    if let Some(error) = section["claim_error"].as_str() {
        return Some(if error == "claim_digest_mismatch" {
            "claim_digest_mismatch"
        } else {
            "knowledge_unavailable"
        });
    }
    if section["claim"]["reference"] != item["check"]["claim"] {
        return Some("unavailable");
    }
    // Only a section that is not historical satisfies; a historical one is invalid for the target.
    claim_shortfall(item, &section["claim"]).or((section["historical"] == true
        && !mutants.on("historical-claim-reason-unavailable"))
    .then_some("invalid_for_target"))
}

/// Read-time claim facts: changes, claim invalidations and unverified items (CONTEXT section 14).
fn read_time_claims(
    tx: &Transaction,
    record: &Value,
    facts: &mut Value,
    reader: &PacketReader,
    mutants: &Mutants,
) {
    let basis = &record["basis"];
    let mut changes = Vec::new();
    let mut invalidated: Vec<Value> = facts["invalidated_items"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut unverified = Vec::new();
    let sections: Vec<Value> = facts["sections"].as_array().cloned().unwrap_or_default();
    for section in sections.iter().filter(|s| s["claim"].is_object()) {
        let snapshot = &section["claim"];
        let reference = &snapshot["reference"];
        let current = claim_snapshot(
            read_claim(tx, reader.publisher, reference),
            reference,
            basis,
            mutants,
        );
        let item = record["items"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|i| i["item_id"] == section["item_id"])
            .cloned()
            .unwrap_or(Value::Null);
        let satisfied_required = !item.is_null()
            && item["obligation"] != "advisory"
            && item["check"]["kind"] == "claim_included"
            && facts["items"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|i| i["item_id"] == item["item_id"] && i["result"] == "satisfied");
        let change = |kind: &str| json!({"section_id": section["section_id"], "claim": reference, "change": kind});
        match current {
            Err(reason) => {
                changes.push(change("unavailable"));
                if satisfied_required && !mutants.on("unavailable-knowledge-valid") {
                    unverified.push(
                        json!({"item_id": item["item_id"], "claim": reference, "reason": reason}),
                    );
                }
            }
            Ok(now) => {
                if now["reliance"]["state"] != snapshot["reliance"]["state"]
                    || now["reliance"]["permitted_use"] != snapshot["reliance"]["permitted_use"]
                {
                    changes.push(change("reliance_changed"));
                }
                if now["applicability"]["result"] != snapshot["applicability"]["result"] {
                    changes.push(change("applicability_changed"));
                }
                if now["conflicts"].as_array().into_iter().flatten().any(|c| {
                    !snapshot["conflicts"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|o| o["conflict"] == c["conflict"])
                }) {
                    changes.push(change("conflict_opened"));
                }
                if now["current_revision"].as_i64() > reference["revision"].as_i64() {
                    changes.push(change("lineage_revised"));
                }
                // For every reliance, a claim now invalid for the target would make its section
                // historical (CONTEXT section 14); a lost permitted use comes first.
                let shortfall = claim_shortfall(&item, &now)
                    .or((now["applicability"]["result"] == "invalid_for_target"
                        && !mutants.on("hypothesis-invalidation-unreported"))
                    .then_some("invalid_for_target"));
                if satisfied_required {
                    match shortfall {
                        Some("not_accepted") if !mutants.on("claim-invalidation-unreported") => {
                            invalidated.push(json!({"item_id": item["item_id"], "claim": reference, "reason": "permitted_use_lost"}));
                        }
                        Some("invalid_for_target")
                            if !mutants.on("claim-invalidation-unreported") =>
                        {
                            invalidated.push(json!({"item_id": item["item_id"], "claim": reference, "reason": "invalid_for_target"}));
                        }
                        Some("applicability_not_established") => {
                            unverified.push(json!({"item_id": item["item_id"], "claim": reference, "reason": "applicability_not_established"}));
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    facts["claim_changes"] = json!(changes);
    facts["invalidated_items"] = json!(invalidated);
    facts["unverified_items"] = json!(unverified);
}

fn excerpt(data: &[u8], payload: &Value, frame_budget: usize) -> Value {
    let offset = (payload["offset"].as_u64().unwrap_or(0) as usize).min(data.len());
    let mut take =
        (payload["max_bytes"].as_u64().unwrap_or(4096) as usize).min(data.len() - offset);
    while take > 1 && take.div_ceil(3) * 4 + 16_384 > frame_budget {
        take /= 2;
    }
    json!({"offset": offset, "length": take, "size": data.len(), "data_base64": crate::json::base64(&data[offset..offset + take])})
}

/// `context.packet.inspect`: result facts and an excerpt without a digest (CONTEXT section 6).
pub fn inspect_packet(
    store: &Store,
    payload: &Value,
    frame_budget: usize,
    reader: &PacketReader,
    mutants: &Mutants,
) -> rusqlite::Result<Option<Value>> {
    let tx = store.reader()?;
    let packet = payload["packet"].as_str().unwrap_or_default();
    let revision = payload["revision"].as_i64().unwrap_or(0);
    let Some((facts, current)) = facts(&tx, packet, revision, reader, mutants)? else {
        return Ok(None);
    };
    let data = if mutants.on("packet-bytes-regenerated") {
        serde_json::to_vec_pretty(&facts["body"]).unwrap_or_default()
    } else {
        evidence::decode_base64(facts["content_base64"].as_str().unwrap_or_default())
            .unwrap_or_default()
    };
    let mut excerpt = excerpt(&data, payload, frame_budget);
    if mutants.on("excerpt-carries-digest") {
        excerpt["digest"] = facts["reference"]["artifact"]["digest"].clone();
    }
    let mut result = facts.clone();
    if let Some(object) = result.as_object_mut() {
        object.remove("body");
        object.remove("content_base64");
        object.remove("supersedes");
    }
    if !facts["supersedes"].is_null() {
        result["supersedes"] = facts["supersedes"].clone();
    }
    result["current"] = json!(current);
    result["invalidated_items"] = facts["invalidated_items"].clone();
    for member in ["claim_changes", "unverified_items"] {
        if let Some(value) = facts.get(member) {
            result[member] = value.clone();
        }
    }
    if let Some(latest) = facts.get("superseded_by") {
        result["superseded_by"] = latest.clone();
    }
    result["excerpt"] = excerpt;
    Ok(Some(result))
}

/// The cited Evidence reference of a packet revision, if both exist.
pub fn citation(
    store: &Store,
    payload: &Value,
    mutants: &Mutants,
) -> rusqlite::Result<Option<Value>> {
    let tx = store.reader()?;
    let packet = payload["packet"].as_str().unwrap_or_default();
    let revision = payload["revision"].as_i64().unwrap_or(0);
    let reader = PacketReader {
        claims: false,
        publisher: &Value::Null,
    };
    let Some((facts, _)) = facts(&tx, packet, revision, &reader, mutants)? else {
        return Ok(None);
    };
    Ok(facts["citations"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| c["citation_id"] == payload["citation"])
        .map(|c| c["evidence"].clone()))
}

/// `context.expand` after authorization: a range of the cited artifact. `Err(code)` is
/// `permission_denied` when the citation cannot be read here, or `artifact_digest_mismatch` when the
/// cited digest is not the artifact's sealed digest.
pub fn expand(
    store: &Store,
    payload: &Value,
    reference: &Value,
    frame_budget: usize,
) -> rusqlite::Result<Result<Value, &'static str>> {
    let tx = store.reader()?;
    let artifact = reference["artifact"]["id"].as_str().unwrap_or_default();
    let Some((_, record)) = evidence::load(&tx, evidence::ARTIFACT, artifact)? else {
        return Ok(Err("permission_denied"));
    };
    if record["state"] != "sealed" {
        return Ok(Err("permission_denied"));
    }
    if record["descriptor"]["digest"] != reference["digest"] {
        return Ok(Err("artifact_digest_mismatch"));
    }
    let data = evidence::sealed_bytes(&tx, artifact)?;
    Ok(Ok(
        json!({"citation": payload["citation"], "evidence": reference, "excerpt": excerpt(&data, payload, frame_budget)}),
    ))
}

/// Requests of a job, for event visibility.
pub fn job_requests(store: &Store, job_id: &str) -> rusqlite::Result<Vec<String>> {
    let Some((_, value, _)) = store.subject(JOB, job_id)? else {
        return Ok(Vec::new());
    };
    let job: Value = serde_json::from_str(&value).unwrap_or(Value::Null);
    let job_id = job_id.to_string();
    let mut requests: Vec<String> = job["requests"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(String::from)
        .collect();
    if !requests.contains(&job_id) {
        requests.push(job_id);
    }
    Ok(requests)
}

/// Investigations in progress, by job and execution. The executor is called from a worker
/// thread, never while this provider holds its processing lock: the executor may be checking a
/// packet here at the same moment (the preparation/resource cycle, CTX-18).
fn investigations() -> &'static std::sync::Mutex<std::collections::HashMap<String, Value>> {
    static REGISTRY: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Value>>,
    > = std::sync::OnceLock::new();
    REGISTRY.get_or_init(Default::default)
}

/// Start (once) and follow a job's investigation execution. Returns whether it has finished.
fn investigate(
    publisher: &Value,
    mutants: &Mutants,
    job_id: &str,
    job: &mut Value,
    step: &Value,
) -> bool {
    let execution = step["execution"].as_str().unwrap_or_default().to_string();
    let key = format!("{job_id}/{execution}");
    if job["investigations"][&execution]["state"] == "finished" {
        return true;
    }
    let mut registry = investigations()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match registry.get(&key).map(|v| v["state"].clone()) {
        Some(state) if state == "finished" => {
            job["investigations"][&execution] = registry[&key].clone();
            return true;
        }
        Some(state) if state == "running" => return false,
        _ => {}
    }
    registry.insert(key.clone(), json!({"state": "running"}));
    drop(registry);
    let peer = publisher["executor"].clone();
    let job_id = job_id.to_string();
    let step = step.clone();
    let re_enrich = mutants.on("job-execution-re-enriched");
    std::thread::spawn(move || {
        let outcome = run_investigation(&peer, &job_id, &execution, &step, re_enrich);
        let mut registry = investigations()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match outcome {
            Some(result) => registry.insert(key, json!({"state": "finished", "result": result})),
            None => registry.remove(&key),
        };
    });
    false
}

fn run_investigation(
    peer: &Value,
    job_id: &str,
    execution: &str,
    step: &Value,
    re_enrich: bool,
) -> Option<Value> {
    let grant = peer["grant"].as_str();
    let mut core = vec!["core.events", "core.capabilities", "core.effects"];
    if grant.is_some() {
        core.push("core.grants");
    }
    let profiles = json!([
        {"name": "core", "majors": [1], "required": true, "required_features": core, "optional_features": []},
        {"name": "execution", "majors": [1], "required": true, "required_features": ["execution.context"], "optional_features": []},
    ]);
    let mut client = crate::peer::Peer::connect(peer, profiles).ok()?;
    let subject = json!({"kind": "execution.execution", "id": execution});
    let mut payload = json!({
        "brief": {"digest": crate::json::sha256_digest(step["brief"].as_str().unwrap_or("investigate").as_bytes()), "media_type": "text/plain"},
        "origin": {"initiator": {"kind": JOB, "id": job_id}, "depth": step["depth"].as_u64().unwrap_or(1), "call_budget": step["call_budget"].as_u64().unwrap_or(1)},
    });
    if re_enrich {
        payload["context_bindings"] = json!([{"binding_id": "job-context", "packet": {"ref": format!("job.{job_id}"), "digest": crate::json::sha256_digest(job_id.as_bytes())},
            "obligation": "advisory", "selected_by": "context-provider"}]);
    }
    client
        .command(
            "execution.submit",
            &format!("job.{job_id}.execute.{execution}"),
            subject.clone(),
            json!([{"subject": subject, "revision": 0}]),
            payload,
            grant,
        )
        .ok()?;
    for _ in 0..600 {
        let result = client
            .query("execution.inspect", json!({"execution": execution}), grant)
            .ok()?;
        if result["result"] == "returned" || result["runtime"] == "exited" {
            return Some(result["result"].clone());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    None
}

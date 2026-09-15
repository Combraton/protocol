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
const COMPILER: &str = "combraton-reference-context";

/// Features of the submitting session that change how a request is prepared.
pub struct Session<'a> {
    pub principal: &'a str,
    pub shared_jobs: bool,
    pub updates: bool,
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

/// Obligation features a submit needs (CTX-9).
pub fn obligation_features(payload: &Value) -> Vec<String> {
    let mut features: Vec<String> = Vec::new();
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
            });
            let mut result = json!({"item_id": item_id, "obligation": obligation});
            if let Some(unmet) = job["unmet"].get(item_id) {
                result["result"] = json!("unmet");
                result["reason"] = unmet.clone();
            } else if satisfied {
                result["result"] = json!("satisfied");
            } else if !finishing {
                result["result"] = json!("pending");
            } else {
                let cause = if omitted {
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
    let mut body = json!({
        "format": PACKET_FORMAT,
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
    Ok(Some((facts, current)))
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
    mutants: &Mutants,
) -> rusqlite::Result<Option<Value>> {
    let tx = store.reader()?;
    let packet = payload["packet"].as_str().unwrap_or_default();
    let revision = payload["revision"].as_i64().unwrap_or(0);
    let Some((facts, current)) = facts(&tx, packet, revision, mutants)? else {
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
    let Some((facts, _)) = facts(&tx, packet, revision, mutants)? else {
        return Ok(None);
    };
    Ok(facts["citations"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| c["citation_id"] == payload["citation"])
        .map(|c| c["evidence"].clone()))
}

/// `context.expand` after authorization: a range of the cited artifact.
pub fn expand(
    store: &Store,
    payload: &Value,
    reference: &Value,
    frame_budget: usize,
) -> rusqlite::Result<Option<Value>> {
    let tx = store.reader()?;
    let artifact = reference["artifact"]["id"].as_str().unwrap_or_default();
    let Some((_, record)) = evidence::load(&tx, evidence::ARTIFACT, artifact)? else {
        return Ok(None);
    };
    if record["state"] != "sealed" || record["descriptor"]["digest"] != reference["digest"] {
        return Ok(None);
    }
    let data = evidence::sealed_bytes(&tx, artifact)?;
    Ok(Some(
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

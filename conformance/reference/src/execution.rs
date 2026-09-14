//! Reference executor for `execution/1` over a scripted fake harness (EXECUTION draft, decision 007).
//!
//! The script is test environment selected by launch configuration (`executor.scripts`). It is
//! never a protocol message. Execution records live in the subjects table (kind
//! `execution.execution`) so Core preconditions and revisions apply unchanged; effects live in their
//! own table.
//!
//! Delivery model (owner decisions, 2026-09-14): the delivery axis is the current determination
//! with its evidence; earlier determinations are kept in `history`. A write-ahead dispatch marker
//! is recorded before any harness write. Recovery after a restart may resume an undispatched
//! delivery under the same identity only with an intact journal and after revalidation, and it
//! fences older dispatchers by advancing the host generation.

use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use crate::mutants::Mutants;
use crate::store::{Store, append_event};

pub const KIND: &str = "execution.execution";
const CRASH_STATUS: i32 = 86;

/// Everything a command needs besides the store.
pub struct Context<'a> {
    pub now: String,
    pub executor: &'a Value,
    pub mutants: &'a Mutants,
    pub principal: &'a str,
    pub grant: Option<&'a str>,
}

/// What recovery needs to revalidate a resumable delivery.
pub struct Recovery<'a> {
    pub now: String,
    pub executor: &'a Value,
    pub mutants: &'a Mutants,
    pub authorities: &'a [String],
    /// False when the provider cannot vouch for journal continuity (for example, a new stream
    /// epoch at this start); a missing dispatch marker is then not proof that dispatch never began.
    pub journal_intact: bool,
}

type Draft = (&'static str, Value, i64, Value);

fn subject(id: &str) -> Value {
    json!({"kind": KIND, "id": id})
}

pub fn load(tx: &Transaction, id: &str) -> rusqlite::Result<Option<(i64, Value)>> {
    let row: Option<(i64, String)> = tx
        .query_row(
            "SELECT revision, value FROM subjects WHERE kind=?1 AND id=?2",
            params![KIND, id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(row.map(|(revision, value)| {
        (
            revision,
            serde_json::from_str(&value).unwrap_or(Value::Null),
        )
    }))
}

fn save(tx: &Transaction, id: &str, revision: i64, record: &Value) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO subjects VALUES (?1, ?2, ?3, ?4, 1)
         ON CONFLICT (kind, id) DO UPDATE SET revision=excluded.revision, value=excluded.value,
         applied_count=applied_count+1",
        params![KIND, id, revision, record.to_string()],
    )?;
    Ok(())
}

fn load_effect(tx: &Transaction, id: &str) -> rusqlite::Result<Option<Value>> {
    let row: Option<String> = tx
        .query_row("SELECT record FROM effects WHERE id=?1", [id], |r| r.get(0))
        .optional()?;
    Ok(row.map(|record| serde_json::from_str(&record).unwrap_or(Value::Null)))
}

fn save_effect(
    tx: &Transaction,
    id: &str,
    execution: &str,
    record: &Value,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO effects VALUES (?1, ?2, ?3) ON CONFLICT (id) DO UPDATE SET record=excluded.record",
        params![id, execution, record.to_string()],
    )?;
    Ok(())
}

fn observe_effect(effect: &mut Value, status: &str, class: &str, source: &str, now: &str) {
    if let Some(list) = effect["observations"].as_array_mut() {
        list.push(json!({"status": status, "evidence": {"class": class, "source": source}, "recorded_at": now}));
    }
}

fn set_obligation(effect: &mut Value, state: &str) {
    for obligation in effect["obligations"].as_array_mut().into_iter().flatten() {
        if obligation["state"] == "open" {
            obligation["state"] = json!(state);
        }
    }
}

fn enforcement_rank(level: &str) -> i32 {
    match level {
        "enforced" => 3,
        "mediated" => 2,
        "cooperative" => 1,
        _ => 0,
    }
}

fn add_seconds(instant: &str, seconds: i64) -> String {
    crate::clock::from_seconds(crate::clock::to_seconds(instant) + seconds)
}

/// Effect status implied by a delivery determination (CORE section 19).
fn status_for(delivery: &str) -> &'static str {
    match delivery {
        "acknowledged" | "delivered" => "succeeded",
        "not_delivered" | "failed_before_delivery" => "failed",
        "ambiguous" => "unknown",
        _ => "pending",
    }
}

fn is_terminal(delivery: &str) -> bool {
    matches!(
        delivery,
        "acknowledged" | "delivered" | "not_delivered" | "failed_before_delivery"
    )
}

/// Replace the current delivery determination, keeping the previous one in history.
fn determine(
    record: &mut Value,
    delivery: &str,
    evidence: Value,
    proof_class: Option<&str>,
    now: &str,
    keep_history: bool,
) {
    let current = record["deliveries"][0].clone();
    let mut history = current["history"].as_array().cloned().unwrap_or_default();
    if keep_history {
        let mut previous =
            json!({"delivery": current["delivery"], "recorded_at": current["determined_at"]});
        if let Some(evidence) = current.get("evidence") {
            previous["evidence"] = evidence.clone();
        }
        history.push(previous);
    } else {
        history.clear();
    }
    let slot = &mut record["deliveries"][0];
    slot["delivery"] = json!(delivery);
    slot["evidence"] = evidence;
    slot["determined_at"] = json!(now);
    slot["history"] = json!(history);
    if let Some(proof) = proof_class {
        slot["proof_class"] = json!(proof);
    }
    record["delivery"] = json!(delivery);
}

/// `execution.submit` inside the Core owner transaction (CORE section 10 step 8).
pub fn submit(
    tx: &Transaction,
    execution_id: &str,
    payload: &Value,
    sequence: i64,
    ctx: &Context,
) -> rusqlite::Result<(i64, Value, Vec<Draft>, Vec<String>)> {
    let adapter = &ctx.executor["adapter"];
    let advertised = adapter["enforcement"].as_str().unwrap_or("cooperative");
    let mut refusal: Option<(&str, String)> = None;
    for required in payload["restrictions"].as_array().into_iter().flatten() {
        let wanted = required["enforcement"].as_str().unwrap_or_default();
        if enforcement_rank(wanted) > enforcement_rank(advertised)
            && !ctx.mutants.on("admits-weaker-enforcement")
        {
            refusal = Some((
                "enforcement_unavailable",
                format!(
                    "run with {advertised} enforcement, or use an executor that enforces {wanted}"
                ),
            ));
        }
    }
    for name in payload["adapter"]["requires"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let status = adapter["predicates"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|p| p["name"] == *name)
            .and_then(|p| p["status"].as_str())
            .unwrap_or("unknown");
        if status != "supported" && !ctx.mutants.on("unknown-predicate-supported") {
            refusal = Some((
                "capability_unavailable",
                format!("capability {name} is {status}"),
            ));
        }
    }
    let script = ctx.executor["scripts"][execution_id]
        .as_array()
        .cloned()
        .or_else(|| ctx.executor["default_script"].as_array().cloned())
        .unwrap_or_default();
    let mut record = json!({
        "admission": if refusal.is_some() { "refused" } else { "admitted" },
        "delivery": "pending",
        "runtime": "preparing",
        "result": "absent",
        "exit": "unavailable",
        "evaluation": "not_requested",
        "deliveries": [],
        "completions": [],
        "effects": [],
        "recovery": [],
        "host": {"id": "scripted-host", "generation": 1},
        "submitted_at": ctx.now,
        "timeouts": payload["timeouts"].clone(),
        "timeouts_passed": [],
        "script": script,
        "script_position": 0,
        "authorization": {"principal": ctx.principal, "grant": ctx.grant},
    });
    if let Some(predecessor) = payload.get("predecessor")
        && !ctx.mutants.on("predecessor-dropped")
    {
        record["predecessor"] = predecessor.clone();
    }
    if let Some(correlation) = payload.get("correlation") {
        record["correlation"] = correlation.clone();
    }
    let mut outcome = json!({"execution": subject(execution_id), "admission": record["admission"]});
    let mut events = vec![(
        "execution.admission.changed",
        subject(execution_id),
        1,
        json!({"admission": record["admission"]}),
    )];
    let mut effect_refs = Vec::new();
    if let Some((reason, alternative)) = refusal {
        outcome["reason"] = json!(reason);
        outcome["alternative"] = json!(alternative);
        events[0].3["reason"] = json!(reason);
    } else {
        let delivery_id = format!("{execution_id}.delivery-1");
        let deadline = payload["timeouts"]["delivery"]
            .as_i64()
            .map_or(Value::Null, |seconds| json!(add_seconds(&ctx.now, seconds)));
        let mut effect = json!({
            "descriptor": {
                "id": delivery_id,
                "kind": "execution.prompt_submission",
                "target": subject(execution_id),
                "payload_digest": payload["brief"]["digest"],
                "authorization": {"principal": ctx.principal},
                "retry_class": "non_repeatable",
                "operation_ref": format!("op-{sequence}"),
            },
            "observations": [],
            "obligations": [{"id": format!("{delivery_id}.evidence"), "expects": "delivery evidence", "deadline": deadline, "state": "open"}],
            "dispatch": Value::Null,
        });
        if let Some(grant) = ctx.grant {
            effect["descriptor"]["authorization"]["grant"] = json!(grant);
        }
        observe_effect(
            &mut effect,
            "pending",
            "recorded_before_dispatch",
            "execution.submit owner transaction",
            &ctx.now,
        );
        if ctx.mutants.on("effect-recorded-after-dispatch") {
            record["deferred_effect"] = effect;
        } else {
            save_effect(tx, &delivery_id, execution_id, &effect)?;
        }
        record["deliveries"] = json!([{"delivery_id": delivery_id, "delivery": "pending", "determined_at": ctx.now, "history": []}]);
        record["effects"] = json!([delivery_id]);
        outcome["delivery_id"] = json!(delivery_id);
        effect_refs.push(delivery_id);
    }
    save(tx, execution_id, 1, &record)?;
    Ok((1, outcome, events, effect_refs))
}

/// `execution.cancel`: records the request and returns a request receipt (EXECUTION section 7).
pub fn cancel(
    tx: &Transaction,
    execution_id: &str,
    sequence: i64,
    ctx: &Context,
) -> rusqlite::Result<(i64, Value, Vec<Draft>, Vec<String>)> {
    let (revision, mut record) = load(tx, execution_id)?.unwrap_or((0, json!({})));
    let receipt = json!({"state": "cancel_requested", "operation_ref": format!("op-{sequence}")});
    record["cancellation"] = json!({"receipt": receipt});
    if ctx.mutants.on("cancel-reports-cancelled") {
        record["cancellation"]["outcome"] = json!("cancelled");
    }
    let revision = revision + 1;
    save(tx, execution_id, revision, &record)?;
    let events = vec![(
        "execution.cancel.requested",
        subject(execution_id),
        revision,
        json!({"receipt": receipt}),
    )];
    Ok((revision, json!({"receipt": receipt}), events, Vec::new()))
}

fn provider_event(tx: &Transaction, stream: &str, now: &str, draft: Draft) -> rusqlite::Result<()> {
    let (event_type, event_subject, revision, payload) = draft;
    append_event(
        tx,
        json!({
            "stream": stream,
            "origin": "provider",
            "type": event_type,
            "subject": event_subject,
            "revision": revision,
            "caused_by": [],
            "recorded_at": now,
            "payload": payload,
        }),
        false,
    )?;
    Ok(())
}

fn execution_ids(tx: &Transaction) -> rusqlite::Result<Vec<String>> {
    let mut statement = tx.prepare("SELECT id FROM subjects WHERE kind=?1 ORDER BY id")?;
    let rows = statement.query_map([KIND], |r| r.get(0))?;
    rows.collect()
}

/// Whether the submitting principal may still act: an authority, or an active, unexpired grant
/// held by the submitter and bound to a current epoch.
fn still_authorized(
    tx: &Transaction,
    record: &Value,
    recovery: &Recovery,
) -> rusqlite::Result<bool> {
    if recovery.mutants.on("recovery-ignores-revocation") {
        return Ok(true);
    }
    let principal = record["authorization"]["principal"]
        .as_str()
        .unwrap_or_default();
    let Some(grant_id) = record["authorization"]["grant"].as_str() else {
        return Ok(recovery.authorities.iter().any(|a| a == principal));
    };
    let grant: Option<String> = tx
        .query_row("SELECT record FROM grants WHERE id=?1", [grant_id], |r| {
            r.get(0)
        })
        .optional()?;
    let Some(grant) = grant.and_then(|g| serde_json::from_str::<Value>(&g).ok()) else {
        return Ok(false);
    };
    if grant["state"] != "active" || grant["holder"] != principal {
        return Ok(false);
    }
    if let Some(expiry) = grant["expires_at"].as_str()
        && recovery.now.as_str() >= expiry
    {
        return Ok(false);
    }
    if let Some(binding) = grant.get("authority_binding") {
        let epoch: i64 = tx
            .query_row(
                "SELECT revision FROM subjects WHERE kind='core-test.authority' AND id=?1",
                [binding["scope"].as_str().unwrap_or_default()],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0);
        if binding["epoch"].as_i64() != Some(epoch) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn deadline_passed(record: &Value, now: &str) -> bool {
    let submitted = record["submitted_at"].as_str().unwrap_or_default();
    ["delivery", "execution_deadline"].iter().any(|name| {
        record["timeouts"][*name]
            .as_i64()
            .is_some_and(|seconds| now >= add_seconds(submitted, seconds).as_str())
    })
}

fn bump_generation(record: &mut Value) {
    let generation = record["host"]["generation"].as_i64().unwrap_or(1) + 1;
    record["host"]["generation"] = json!(generation);
}

/// Crash recovery at process start (EXECUTION section 7.1). Every still-pending delivery gets a
/// recorded decision:
/// - a dispatch marker exists, or the journal is not intact: `ambiguous`, never re-sent;
/// - otherwise revalidate cancellation, authorization and deadlines, then apply the executor's
///   recovery policy: resume under the same identity, or terminate as `failed_before_delivery`.
///
/// A resumed or ambiguous delivery advances the host generation so older dispatchers are fenced.
/// A delivery already declared `failed_before_delivery` is never reopened.
pub fn recover(store: &mut Store, recovery: &Recovery) -> rusqlite::Result<()> {
    let stream = store.stream_id()?;
    let tx = store.transaction()?;
    let now = recovery.now.as_str();
    let mutants = recovery.mutants;
    for id in execution_ids(&tx)? {
        let Some((mut revision, mut record)) = load(&tx, &id)? else {
            continue;
        };
        let reopen = record["delivery"] == "failed_before_delivery"
            && mutants.on("failed-before-delivery-reopened");
        if record["admission"] != "admitted" || (record["delivery"] != "pending" && !reopen) {
            continue;
        }
        let Some(delivery_id) = record["deliveries"][0]["delivery_id"]
            .as_str()
            .map(String::from)
        else {
            continue;
        };
        let Some(mut effect) = load_effect(&tx, &delivery_id)? else {
            continue;
        };
        let marker = !effect["dispatch"].is_null();
        let journal_intact =
            recovery.journal_intact || mutants.on("recovery-trusts-damaged-journal");
        let (decision, reason) = if marker && !mutants.on("ambiguous-dispatch-resent") {
            if mutants.on("ambiguity-overwritten") {
                continue;
            }
            ("ambiguous", "dispatch_may_have_begun")
        } else if reopen {
            ("dispatch_resumed", "provably_not_dispatched")
        } else if !journal_intact {
            ("ambiguous", "journal_not_intact")
        } else if record.get("cancellation").is_some()
            && !mutants.on("recovery-ignores-cancellation")
        {
            ("failed_before_delivery", "cancelled")
        } else if !still_authorized(&tx, &record, recovery)? {
            ("failed_before_delivery", "authorization_lost")
        } else if deadline_passed(&record, now) && !mutants.on("recovery-ignores-deadline") {
            ("failed_before_delivery", "deadline_passed")
        } else if recovery.executor["recovery_policy"] == "terminate" {
            ("failed_before_delivery", "recovery_policy")
        } else {
            ("dispatch_resumed", "provably_not_dispatched")
        };
        let evidence =
            json!({"class": "recovery", "source": format!("restart recovery: {reason}")});
        revision += 1;
        let mut drafts: Vec<Draft> = vec![(
            "execution.recovery.decided",
            subject(&id),
            revision,
            json!({"delivery_id": delivery_id, "decision": decision, "reason": reason}),
        )];
        match decision {
            "ambiguous" => {
                determine(&mut record, "ambiguous", evidence.clone(), None, now, true);
                observe_effect(&mut effect, "unknown", "dispatch_uncertain", reason, now);
                bump_generation(&mut record);
                drafts.push((
                    "execution.delivery.observed",
                    subject(&id),
                    revision,
                    json!({"delivery_id": delivery_id, "delivery": "ambiguous", "evidence": evidence}),
                ));
            }
            "failed_before_delivery" => {
                determine(
                    &mut record,
                    "failed_before_delivery",
                    evidence.clone(),
                    None,
                    now,
                    true,
                );
                observe_effect(&mut effect, "failed", "never_dispatched", reason, now);
                set_obligation(&mut effect, "satisfied");
                drafts.push((
                    "execution.delivery.observed",
                    subject(&id),
                    revision,
                    json!({"delivery_id": delivery_id, "delivery": "failed_before_delivery", "evidence": evidence}),
                ));
            }
            _ => {
                if reopen {
                    determine(&mut record, "pending", evidence.clone(), None, now, true);
                }
                bump_generation(&mut record);
            }
        }
        drafts[0].3["host"] = record["host"].clone();
        if let Some(list) = record["recovery"].as_array_mut() {
            list.push(json!({"delivery_id": delivery_id, "decision": decision, "reason": reason, "recorded_at": now}));
        }
        save_effect(&tx, &delivery_id, &id, &effect)?;
        save(&tx, &id, revision, &record)?;
        for draft in drafts {
            provider_event(&tx, &stream, now, draft)?;
        }
    }
    tx.commit()
}

/// Advance every execution's script and timeouts as far as the provider clock allows.
pub fn tick(
    store: &mut Store,
    now: &str,
    executor: &Value,
    mutants: &Mutants,
) -> rusqlite::Result<()> {
    let stream = store.stream_id()?;
    let ids = {
        let tx = store.transaction()?;
        let ids = execution_ids(&tx)?;
        tx.commit()?;
        ids
    };
    for id in ids {
        loop {
            let tx = store.transaction()?;
            let Some((revision, record)) = load(&tx, &id)? else {
                break;
            };
            let progressed = step(&tx, &stream, now, executor, mutants, &id, revision, record)?;
            tx.commit()?;
            if !progressed {
                break;
            }
        }
        let tx = store.transaction()?;
        if let Some((revision, record)) = load(&tx, &id)? {
            timeouts(&tx, &stream, now, mutants, &id, revision, record)?;
        }
        tx.commit()?;
    }
    Ok(())
}

/// Record the write-ahead dispatch marker before any harness write.
fn mark_dispatch(effect: &mut Value, generation: &Value, now: &str) {
    if effect["dispatch"].is_null() {
        effect["dispatch"] = json!({"generation": generation});
        observe_effect(
            effect,
            "pending",
            "dispatch_intent",
            "write-ahead dispatch marker",
            now,
        );
    }
}

/// Apply one script step. Returns whether the script advanced.
#[allow(clippy::too_many_arguments)]
fn step(
    tx: &Transaction,
    stream: &str,
    now: &str,
    executor: &Value,
    mutants: &Mutants,
    id: &str,
    revision: i64,
    mut record: Value,
) -> rusqlite::Result<bool> {
    if record["admission"] != "admitted" {
        return Ok(false);
    }
    let position = record["script_position"].as_u64().unwrap_or(0) as usize;
    let Some(step) = record["script"].get(position).cloned() else {
        return Ok(false);
    };
    let delivery_id = record["deliveries"][0]["delivery_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let mut effect = load_effect(tx, &delivery_id)?.unwrap_or(Value::Null);
    let revision = revision + 1;
    let mut drafts: Vec<Draft> = Vec::new();
    record["script_position"] = json!(position + 1);
    let generation = record["host"]["generation"].clone();
    let may_dispatch = record["delivery"] == "pending";

    if (step.get("deliver").is_some() || step.get("crash").is_some()) && !may_dispatch {
        // A terminal or ambiguous delivery is never dispatched again under this identity.
    } else if let Some(instant) = step["wait_until"].as_str() {
        if now < instant {
            return Ok(false);
        }
    } else if step["wait_for"] == "cancel" {
        if record.get("cancellation").is_none() {
            return Ok(false);
        }
    } else if let Some(outcome) = step["on_cancel"].as_str() {
        if record.get("cancellation").is_none() {
            return Ok(false);
        }
        if record["cancellation"].get("outcome").is_none() {
            record["cancellation"]["outcome"] = json!(outcome);
            drafts.push((
                "execution.cancel.observed",
                subject(id),
                revision,
                json!({"outcome": outcome}),
            ));
        }
    } else if let Some(when) = step["crash"].as_str() {
        if record.get("deferred_effect").is_some() {
            if when == "after_write" {
                let mut deferred = record["deferred_effect"].take();
                mark_dispatch(&mut deferred, &generation, now);
                save_effect(tx, &delivery_id, id, &deferred)?;
            }
            if let Some(map) = record.as_object_mut() {
                map.remove("deferred_effect");
            }
        } else if when == "after_write" && effect.is_object() {
            mark_dispatch(&mut effect, &generation, now);
            save_effect(tx, &delivery_id, id, &effect)?;
        }
        save(tx, id, revision - 1, &record)?;
        tx.execute_batch("COMMIT; BEGIN;")?;
        std::process::exit(CRASH_STATUS);
    } else if let Some(stale) = step["stale_dispatch"]["generation"].as_i64() {
        let current = generation.as_i64().unwrap_or(1);
        if stale < current && !mutants.on("stale-dispatcher-sends") {
            drafts.push((
                "execution.dispatch.fenced",
                subject(id),
                revision,
                json!({"delivery_id": delivery_id, "generation": stale, "current_generation": current}),
            ));
        } else if effect.is_object() && may_dispatch {
            mark_dispatch(&mut effect, &json!(stale), now);
            observe_effect(
                &mut effect,
                "succeeded",
                "provider_ack_id",
                "stale dispatcher",
                now,
            );
            set_obligation(&mut effect, "satisfied");
            save_effect(tx, &delivery_id, id, &effect)?;
            let evidence = json!({"class": "provider_ack_id", "source": "stale dispatcher"});
            determine(
                &mut record,
                "acknowledged",
                evidence.clone(),
                Some("provider_ack_id"),
                now,
                true,
            );
            drafts.push((
                "execution.delivery.observed",
                subject(id),
                revision,
                json!({"delivery_id": delivery_id, "delivery": "acknowledged", "proof_class": "provider_ack_id", "evidence": evidence}),
            ));
        }
    } else if let Some(proof) = step["deliver"].as_str() {
        if record.get("deferred_effect").is_some() {
            effect = record["deferred_effect"].take();
            if let Some(map) = record.as_object_mut() {
                map.remove("deferred_effect");
            }
        }
        if effect.is_object() {
            mark_dispatch(&mut effect, &generation, now);
        }
        let proves = match proof {
            "provider_ack_id" => true,
            "echo" => {
                executor["adapter"]["echo_proves_delivery"]
                    .as_bool()
                    .unwrap_or(false)
                    || mutants.on("echo-always-acknowledged")
            }
            "bytes_written" => mutants.on("bytes-written-acknowledged"),
            _ => false,
        };
        let evidence = json!({"class": proof, "source": "scripted harness"});
        if proves {
            observe_effect(&mut effect, "succeeded", proof, "scripted harness", now);
            set_obligation(&mut effect, "satisfied");
            determine(
                &mut record,
                "acknowledged",
                evidence.clone(),
                Some(proof),
                now,
                true,
            );
        } else {
            // Evidence that does not establish delivery: still awaiting evidence.
            observe_effect(&mut effect, "pending", proof, "scripted harness", now);
            record["deliveries"][0]["proof_class"] = json!(proof);
            record["deliveries"][0]["evidence"] = evidence.clone();
        }
        if effect.is_object() {
            save_effect(tx, &delivery_id, id, &effect)?;
        }
        drafts.push((
            "execution.delivery.observed",
            subject(id),
            revision,
            json!({"delivery_id": delivery_id, "delivery": record["delivery"], "proof_class": proof, "evidence": evidence}),
        ));
    } else if let Some(found) = step["reconcile_finds"].as_str() {
        let resolved = match found {
            "delivered" => "delivered",
            "not_delivered" => "not_delivered",
            _ => "ambiguous",
        };
        let evidence =
            json!({"class": "reconciliation", "source": "scripted harness reconciliation"});
        observe_effect(
            &mut effect,
            status_for(resolved),
            "reconciliation",
            "scripted harness reconciliation",
            now,
        );
        if resolved != "ambiguous" {
            set_obligation(&mut effect, "satisfied");
        }
        save_effect(tx, &delivery_id, id, &effect)?;
        if resolved != "ambiguous" && !mutants.on("reconciliation-keeps-ambiguous") {
            let keep = !mutants.on("reconciliation-erases-ambiguity");
            determine(&mut record, resolved, evidence.clone(), None, now, keep);
        }
        drafts.push((
            "execution.delivery.reconciled",
            subject(id),
            revision,
            json!({"delivery_id": delivery_id, "outcome": found, "delivery": record["delivery"], "evidence": evidence}),
        ));
    } else if let Some(runtime) = step["runtime"].as_str() {
        record["runtime"] = json!(runtime);
        let mut payload = json!({"runtime": runtime});
        if let Some(action) = step.get("action_id") {
            payload["action_id"] = action.clone();
            payload["owner"] = step["owner"].clone();
        }
        drafts.push(("execution.runtime.changed", subject(id), revision, payload));
    } else if step.get("host_restart").is_some() {
        bump_generation(&mut record);
        record["runtime"] = json!("unknown");
        drafts.push((
            "execution.host.changed",
            subject(id),
            revision,
            json!({"host": record["host"]}),
        ));
    } else if let Some(completion) = step.get("complete") {
        complete(&mut record, completion, mutants, id, revision, &mut drafts);
    } else if let Some(exit) = step.get("exit") {
        record["exit"] = exit.clone();
        record["runtime"] = json!("exited");
        if mutants.on("evaluation-from-exit") && exit["code"] == 0 {
            record["evaluation"] = json!({"reference": "exit-status-zero"});
        }
        drafts.push((
            "execution.exit.observed",
            subject(id),
            revision,
            json!({"exit": exit}),
        ));
    } else if step.get("stall").is_some() {
        return Ok(false);
    }
    save(tx, id, revision, &record)?;
    for draft in drafts {
        provider_event(tx, stream, now, draft)?;
    }
    Ok(true)
}

fn complete(
    record: &mut Value,
    completion: &Value,
    mutants: &Mutants,
    id: &str,
    revision: i64,
    drafts: &mut Vec<Draft>,
) {
    let completion_id = completion["completion_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let digest = crate::json::sha256_digest(
        completion["content"]
            .as_str()
            .unwrap_or_default()
            .as_bytes(),
    );
    let current = record["host"]["generation"].as_i64().unwrap_or(1);
    let generation = completion["generation"].as_i64().unwrap_or(current);
    let prior = record["completions"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| c["completion_id"] == completion_id.as_str() && c["status"] == "recorded")
        .cloned();
    let status = if generation < current && !mutants.on("old-attempt-finalizes") {
        "superseded_attempt"
    } else {
        match prior {
            Some(prior) if prior["digest"] == digest.as_str() => "duplicate",
            Some(_) if mutants.on("last-completion-wins") => {
                record["finalized_digest"] = json!(digest);
                "recorded"
            }
            Some(_) => "conflict",
            None => "recorded",
        }
    };
    if status == "recorded" && record.get("finalized_by").is_none() {
        record["finalized_by"] = json!(completion_id);
        record["finalized_digest"] = json!(digest);
        record["result"] = json!("returned");
    }
    if let Some(list) = record["completions"].as_array_mut() {
        list.push(json!({"completion_id": completion_id, "digest": digest, "generation": generation, "status": status}));
    }
    drafts.push((
        "execution.completion.recorded",
        subject(id),
        revision,
        json!({"completion_id": completion_id, "digest": digest, "host": {"id": record["host"]["id"], "generation": generation}, "status": status}),
    ));
}

fn timeouts(
    tx: &Transaction,
    stream: &str,
    now: &str,
    mutants: &Mutants,
    id: &str,
    revision: i64,
    mut record: Value,
) -> rusqlite::Result<()> {
    if record["admission"] != "admitted" {
        return Ok(());
    }
    let submitted = record["submitted_at"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let passed: Vec<String> = record["timeouts_passed"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(String::from)
        .collect();
    let mut due = Vec::new();
    for (name, applies) in [
        (
            "delivery",
            !is_terminal(record["delivery"].as_str().unwrap_or_default()),
        ),
        ("execution_deadline", record["runtime"] != "exited"),
    ] {
        let Some(seconds) = record["timeouts"][name].as_i64() else {
            continue;
        };
        if applies
            && !passed.contains(&name.to_string())
            && now >= add_seconds(&submitted, seconds).as_str()
        {
            due.push(name);
        }
    }
    if due.is_empty() {
        return Ok(());
    }
    if mutants.on("timeouts-collapsed") {
        due = vec!["delivery", "execution_deadline"];
    }
    let delivery_id = record["deliveries"][0]["delivery_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let mut effect = load_effect(tx, &delivery_id)?.unwrap_or(Value::Null);
    let mut revision = revision;
    let mut drafts: Vec<Draft> = Vec::new();
    for name in due {
        if passed.contains(&name.to_string()) {
            continue;
        }
        revision += 1;
        if let Some(list) = record["timeouts_passed"].as_array_mut() {
            list.push(json!(name));
        }
        drafts.push((
            "execution.timeout.passed",
            subject(id),
            revision,
            json!({"timeout": name}),
        ));
        if name == "delivery" && effect.is_object() {
            let open: Vec<Value> = effect["obligations"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|o| o["state"] == "open")
                .cloned()
                .collect();
            set_obligation(&mut effect, "overdue");
            for obligation in open {
                drafts.push((
                    "core.effect.obligation.overdue",
                    subject(id),
                    revision,
                    json!({"effect": delivery_id, "obligation": obligation["id"]}),
                ));
            }
            // The wait for delivery evidence ended without a known outcome (owner decision 3).
            if record["delivery"] == "pending" && !mutants.on("pending-forever") {
                let dispatched = !effect["dispatch"].is_null();
                let (delivery, class) = if dispatched {
                    ("ambiguous", "delivery_timeout")
                } else {
                    ("failed_before_delivery", "delivery_timeout_before_dispatch")
                };
                let evidence = json!({"class": class, "source": "delivery timeout passed"});
                determine(&mut record, delivery, evidence.clone(), None, now, true);
                observe_effect(
                    &mut effect,
                    status_for(delivery),
                    class,
                    "delivery timeout passed",
                    now,
                );
                drafts.push((
                    "execution.delivery.observed",
                    subject(id),
                    revision,
                    json!({"delivery_id": delivery_id, "delivery": delivery, "evidence": evidence}),
                ));
            }
        }
        if name == "execution_deadline"
            && mutants.on("deadline-marks-effect-failed")
            && effect.is_object()
        {
            observe_effect(
                &mut effect,
                "failed",
                "timeout",
                "execution deadline passed",
                now,
            );
        }
    }
    if effect.is_object() {
        save_effect(tx, &delivery_id, id, &effect)?;
    }
    save(tx, id, revision, &record)?;
    for draft in drafts {
        provider_event(tx, stream, now, draft)?;
    }
    Ok(())
}

fn effect_status(effect: &Value) -> Value {
    effect["observations"]
        .as_array()
        .and_then(|o| o.last())
        .map_or(json!("pending"), |o| o["status"].clone())
}

fn public_delivery(delivery: &Value) -> Value {
    let mut out = json!({"delivery_id": delivery["delivery_id"], "delivery": delivery["delivery"], "history": delivery["history"]});
    for key in ["evidence", "proof_class", "determined_at"] {
        if let Some(value) = delivery.get(key) {
            out[key] = value.clone();
        }
    }
    out
}

/// `execution.inspect`.
pub fn inspect(store: &mut Store, id: &str, cursor: String) -> rusqlite::Result<Option<Value>> {
    let tx = store.transaction()?;
    let Some((revision, record)) = load(&tx, id)? else {
        return Ok(None);
    };
    let mut obligations = Vec::new();
    for effect_id in record["effects"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if let Some(effect) = load_effect(&tx, effect_id)? {
            obligations.extend(
                effect["obligations"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
            );
        }
    }
    let deliveries: Vec<Value> = record["deliveries"]
        .as_array()
        .into_iter()
        .flatten()
        .map(public_delivery)
        .collect();
    let mut result = json!({
        "execution": subject(id),
        "revision": revision,
        "admission": record["admission"],
        "delivery": record["delivery"],
        "runtime": record["runtime"],
        "result": record["result"],
        "exit": record["exit"],
        "evaluation": record["evaluation"],
        "deliveries": deliveries,
        "completions": record["completions"],
        "effects": record["effects"],
        "obligations": obligations,
        "recovery": record["recovery"],
        "host": record["host"],
        "next_cursor": cursor,
    });
    for key in ["predecessor", "correlation", "finalized_by", "cancellation"] {
        if let Some(value) = record.get(key) {
            result[key] = value.clone();
        }
    }
    tx.commit()?;
    Ok(Some(result))
}

/// `execution.reconcile`: scoped observations only; it never submits (EXE-10).
pub fn reconcile(
    store: &mut Store,
    execution: Option<String>,
    delivery_id: Option<&str>,
    mutants: &Mutants,
) -> rusqlite::Result<Value> {
    let tx = store.transaction()?;
    let id = match (execution, delivery_id) {
        (Some(id), _) => Some(id),
        (None, Some(delivery)) => tx
            .query_row(
                "SELECT execution FROM effects WHERE id=?1",
                [delivery],
                |r| r.get(0),
            )
            .optional()?,
        _ => None,
    };
    let empty = json!({"executions": [], "deliveries": [], "obligations": []});
    let Some(id) = id else {
        tx.commit()?;
        return Ok(empty);
    };
    let Some((revision, mut record)) = load(&tx, &id)? else {
        tx.commit()?;
        return Ok(empty);
    };
    if mutants.on("reconcile-resubmits") {
        let count = record["deliveries"].as_array().map_or(0, Vec::len) + 1;
        if let Some(list) = record["deliveries"].as_array_mut() {
            list.push(json!({"delivery_id": format!("{id}.delivery-{count}"), "delivery": "pending", "history": []}));
        }
        save(&tx, &id, revision + 1, &record)?;
    }
    let mut obligations = Vec::new();
    for effect_id in record["effects"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if let Some(effect) = load_effect(&tx, effect_id)? {
            obligations.extend(
                effect["obligations"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|o| o["state"] == "open")
                    .cloned(),
            );
        }
    }
    tx.commit()?;
    let deliveries: Vec<Value> = record["deliveries"]
        .as_array()
        .into_iter()
        .flatten()
        .map(public_delivery)
        .collect();
    Ok(json!({"executions": [subject(&id)], "deliveries": deliveries, "obligations": obligations}))
}

/// `core.effects.get` (CORE section 19.2).
pub fn effect(store: &mut Store, id: &str, mutants: &Mutants) -> rusqlite::Result<Option<Value>> {
    let tx = store.transaction()?;
    let effect = load_effect(&tx, id)?;
    tx.commit()?;
    let Some(effect) = effect else {
        return Ok(None);
    };
    let status = effect_status(&effect);
    if status == "unknown" && mutants.on("unknown-effect-not-found") {
        return Ok(None);
    }
    Ok(Some(json!({
        "effect": effect["descriptor"],
        "status": status,
        "observations": effect["observations"],
        "obligations": effect["obligations"],
    })))
}

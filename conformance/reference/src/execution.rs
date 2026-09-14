//! Reference executor for `execution/1` over a scripted fake harness (EXECUTION draft, decision 007).
//!
//! The script is test environment selected by launch configuration (`executor.scripts`). It is
//! never a protocol message. Execution records live in the subjects table (kind `execution`) so
//! Core preconditions and revisions apply unchanged; effects live in their own table.

use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use crate::mutants::Mutants;
use crate::store::{Store, append_event};

pub const KIND: &str = "execution.execution";
const CRASH_STATUS: i32 = 86;

/// Everything a command or tick needs besides the store.
pub struct Context<'a> {
    pub now: String,
    pub executor: &'a Value,
    pub mutants: &'a Mutants,
    pub principal: &'a str,
    pub grant: Option<&'a str>,
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
        "host": {"id": "scripted-host", "generation": 1},
        "submitted_at": ctx.now,
        "timeouts": payload["timeouts"].clone(),
        "timeouts_passed": [],
        "script": script,
        "script_position": 0,
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
            "dispatch_started": false,
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
        if !ctx.mutants.on("effect-recorded-after-dispatch") {
            save_effect(tx, &delivery_id, execution_id, &effect)?;
        } else {
            record["deferred_effect"] = effect;
        }
        record["deliveries"] = json!([{"delivery_id": delivery_id, "delivery": "pending"}]);
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

/// Crash recovery at process start: pending deliveries become `failed_before_delivery` when dispatch
/// never began, and `ambiguous` when it began without an observation (EXECUTION section 3).
pub fn recover(store: &mut Store, now: &str, mutants: &Mutants) -> rusqlite::Result<()> {
    let stream = store.stream_id()?;
    let tx = store.transaction()?;
    for id in execution_ids(&tx)? {
        let Some((mut revision, mut record)) = load(&tx, &id)? else {
            continue;
        };
        if record["delivery"] != "pending" || record["admission"] != "admitted" {
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
        if effect["observations"]
            .as_array()
            .is_some_and(|o| o.len() > 1)
        {
            continue;
        }
        let dispatched = effect["dispatch_started"].as_bool().unwrap_or(false);
        let (delivery, status, class, source) = if dispatched {
            (
                "ambiguous",
                "unknown",
                "dispatch_uncertain",
                "provider restarted after dispatch began",
            )
        } else if mutants.on("restart-redispatches") {
            (
                "acknowledged",
                "succeeded",
                "provider_ack_id",
                "provider restarted and dispatched again",
            )
        } else {
            (
                "failed_before_delivery",
                "failed",
                "never_dispatched",
                "provider restarted before dispatch began",
            )
        };
        if dispatched && mutants.on("ambiguity-overwritten") {
            continue;
        }
        observe_effect(&mut effect, status, class, source, now);
        if status != "unknown" {
            set_obligation(&mut effect, "satisfied");
        }
        save_effect(&tx, &delivery_id, &id, &effect)?;
        record["delivery"] = json!(delivery);
        record["deliveries"][0]["delivery"] = json!(delivery);
        if mutants.on("restart-redispatches")
            && !dispatched
            && let Some(d) = record["deliveries"].as_array_mut()
        {
            d.push(json!({"delivery_id": format!("{id}.delivery-2"), "delivery": "acknowledged", "proof_class": "provider_ack_id"}))
        }
        revision += 1;
        save(&tx, &id, revision, &record)?;
        let mut payload = json!({"delivery_id": delivery_id, "delivery": delivery, "evidence": {"class": class, "source": source}});
        if delivery == "acknowledged" {
            payload["proof_class"] = json!("provider_ack_id");
        }
        provider_event(
            &tx,
            &stream,
            now,
            (
                "execution.delivery.observed",
                subject(&id),
                revision,
                payload,
            ),
        )?;
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

    let never_dispatched = record["delivery"] == "failed_before_delivery";
    if never_dispatched && (step.get("deliver").is_some() || step.get("crash").is_some()) {
        // Recovery decided this delivery was never dispatched; a later attempt is a new execution.
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
            // Mutant effect-recorded-after-dispatch: the effect is only written at dispatch.
            if when == "after_write" {
                let mut deferred = record["deferred_effect"].take();
                deferred["dispatch_started"] = json!(true);
                save_effect(tx, &delivery_id, id, &deferred)?;
            }
            record.as_object_mut().map(|m| m.remove("deferred_effect"));
        } else if when == "after_write" && effect.is_object() {
            effect["dispatch_started"] = json!(true);
            save_effect(tx, &delivery_id, id, &effect)?;
        }
        save(tx, id, revision - 1, &record)?;
        // The script position is committed with this transaction by the caller only on success,
        // so commit explicitly before exiting.
        tx.execute_batch("COMMIT; BEGIN;")?;
        std::process::exit(CRASH_STATUS);
    } else if let Some(proof) = step["deliver"].as_str() {
        if record.get("deferred_effect").is_some() {
            effect = record["deferred_effect"].take();
            record.as_object_mut().map(|m| m.remove("deferred_effect"));
        }
        effect["dispatch_started"] = json!(true);
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
        let delivery = if proves { "acknowledged" } else { "pending" };
        observe_effect(
            &mut effect,
            if proves { "succeeded" } else { "unknown" },
            proof,
            "scripted harness",
            now,
        );
        if proves {
            set_obligation(&mut effect, "satisfied");
        }
        save_effect(tx, &delivery_id, id, &effect)?;
        record["delivery"] = json!(delivery);
        record["deliveries"][0]["delivery"] = json!(delivery);
        record["deliveries"][0]["proof_class"] = json!(proof);
        drafts.push((
            "execution.delivery.observed",
            subject(id),
            revision,
            json!({"delivery_id": delivery_id, "delivery": delivery, "proof_class": proof, "evidence": {"class": proof, "source": "scripted harness"}}),
        ));
    } else if let Some(found) = step["reconcile_finds"].as_str() {
        let status = match found {
            "delivered" => "succeeded",
            "not_delivered" => "failed",
            _ => "unknown",
        };
        observe_effect(
            &mut effect,
            status,
            "reconciliation",
            "scripted harness reconciliation",
            now,
        );
        if status != "unknown" {
            set_obligation(&mut effect, "satisfied");
        }
        save_effect(tx, &delivery_id, id, &effect)?;
        record["deliveries"][0]["reconciliation"] = json!(found);
        drafts.push((
            "execution.delivery.reconciled",
            subject(id),
            revision,
            json!({"delivery_id": delivery_id, "outcome": found, "evidence": {"class": "reconciliation", "source": "scripted harness reconciliation"}}),
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
        let generation = record["host"]["generation"].as_i64().unwrap_or(1) + 1;
        record["host"]["generation"] = json!(generation);
        record["runtime"] = json!("unknown");
        drafts.push((
            "execution.host.changed",
            subject(id),
            revision,
            json!({"host": record["host"]}),
        ));
    } else if let Some(completion) = step.get("complete") {
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
        if let Some(c) = record["completions"].as_array_mut() {
            c.push(json!({"completion_id": completion_id, "digest": digest, "generation": generation, "status": status}))
        }
        drafts.push((
            "execution.completion.recorded",
            subject(id),
            revision,
            json!({"completion_id": completion_id, "digest": digest, "host": {"id": record["host"]["id"], "generation": generation}, "status": status}),
        ));
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
            matches!(record["delivery"].as_str(), Some("pending" | "ambiguous")),
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
    for name in due {
        if passed.contains(&name.to_string()) {
            continue;
        }
        revision += 1;
        if let Some(p) = record["timeouts_passed"].as_array_mut() {
            p.push(json!(name))
        }
        provider_event(
            tx,
            stream,
            now,
            (
                "execution.timeout.passed",
                subject(id),
                revision,
                json!({"timeout": name}),
            ),
        )?;
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
                provider_event(
                    tx,
                    stream,
                    now,
                    (
                        "core.effect.obligation.overdue",
                        subject(id),
                        revision,
                        json!({"effect": delivery_id, "obligation": obligation["id"]}),
                    ),
                )?;
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
    save(tx, id, revision, &record)
}

fn effect_status(effect: &Value) -> Value {
    effect["observations"]
        .as_array()
        .and_then(|o| o.last())
        .map_or(json!("pending"), |o| o["status"].clone())
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
    let mut result = json!({
        "execution": subject(id),
        "revision": revision,
        "admission": record["admission"],
        "delivery": record["delivery"],
        "runtime": record["runtime"],
        "result": record["result"],
        "exit": record["exit"],
        "evaluation": record["evaluation"],
        "deliveries": record["deliveries"],
        "completions": record["completions"],
        "effects": record["effects"],
        "obligations": obligations,
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
    let Some(id) = id else {
        tx.commit()?;
        return Ok(json!({"executions": [], "deliveries": [], "obligations": []}));
    };
    let Some((revision, mut record)) = load(&tx, &id)? else {
        tx.commit()?;
        return Ok(json!({"executions": [], "deliveries": [], "obligations": []}));
    };
    if mutants.on("reconcile-resubmits") {
        let count = record["deliveries"].as_array().map_or(0, Vec::len) + 1;
        if let Some(d) = record["deliveries"].as_array_mut() {
            d.push(json!({"delivery_id": format!("{id}.delivery-{count}"), "delivery": "pending"}))
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
    Ok(
        json!({"executions": [subject(&id)], "deliveries": record["deliveries"], "obligations": obligations}),
    )
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

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
//!
//! Optional features (EXECUTION section 11) keep their state in the same record; their commands
//! are in `features.rs`.

use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use crate::mutants::Mutants;
use crate::store::{Store, append_event};

pub const KIND: &str = "execution.execution";
pub const CONTROLLER_KIND: &str = "execution.controller";
const CRASH_STATUS: i32 = 86;
/// Attempts an executor makes for one effect when its retry class permits retries (CORE 19.3).
const MAX_ATTEMPTS: usize = 3;

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

pub(crate) type Draft = (&'static str, Value, i64, Value);
/// (revision, outcome, events, effect references) of one applied command.
pub(crate) type Applied = (i64, Value, Vec<Draft>, Vec<String>);

/// The scripted executor's single host slot.
pub fn host_id(executor: &Value) -> &str {
    executor["host_id"].as_str().unwrap_or("scripted-host")
}

pub(crate) fn subject(id: &str) -> Value {
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

pub(crate) fn save(
    tx: &Transaction,
    id: &str,
    revision: i64,
    record: &Value,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO subjects VALUES (?1, ?2, ?3, ?4, 1)
         ON CONFLICT (kind, id) DO UPDATE SET revision=excluded.revision, value=excluded.value,
         applied_count=applied_count+1",
        params![KIND, id, revision, record.to_string()],
    )?;
    Ok(())
}

pub(crate) fn load_effect(tx: &Transaction, id: &str) -> rusqlite::Result<Option<Value>> {
    let row: Option<String> = tx
        .query_row("SELECT record FROM effects WHERE id=?1", [id], |r| r.get(0))
        .optional()?;
    Ok(row.map(|record| serde_json::from_str(&record).unwrap_or(Value::Null)))
}

/// Save an effect record, advancing its revision (the precondition revision of `core.effect`).
pub(crate) fn save_effect(
    tx: &Transaction,
    id: &str,
    execution: &str,
    record: &Value,
) -> rusqlite::Result<i64> {
    let mut record = record.clone();
    let revision = record["revision"].as_i64().unwrap_or(0) + 1;
    record["revision"] = json!(revision);
    tx.execute(
        "INSERT INTO effects VALUES (?1, ?2, ?3) ON CONFLICT (id) DO UPDATE SET record=excluded.record",
        params![id, execution, record.to_string()],
    )?;
    Ok(revision)
}

pub(crate) fn observe_effect(
    effect: &mut Value,
    status: &str,
    class: &str,
    source: &str,
    now: &str,
) {
    if let Some(list) = effect["observations"].as_array_mut() {
        list.push(json!({"status": status, "evidence": {"class": class, "source": source}, "recorded_at": now}));
    }
}

pub(crate) fn set_obligation(effect: &mut Value, state: &str) {
    for obligation in effect["obligations"].as_array_mut().into_iter().flatten() {
        if obligation["state"] == "open" {
            obligation["state"] = json!(state);
        }
    }
}

/// Append to an array member, creating it when absent.
pub(crate) fn push(value: &mut Value, key: &str, item: Value) {
    if !value[key].is_array() {
        value[key] = json!([]);
    }
    if let Some(list) = value[key].as_array_mut() {
        list.push(item);
    }
}

fn count_prefixed(record: &Value, key: &str, marker: &str) -> usize {
    record[key]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|id| id.contains(marker))
        .count()
}

/// A new effect record, observed as recorded before any external I/O (CORE section 19.1).
#[allow(clippy::too_many_arguments)]
pub(crate) fn new_effect(
    id: &str,
    kind: &str,
    execution_id: &str,
    payload_digest: &Value,
    record: &Value,
    retry_class: &str,
    operation_ref: &Value,
    obligation: Option<(&str, &str, Value)>,
    now: &str,
    source: &str,
) -> Value {
    let mut effect = json!({
        "descriptor": {
            "id": id,
            "kind": kind,
            "target": subject(execution_id),
            "payload_digest": payload_digest,
            "authorization": {"principal": record["authorization"]["principal"]},
            "retry_class": retry_class,
            "operation_ref": operation_ref,
        },
        "observations": [],
        "attempts": [],
        "obligations": [],
        "dispatch": Value::Null,
    });
    if let Some(grant) = record["authorization"]["grant"].as_str() {
        effect["descriptor"]["authorization"]["grant"] = json!(grant);
    }
    if retry_class == "idempotent_key" {
        effect["descriptor"]["idempotency_key"] = json!(id);
    }
    if let Some((suffix, expects, deadline)) = obligation {
        effect["obligations"] = json!([{"id": format!("{id}.{suffix}"), "expects": expects, "deadline": deadline, "state": "open"}]);
    }
    observe_effect(
        &mut effect,
        "pending",
        "recorded_before_dispatch",
        source,
        now,
    );
    effect
}

/// Try an effect against the scripted harness, retrying only as its retry class permits
/// (CORE section 19.3). Scripted transport errors make an attempt's outcome unknown. Returns
/// whether an attempt completed.
fn run_attempts(effect: &mut Value, record: &mut Value, now: &str, mutants: &Mutants) -> bool {
    let class = effect["descriptor"]["retry_class"]
        .as_str()
        .unwrap_or("non_repeatable")
        .to_string();
    let key = effect["descriptor"]["idempotency_key"]
        .as_str()
        .map(String::from);
    let mut errors = record["pending_transport_errors"].as_i64().unwrap_or(0);
    let mut completed = false;
    for number in 1..=MAX_ATTEMPTS {
        let mut attempt = json!({"attempt": number, "recorded_at": now});
        if let Some(key) = &key {
            attempt["idempotency_key"] = if number > 1 && mutants.on("idempotent-retry-new-key") {
                json!(format!("{key}.retry-{number}"))
            } else {
                json!(key)
            };
        }
        if errors > 0 {
            errors -= 1;
            attempt["outcome"] = json!("unknown");
            push(effect, "attempts", attempt);
            let retry = match class.as_str() {
                "pure" => true,
                "read" => !mutants.on("read-never-retried"),
                "idempotent_key" => key.is_some(),
                _ => mutants.on("non-repeatable-retried"),
            };
            if !retry {
                break;
            }
        } else {
            attempt["outcome"] = json!("completed");
            push(effect, "attempts", attempt);
            completed = true;
            break;
        }
    }
    record["pending_transport_errors"] = json!(errors);
    completed
}

fn enforcement_rank(level: &str) -> i32 {
    match level {
        "enforced" => 3,
        "mediated" => 2,
        "cooperative" => 1,
        _ => 0,
    }
}

pub(crate) fn add_seconds(instant: &str, seconds: i64) -> String {
    crate::clock::from_seconds(crate::clock::to_seconds(instant) + seconds)
}

/// Effect status implied by a delivery determination (CORE section 19).
pub(crate) fn status_for(delivery: &str) -> &'static str {
    match delivery {
        "acknowledged" | "delivered" => "succeeded",
        "not_delivered" | "failed_before_delivery" => "failed",
        "ambiguous" => "unknown",
        _ => "pending",
    }
}

/// Replace the current delivery determination, keeping the previous one in history.
pub(crate) fn determine(
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
    if delivery == "ambiguous" {
        record["ambiguous_since"] = json!(now);
    }
    // Durable evidence that the invocation was never dispatched and never will be under this
    // identity releases its budget reservation (EXECUTION section 12).
    if delivery == "failed_before_delivery" && record["budget"]["reservation"] == "reserved" {
        record["budget"]["reservation"] = json!("released");
    }
}

fn operation_sequence(record: &Value) -> i64 {
    record["operation_ref"]
        .as_str()
        .and_then(|r| r.strip_prefix("op-"))
        .and_then(|n| n.parse().ok())
        .unwrap_or(i64::MAX)
}

/// Admitted executions that have not exited (the capacity in use).
fn running(tx: &Transaction) -> rusqlite::Result<i64> {
    let mut count = 0;
    for id in execution_ids(tx)? {
        if let Some((_, record)) = load(tx, &id)?
            && record["admission"] == "admitted"
            && record["runtime"] != "exited"
            && !matches!(
                record["delivery"].as_str(),
                Some("failed_before_delivery" | "not_delivered")
            )
        {
            count += 1;
        }
    }
    Ok(count)
}

/// Amount held in a budget pool: active reservations and settled consumption.
fn held_in_pool(tx: &Transaction, pool: &str) -> rusqlite::Result<i64> {
    let mut held = 0;
    for id in execution_ids(tx)? {
        if let Some((_, record)) = load(tx, &id)?
            && record["budget"]["pool"] == pool
            && matches!(
                record["budget"]["reservation"].as_str(),
                Some("reserved" | "settled")
            )
        {
            held += record["budget"]["amount"].as_i64().unwrap_or(1);
        }
    }
    Ok(held)
}

/// Record the prompt delivery effect and its delivery slot for an admitted execution.
fn open_delivery(
    tx: &Transaction,
    execution_id: &str,
    record: &mut Value,
    now: &str,
    source: &str,
    mutants: &Mutants,
) -> rusqlite::Result<String> {
    let delivery_id = format!("{execution_id}.delivery-1");
    let deadline = record["timeouts"]["delivery"]
        .as_i64()
        .map_or(Value::Null, |seconds| json!(add_seconds(now, seconds)));
    let effect = new_effect(
        &delivery_id,
        "execution.prompt_submission",
        execution_id,
        &record["brief_digest"],
        record,
        "non_repeatable",
        &record["operation_ref"],
        Some(("evidence", "delivery evidence", deadline)),
        now,
        source,
    );
    if mutants.on("effect-recorded-after-dispatch") {
        record["deferred_effect"] = effect;
    } else {
        save_effect(tx, &delivery_id, execution_id, &effect)?;
    }
    record["deliveries"] = json!([{"delivery_id": delivery_id, "delivery": "pending", "determined_at": now, "history": []}]);
    push(record, "effects", json!(delivery_id));
    Ok(delivery_id)
}

/// `execution.submit` inside the Core owner transaction (CORE section 10 step 8).
pub fn submit(
    tx: &Transaction,
    execution_id: &str,
    payload: &Value,
    sequence: i64,
    ctx: &Context,
) -> rusqlite::Result<Applied> {
    let adapter = &ctx.executor["adapter"];
    let mutants = ctx.mutants;
    let advertised = adapter["enforcement"].as_str().unwrap_or("cooperative");
    let mut refusal: Option<(&str, String)> = None;
    for required in payload["restrictions"].as_array().into_iter().flatten() {
        let wanted = required["enforcement"].as_str().unwrap_or_default();
        if enforcement_rank(wanted) > enforcement_rank(advertised)
            && !mutants.on("admits-weaker-enforcement")
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
        let status = predicate_status(adapter, name.as_str().unwrap_or_default());
        if status != "supported" && !mutants.on("unknown-predicate-supported") {
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
        "admission": "admitted",
        "delivery": "pending",
        "runtime": "preparing",
        "result": "absent",
        "exit": "unavailable",
        "evaluation": "not_requested",
        "deliveries": [],
        "completions": [],
        "effects": [],
        "recovery": [],
        "host": {"id": host_id(ctx.executor), "generation": 1},
        "submitted_at": ctx.now,
        "admitted_at": ctx.now,
        "last_observation_at": ctx.now,
        "operation_ref": format!("op-{sequence}"),
        "brief_digest": payload["brief"]["digest"],
        "timeouts": payload["timeouts"].clone(),
        "timeouts_passed": [],
        "script": script,
        "script_position": 0,
        "authorization": {"principal": ctx.principal, "grant": ctx.grant},
    });
    if let Some(predecessor) = payload.get("predecessor")
        && !mutants.on("predecessor-dropped")
    {
        record["predecessor"] = predecessor.clone();
    }
    if let Some(correlation) = payload.get("correlation")
        && !mutants.on("correlation-dropped")
    {
        record["correlation"] = correlation.clone();
    }
    // Usage and liability (EXECUTION section 12).
    if let Some(budget) = payload.get("budget") {
        let pool_id = budget["pool"].as_str().unwrap_or_default();
        let ceiling = budget["ceiling"].as_str().unwrap_or("soft");
        let amount = budget["amount"].as_i64().unwrap_or(1);
        record["budget"] = json!({"pool": pool_id, "ceiling": ceiling, "amount": amount, "reservation": "not_reserved"});
        match ctx.executor["budget_pools"].get(pool_id) {
            _ if refusal.is_some() => {}
            None => {
                refusal = Some((
                    "budget_unavailable",
                    "name a budget pool this executor defines".to_string(),
                ));
            }
            Some(pool) => {
                let measure = pool["measure"].as_str().unwrap_or("invocations");
                record["budget"]["measure"] = json!(measure);
                let enforced = adapter["enforced_bounds"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|bound| bound == measure);
                if ceiling == "hard" && !enforced && !mutants.on("admits-unenforceable-ceiling") {
                    refusal = Some((
                        "enforcement_unavailable",
                        format!(
                            "request a soft ceiling, or use an executor that enforces a {measure} bound"
                        ),
                    ));
                } else if held_in_pool(tx, pool_id)? + amount > pool["limit"].as_i64().unwrap_or(0)
                {
                    refusal = Some((
                        "budget_exhausted",
                        format!(
                            "wait until reservations in budget pool {pool_id} are released or settled"
                        ),
                    ));
                }
            }
        }
    }
    let mut queue_reason: Option<&str> = None;
    // Context bindings (EXECUTION section 13).
    if let Some(bindings) = payload["context_bindings"].as_array() {
        let held = ctx.executor["context_packets"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let mut states = Vec::new();
        for binding in bindings {
            let packet = &binding["packet"];
            let holds = held.iter().any(|p| {
                p["ref"] == packet["ref"]
                    && (p["digest"] == packet["digest"] || mutants.on("digest-mismatch-satisfies"))
            });
            let obligation = binding["obligation"].as_str().unwrap_or_default();
            let state = match (holds, obligation) {
                (true, _) => "satisfied",
                (false, "advisory") => "gap",
                _ => "unsatisfied",
            };
            if !holds
                && obligation == "required_before_start"
                && !mutants.on("required-binding-admitted")
            {
                queue_reason = Some("context_binding_unsatisfied");
            }
            let mut entry = binding.clone();
            entry["state"] = json!(state);
            states.push(entry);
        }
        record["context"] = json!({"bindings": states, "deliveries": []});
    }
    if let Some(workspace) = payload.get("workspace") {
        let mut lease = workspace.clone();
        lease["lease_id"] = json!(format!("{execution_id}.workspace"));
        lease["writer"] = json!(ctx.principal);
        lease["lease_epoch"] = json!(1);
        record["workspace"] = json!({"lease": lease, "checkpoints": [], "annotations": []});
    }
    if let Some(continuation) = payload.get("continuation") {
        let mode = continuation["mode"].as_str().unwrap_or("resume");
        let predicate = if mode == "fork" {
            "adapter.fork"
        } else {
            "adapter.resume"
        };
        let label = if predicate_status(adapter, predicate) == "supported" {
            if mode == "fork" { "forked" } else { "resumed" }
        } else if mode == "resume" && mutants.on("fresh-labeled-resumed") {
            "resumed"
        } else {
            "fresh_continuation"
        };
        record["continuation"] =
            json!({"of": continuation["of"], "requested": mode, "label": label});
    }
    if refusal.is_none()
        && queue_reason.is_none()
        && let Some(capacity) = ctx.executor["capacity"].as_i64()
        && !mutants.on("capacity-ignored")
        && running(tx)? >= capacity
    {
        queue_reason = Some("capacity");
    }
    let admission = match (&refusal, queue_reason) {
        (Some(_), _) => "refused",
        (None, Some(_)) => "queued",
        (None, None) => "admitted",
    };
    record["admission"] = json!(admission);
    if admission == "refused" {
        // Never dispatched and never will be: the axis does not stay pending (EXECUTION 3.1).
        record["delivery"] = json!("failed_before_delivery");
    }
    let mut outcome = json!({"execution": subject(execution_id), "admission": admission});
    let mut event = json!({"admission": admission});
    let mut effect_refs = Vec::new();
    if let Some((reason, alternative)) = refusal {
        for target in [&mut outcome, &mut record] {
            target["reason"] = json!(reason);
            target["alternative"] = json!(alternative);
        }
        event["reason"] = json!(reason);
    } else {
        if record["budget"].is_object() {
            record["budget"]["reservation"] = json!("reserved");
        }
        if let Some(reason) = queue_reason {
            outcome["queue_reason"] = json!(reason);
            record["queue_reason"] = json!(reason);
            event["queue_reason"] = json!(reason);
        } else {
            let delivery_id = open_delivery(
                tx,
                execution_id,
                &mut record,
                &ctx.now,
                "execution.submit owner transaction",
                mutants,
            )?;
            outcome["delivery_id"] = json!(delivery_id);
            effect_refs.push(delivery_id);
        }
    }
    if let Some(continuation) = record.get("continuation") {
        outcome["continuation"] = continuation.clone();
    }
    save(tx, execution_id, 1, &record)?;
    let events = vec![(
        "execution.admission.changed",
        subject(execution_id),
        1,
        event,
    )];
    Ok((1, outcome, events, effect_refs))
}

fn predicate_status<'a>(adapter: &'a Value, name: &str) -> &'a str {
    adapter["predicates"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["name"] == name)
        .and_then(|p| p["status"].as_str())
        .unwrap_or("unknown")
}

/// `execution.cancel`: records the request and its forwarding effect and returns a request
/// receipt (EXECUTION section 7). The harness is asked later; the outcome is observed separately.
pub fn cancel(
    tx: &Transaction,
    execution_id: &str,
    sequence: i64,
    ctx: &Context,
) -> rusqlite::Result<Applied> {
    let (revision, mut record) = load(tx, execution_id)?.unwrap_or((0, json!({})));
    let operation_ref = format!("op-{sequence}");
    let receipt = json!({"state": "cancel_requested", "operation_ref": operation_ref});
    let effect_id = format!(
        "{execution_id}.cancel-{}",
        count_prefixed(&record, "effects", ".cancel-") + 1
    );
    let effect = new_effect(
        &effect_id,
        "execution.cancel_forwarding",
        execution_id,
        &json!(crate::json::sha256_digest(operation_ref.as_bytes())),
        &record,
        "idempotent_key",
        &json!(operation_ref),
        Some(("outcome", "cancellation outcome", Value::Null)),
        &ctx.now,
        "execution.cancel owner transaction",
    );
    save_effect(tx, &effect_id, execution_id, &effect)?;
    push(&mut record, "effects", json!(effect_id));
    record["cancellation"] = json!({"receipt": receipt, "effect": effect_id});
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
    Ok((
        revision,
        json!({"receipt": receipt}),
        events,
        vec![effect_id],
    ))
}

pub(crate) fn provider_event(
    tx: &Transaction,
    stream: &str,
    now: &str,
    draft: Draft,
) -> rusqlite::Result<()> {
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

pub(crate) fn execution_ids(tx: &Transaction) -> rusqlite::Result<Vec<String>> {
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

fn started_at(record: &Value) -> String {
    record["admitted_at"]
        .as_str()
        .or_else(|| record["submitted_at"].as_str())
        .unwrap_or_default()
        .to_string()
}

fn deadline_passed(record: &Value, now: &str) -> bool {
    let started = started_at(record);
    ["delivery", "execution_deadline"].iter().any(|name| {
        record["timeouts"][*name]
            .as_i64()
            .is_some_and(|seconds| now >= add_seconds(&started, seconds).as_str())
    })
}

pub(crate) fn bump_generation(record: &mut Value) {
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
/// A delivery already declared `failed_before_delivery` is never reopened. Executions whose host
/// survived keep their host generation and are reattached, not respawned (SCN-13); pending native
/// action requests survive (SCN-12).
pub fn recover(store: &mut Store, recovery: &Recovery) -> rusqlite::Result<()> {
    let stream = store.stream_id()?;
    let tx = store.transaction()?;
    let now = recovery.now.as_str();
    let mutants = recovery.mutants;
    for id in execution_ids(&tx)? {
        let Some((mut revision, mut record)) = load(&tx, &id)? else {
            continue;
        };
        if mutants.on("actions-lost-on-restart")
            && record["actions"]
                .as_array()
                .is_some_and(|list| list.iter().any(|a| a["state"] == "pending"))
        {
            if let Some(list) = record["actions"].as_array_mut() {
                list.retain(|a| a["state"] != "pending");
            }
            record["runtime"] = json!("unknown");
            if let Some(map) = record.as_object_mut() {
                map.remove("runtime_detail");
            }
            revision += 1;
            save(&tx, &id, revision, &record)?;
        }
        if mutants.on("respawn-on-restart")
            && record["admission"] == "admitted"
            && matches!(
                record["delivery"].as_str(),
                Some("acknowledged" | "delivered")
            )
            && record["runtime"] != "exited"
        {
            bump_generation(&mut record);
            let count = record["deliveries"].as_array().map_or(0, Vec::len) + 1;
            let delivery_id = format!("{id}.delivery-{count}");
            let effect = new_effect(
                &delivery_id,
                "execution.prompt_submission",
                &id,
                &record["brief_digest"],
                &record,
                "non_repeatable",
                &record["operation_ref"],
                None,
                now,
                "respawn",
            );
            save_effect(&tx, &delivery_id, &id, &effect)?;
            push(
                &mut record,
                "deliveries",
                json!({"delivery_id": delivery_id, "delivery": "pending", "determined_at": now, "history": []}),
            );
            push(&mut record, "effects", json!(delivery_id));
            save(&tx, &id, revision + 1, &record)?;
            continue;
        }
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
                bump_generation(&mut record);
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
        if !mutants.on("recovery-host-change-silent") {
            drafts.push((
                "execution.host.changed",
                subject(&id),
                revision,
                json!({"host": record["host"]}),
            ));
        }
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

/// Mutant `cancels-on-disconnect` only: a closing session cancels every running execution.
pub fn cancel_on_disconnect(store: &mut Store, now: &str) -> rusqlite::Result<()> {
    let tx = store.transaction()?;
    for id in execution_ids(&tx)? {
        if let Some((revision, mut record)) = load(&tx, &id)?
            && record["admission"] == "admitted"
            && record["runtime"] != "exited"
            && record.get("cancellation").is_none()
        {
            record["cancellation"] = json!({"receipt": {"state": "cancel_requested", "operation_ref": "op-disconnect"}, "outcome": "cancelled"});
            save(&tx, &id, revision + 1, &record)?;
        }
    }
    let _ = now;
    tx.commit()
}

/// Advance every execution's script and timeouts as far as the provider clock allows, then admit
/// executions waiting for capacity.
pub fn tick(
    store: &mut Store,
    now: &str,
    executor: &Value,
    mutants: &Mutants,
) -> rusqlite::Result<()> {
    let stream = store.stream_id()?;
    loop {
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
        if !admit_from_queue(store, &stream, now, executor, mutants)? {
            return Ok(());
        }
    }
}

/// Admit executions queued for capacity, oldest first, while capacity allows (EXE-2).
fn admit_from_queue(
    store: &mut Store,
    stream: &str,
    now: &str,
    executor: &Value,
    mutants: &Mutants,
) -> rusqlite::Result<bool> {
    let tx = store.transaction()?;
    let mut queued = Vec::new();
    for id in execution_ids(&tx)? {
        if let Some((revision, record)) = load(&tx, &id)?
            && record["admission"] == "queued"
            && record["queue_reason"] == "capacity"
        {
            queued.push((operation_sequence(&record), id, revision, record));
        }
    }
    queued.sort_by_key(|entry| entry.0);
    let mut admitted = false;
    for (_, id, revision, mut record) in queued {
        if executor["capacity"]
            .as_i64()
            .is_some_and(|capacity| running(&tx).is_ok_and(|n| n >= capacity))
        {
            break;
        }
        record["admission"] = json!("admitted");
        record["admitted_at"] = json!(now);
        record["last_observation_at"] = json!(now);
        if let Some(map) = record.as_object_mut() {
            map.remove("queue_reason");
        }
        let delivery_id = open_delivery(
            &tx,
            &id,
            &mut record,
            now,
            "admission from the capacity queue",
            mutants,
        )?;
        save(&tx, &id, revision + 1, &record)?;
        provider_event(
            &tx,
            stream,
            now,
            (
                "execution.admission.changed",
                subject(&id),
                revision + 1,
                json!({"admission": "admitted", "delivery_id": delivery_id}),
            ),
        )?;
        admitted = true;
    }
    tx.commit()?;
    Ok(admitted)
}

/// Record the write-ahead dispatch marker before any harness write.
pub(crate) fn mark_dispatch(effect: &mut Value, generation: &Value, now: &str) {
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

fn liability(record: &Value) -> &'static str {
    let observations = record["usage"]["observations"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if observations.is_empty() {
        return "none";
    }
    let mut latest: Vec<(Value, Value)> = Vec::new();
    for observation in observations {
        let invocation = observation["invocation_id"].clone();
        latest.retain(|(id, _)| *id != invocation);
        latest.push((invocation, observation["basis"].clone()));
    }
    if latest
        .iter()
        .all(|(_, basis)| basis == "observed" || basis == "enforced_bound")
    {
        "resolved"
    } else {
        "unresolved"
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
    } else if let Some(what) = step["wait_for"].as_str() {
        let ready = match what {
            "cancel" => record.get("cancellation").is_some(),
            "steer" => record["steering"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|s| s["request"] == "recorded"),
            "action" => record["actions"].as_array().is_some_and(|list| {
                !list.is_empty() && list.iter().all(|a| a["state"] == "answered")
            }),
            _ => false,
        };
        if !ready {
            return Ok(false);
        }
        if what == "action" {
            dispatch_action_responses(tx, id, &mut record, &generation, now, mutants)?;
            record["runtime"] = json!("active");
            if let Some(map) = record.as_object_mut() {
                map.remove("runtime_detail");
            }
            drafts.push((
                "execution.runtime.changed",
                subject(id),
                revision,
                json!({"runtime": "active"}),
            ));
        }
    } else if let Some(outcome) = step["on_cancel"].as_str() {
        if record.get("cancellation").is_none() {
            return Ok(false);
        }
        if record["cancellation"].get("outcome").is_none() {
            let mut outcome = outcome.to_string();
            if let Some(effect_id) = record["cancellation"]["effect"].as_str().map(String::from)
                && let Some(mut forwarding) = load_effect(tx, &effect_id)?
            {
                mark_dispatch(&mut forwarding, &generation, now);
                if run_attempts(&mut forwarding, &mut record, now, mutants) {
                    observe_effect(
                        &mut forwarding,
                        "succeeded",
                        "harness_response",
                        "scripted harness cancellation response",
                        now,
                    );
                    set_obligation(&mut forwarding, "satisfied");
                } else {
                    observe_effect(
                        &mut forwarding,
                        "unknown",
                        "transport_error",
                        "scripted harness transport",
                        now,
                    );
                    outcome = "unknown".to_string();
                }
                save_effect(tx, &effect_id, id, &forwarding)?;
            }
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
        if stale != current && !mutants.on("stale-dispatcher-sends") {
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
        // A later report while delivery is still pending is evidence for the same dispatch, not a
        // second attempt (EXECUTION 15.1).
        let sent = if effect.is_object() && effect["dispatch"].is_null() {
            mark_dispatch(&mut effect, &generation, now);
            run_attempts(&mut effect, &mut record, now, mutants)
        } else {
            true
        };
        if !sent {
            // A non-repeatable submission whose attempt outcome is unknown is never retried
            // automatically (CORE section 19.3); the delivery is ambiguous until reconciled.
            let evidence =
                json!({"class": "transport_error", "source": "scripted harness transport"});
            observe_effect(
                &mut effect,
                "unknown",
                "transport_error",
                "scripted harness transport",
                now,
            );
            determine(&mut record, "ambiguous", evidence.clone(), None, now, true);
            drafts.push((
                "execution.delivery.observed",
                subject(id),
                revision,
                json!({"delivery_id": delivery_id, "delivery": "ambiguous", "evidence": evidence}),
            ));
        } else {
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
            drafts.push((
                "execution.delivery.observed",
                subject(id),
                revision,
                json!({"delivery_id": delivery_id, "delivery": record["delivery"], "proof_class": proof, "evidence": evidence}),
            ));
        }
        if effect.is_object() {
            save_effect(tx, &delivery_id, id, &effect)?;
        }
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
        drafts.push((
            "execution.runtime.changed",
            subject(id),
            revision,
            json!({"runtime": runtime}),
        ));
    } else if let Some(request) = step.get("request_action") {
        push(
            &mut record,
            "actions",
            json!({"action_id": request["action_id"], "owner": request["owner"], "state": "pending", "requested_at": now}),
        );
        record["runtime"] = json!("requires_action");
        let mut payload = json!({"runtime": "requires_action"});
        if !mutants.on("requires-action-without-identity") {
            let detail = json!({"action_id": request["action_id"], "owner": request["owner"]});
            record["runtime_detail"] = detail.clone();
            payload["action_id"] = detail["action_id"].clone();
            payload["owner"] = detail["owner"].clone();
        }
        drafts.push(("execution.runtime.changed", subject(id), revision, payload));
    } else if let Some(proof) = step["steer_deliver"].as_str() {
        if !deliver_steering(
            tx,
            id,
            &mut record,
            proof,
            &generation,
            now,
            mutants,
            revision,
            &mut drafts,
        )? {
            return Ok(false);
        }
    } else if step.get("steer_behavior").is_some() {
        let Some(entry) = record["steering"]
            .as_array_mut()
            .and_then(|list| list.iter_mut().rev().find(|s| s["request"] == "recorded"))
        else {
            return Ok(false);
        };
        let evidence = json!({"class": "harness_observation", "source": "scripted harness"});
        entry["behavior"] = json!("observed");
        entry["behavior_evidence"] = evidence.clone();
        drafts.push((
            "execution.steer.behavior.observed",
            subject(id),
            revision,
            json!({"steer_id": entry["steer_id"], "evidence": evidence}),
        ));
    } else if let Some(probe) = step.get("workspace") {
        if record["workspace"].is_object() {
            let mut probe = probe.clone();
            probe["probed_at"] = json!(now);
            record["workspace"]["probe"] = probe;
        }
    } else if let Some(commit) = step["agent_reports_commit"].as_str() {
        if record["workspace"].is_object() {
            push(
                &mut record["workspace"],
                "annotations",
                json!({"kind": "agent_reported_commit", "value": commit, "basis": "agent_report", "recorded_at": now}),
            );
        }
    } else if let Some(usage) = step.get("usage") {
        let mut observation = usage.clone();
        observation["recorded_at"] = json!(now);
        if !record["usage"].is_object() {
            record["usage"] = json!({"observations": []});
        }
        push(&mut record["usage"], "observations", observation.clone());
        if liability(&record) == "resolved" && record["budget"]["reservation"] == "reserved" {
            record["budget"]["reservation"] = json!("settled");
        }
        drafts.push((
            "execution.usage.observed",
            subject(id),
            revision,
            observation,
        ));
    } else if let Some(delivery) = step.get("context_delivery") {
        let binding = record["context"]["bindings"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|b| b["binding_id"] == delivery["binding_id"])
            .cloned()
            .unwrap_or(Value::Null);
        let boundary = delivery["boundary"].as_str().unwrap_or_default();
        let supported = executor["adapter"]["context_boundaries"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|b| b == boundary);
        let after_transition = binding["obligation"] == "required_before_transition"
            && record["transitions"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|t| *t == binding["transition"]);
        let outcome = if !supported {
            "unavailable"
        } else if after_transition && !mutants.on("no-late-state") {
            "late"
        } else {
            match delivery["harness"].as_str() {
                Some("acknowledged") => "acknowledged",
                Some("accepted") => "delivered",
                Some("queued") => "queued",
                _ => "unknown",
            }
        };
        let entry = json!({
            "binding_id": delivery["binding_id"],
            "digest": binding["packet"]["digest"],
            "target": {"execution": subject(id)},
            "boundary": boundary,
            "outcome": outcome,
            "recorded_at": now,
        });
        if record["context"].is_object() {
            push(&mut record["context"], "deliveries", entry.clone());
        }
        drafts.push((
            "execution.context.delivery.observed",
            subject(id),
            revision,
            entry,
        ));
    } else if let Some(transition) = step["transition"].as_str() {
        push(&mut record, "transitions", json!(transition));
        drafts.push((
            "execution.transition.observed",
            subject(id),
            revision,
            json!({"transition": transition}),
        ));
    } else if let Some(output) = step.get("output") {
        let mut bytes = output["text"]
            .as_str()
            .unwrap_or_default()
            .as_bytes()
            .to_vec();
        if let (Some(unit), Some(count)) = (output["repeat"].as_str(), output["bytes"].as_u64()) {
            bytes.extend(unit.bytes().cycle().take(count as usize));
        }
        let spool = executor["output_spool_bytes"].as_u64().unwrap_or(65_536) as usize;
        append_output(tx, id, &bytes, spool, mutants, revision, &mut drafts)?;
    } else if let Some(lost) = step.get("output_lost") {
        let mut output = load_output(tx, id)?;
        let end = output["end"].clone();
        let mut range =
            json!({"from": end, "to": end, "reason": lost["reason"], "coverage": "incomplete"});
        if let Some(bytes) = lost.get("bytes") {
            range["bytes"] = bytes.clone();
        }
        push(&mut output, "lost_ranges", range.clone());
        save_output(tx, id, &output)?;
        drafts.push(("execution.output.lost", subject(id), revision, range));
    } else if let Some(count) = step["runtime_burst"].as_u64() {
        for index in 0..count {
            let runtime = if index % 2 == 0 {
                "quiescent"
            } else {
                "active"
            };
            record["runtime"] = json!(runtime);
            drafts.push((
                "execution.runtime.changed",
                subject(id),
                revision,
                json!({"runtime": runtime}),
            ));
        }
    } else if let Some(count) = step["transport_errors"].as_i64() {
        record["pending_transport_errors"] = json!(count);
    } else if step.get("probe_status").is_some() {
        let probe_id = format!(
            "{id}.probe-{}",
            count_prefixed(&record, "effects", ".probe-") + 1
        );
        let mut probe = new_effect(
            &probe_id,
            "execution.status_probe",
            id,
            &json!(crate::json::sha256_digest(probe_id.as_bytes())),
            &record,
            "read",
            &record["operation_ref"],
            Some(("result", "harness status", Value::Null)),
            now,
            "scripted executor status probe",
        );
        mark_dispatch(&mut probe, &generation, now);
        if run_attempts(&mut probe, &mut record, now, mutants) {
            observe_effect(
                &mut probe,
                "succeeded",
                "harness_status",
                "scripted harness",
                now,
            );
            set_obligation(&mut probe, "satisfied");
        } else {
            observe_effect(
                &mut probe,
                "unknown",
                "transport_error",
                "scripted harness transport",
                now,
            );
        }
        save_effect(tx, &probe_id, id, &probe)?;
        push(&mut record, "effects", json!(probe_id));
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
    if !drafts.is_empty() {
        record["last_observation_at"] = json!(now);
    }
    save(tx, id, revision, &record)?;
    for draft in drafts {
        provider_event(tx, stream, now, draft)?;
    }
    Ok(true)
}

fn load_output(tx: &Transaction, id: &str) -> rusqlite::Result<Value> {
    let record: Option<String> = tx
        .query_row("SELECT record FROM outputs WHERE execution=?1", [id], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(record
        .and_then(|r| serde_json::from_str(&r).ok())
        .unwrap_or_else(|| json!({"base": 0, "end": 0, "data": "", "lost_ranges": []})))
}

fn save_output(tx: &Transaction, id: &str, output: &Value) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO outputs VALUES (?1, ?2) ON CONFLICT (execution) DO UPDATE SET record=excluded.record",
        params![id, output.to_string()],
    )?;
    Ok(())
}

/// Append harness output to the execution's bounded spool. Bytes the spool cannot keep are
/// discarded oldest first, and every discard is a declared lost range with its byte count and a
/// semantic event (OBS-7). Adjacent spool discards are coalesced into one range.
fn append_output(
    tx: &Transaction,
    id: &str,
    bytes: &[u8],
    spool: usize,
    mutants: &Mutants,
    revision: i64,
    drafts: &mut Vec<Draft>,
) -> rusqlite::Result<()> {
    let mut output = load_output(tx, id)?;
    let mut data = hex::decode(output["data"].as_str().unwrap_or_default()).unwrap_or_default();
    data.extend_from_slice(bytes);
    let base = output["base"].as_u64().unwrap_or(0);
    output["end"] = json!(output["end"].as_u64().unwrap_or(0) + bytes.len() as u64);
    if data.len() > spool {
        let discarded = (data.len() - spool) as u64;
        data.drain(..discarded as usize);
        let (from, to) = (base, base + discarded);
        output["base"] = json!(to);
        if !mutants.on("output-dropped-silently") {
            let mut range = json!({"from": from, "to": to, "bytes": discarded, "reason": "spool_limit", "coverage": "incomplete"});
            if mutants.on("lost-range-without-bytes") {
                range.as_object_mut().map(|m| m.remove("bytes"));
            }
            let ranges = output["lost_ranges"].as_array_mut();
            match ranges.and_then(|list| {
                list.iter_mut()
                    .find(|r| r["reason"] == "spool_limit" && r["to"] == from)
            }) {
                Some(last) if last["reason"] == "spool_limit" && last["to"] == from => {
                    last["to"] = json!(to);
                    if let Some(total) = last["bytes"].as_u64() {
                        last["bytes"] = json!(total + discarded);
                    }
                }
                _ => push(&mut output, "lost_ranges", range.clone()),
            }
            drafts.push(("execution.output.lost", subject(id), revision, range));
        }
    }
    output["data"] = json!(hex::encode(&data));
    save_output(tx, id, &output)
}

/// `execution.output.read`: a bounded read of the output spool by byte offset, with every
/// declared lost range (EXECUTION section 14).
pub fn read_output(
    store: &mut Store,
    id: &str,
    offset: u64,
    max_bytes: usize,
    executor: &Value,
) -> rusqlite::Result<Option<Value>> {
    let tx = store.transaction()?;
    if load(&tx, id)?.is_none() {
        return Ok(None);
    }
    let output = load_output(&tx, id)?;
    tx.commit()?;
    let data = hex::decode(output["data"].as_str().unwrap_or_default()).unwrap_or_default();
    let base = output["base"].as_u64().unwrap_or(0);
    let end = output["end"].as_u64().unwrap_or(0);
    let start = offset.clamp(base, end);
    let from = (start - base) as usize;
    let slice = &data[from..(from + max_bytes).min(data.len())];
    let lost = output["lost_ranges"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    Ok(Some(json!({
        "execution": subject(id),
        "offset": start,
        "data_base64": crate::json::base64(slice),
        "next_offset": start + slice.len() as u64,
        "end_offset": end,
        "lost_ranges": lost,
        "coverage": if lost.is_empty() { "complete" } else { "incomplete" },
        "policy": {"spool_bytes": executor["output_spool_bytes"].as_u64().unwrap_or(65_536), "overflow": "discard_oldest"},
    })))
}

/// Deliver the oldest undispatched steering message. Returns false when none is waiting.
#[allow(clippy::too_many_arguments)]
fn deliver_steering(
    tx: &Transaction,
    id: &str,
    record: &mut Value,
    proof: &str,
    generation: &Value,
    now: &str,
    mutants: &Mutants,
    revision: i64,
    drafts: &mut Vec<Draft>,
) -> rusqlite::Result<bool> {
    let Some(index) = record["steering"].as_array().and_then(|list| {
        list.iter()
            .position(|s| s["request"] == "recorded" && s["dispatched"] != true)
    }) else {
        return Ok(false);
    };
    let delivery_id = record["steering"][index]["delivery_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let Some(mut effect) = load_effect(tx, &delivery_id)? else {
        return Ok(false);
    };
    mark_dispatch(&mut effect, generation, now);
    let sent = run_attempts(&mut effect, record, now, mutants);
    let entry = &mut record["steering"][index];
    entry["dispatched"] = json!(true);
    let evidence = if sent {
        json!({"class": proof, "source": "scripted harness"})
    } else {
        json!({"class": "transport_error", "source": "scripted harness transport"})
    };
    if !sent {
        observe_effect(
            &mut effect,
            "unknown",
            "transport_error",
            "scripted harness transport",
            now,
        );
        entry["delivery"] = json!("ambiguous");
    } else if proof == "provider_ack_id" {
        observe_effect(&mut effect, "succeeded", proof, "scripted harness", now);
        set_obligation(&mut effect, "satisfied");
        entry["delivery"] = json!("acknowledged");
        entry["proof_class"] = json!(proof);
        if mutants.on("steer-ack-implies-behavior") {
            entry["behavior"] = json!("observed");
            entry["behavior_evidence"] = evidence.clone();
        }
    } else {
        observe_effect(&mut effect, "pending", proof, "scripted harness", now);
        entry["proof_class"] = json!(proof);
    }
    entry["evidence"] = evidence.clone();
    let payload = json!({"steer_id": entry["steer_id"], "delivery_id": delivery_id, "delivery": entry["delivery"], "evidence": evidence});
    save_effect(tx, &delivery_id, id, &effect)?;
    drafts.push((
        "execution.steer.delivery.observed",
        subject(id),
        revision,
        payload,
    ));
    Ok(true)
}

/// Send recorded responses to answered native action requests.
fn dispatch_action_responses(
    tx: &Transaction,
    id: &str,
    record: &mut Value,
    generation: &Value,
    now: &str,
    mutants: &Mutants,
) -> rusqlite::Result<()> {
    let count = record["actions"].as_array().map_or(0, Vec::len);
    for index in 0..count {
        let action = record["actions"][index].clone();
        let Some(effect_id) = action["response_effect"].as_str() else {
            continue;
        };
        if action["dispatched"] == true {
            continue;
        }
        let Some(mut effect) = load_effect(tx, effect_id)? else {
            continue;
        };
        mark_dispatch(&mut effect, generation, now);
        if run_attempts(&mut effect, record, now, mutants) {
            observe_effect(
                &mut effect,
                "succeeded",
                "provider_ack_id",
                "scripted harness",
                now,
            );
            set_obligation(&mut effect, "satisfied");
        } else {
            observe_effect(
                &mut effect,
                "unknown",
                "transport_error",
                "scripted harness transport",
                now,
            );
        }
        save_effect(tx, effect_id, id, &effect)?;
        record["actions"][index]["dispatched"] = json!(true);
    }
    Ok(())
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
    let passed: Vec<String> = record["timeouts_passed"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(String::from)
        .collect();
    let due_at = |base: Option<&str>, name: &str| -> bool {
        match (base, record["timeouts"][name].as_i64()) {
            (Some(base), Some(seconds)) => {
                !passed.iter().any(|p| p == name) && now >= add_seconds(base, seconds).as_str()
            }
            _ => false,
        }
    };
    if record["admission"] == "queued" {
        if !due_at(record["submitted_at"].as_str(), "queue") {
            return Ok(());
        }
        let revision = revision + 1;
        push(&mut record, "timeouts_passed", json!("queue"));
        record["admission"] = json!("refused");
        record["delivery"] = json!("failed_before_delivery");
        record["reason"] = json!("queue_timeout");
        record["alternative"] = json!("submit again when the executor can admit it");
        if let Some(map) = record.as_object_mut() {
            map.remove("queue_reason");
        }
        if record["budget"]["reservation"] == "reserved" {
            record["budget"]["reservation"] = json!("released");
        }
        save(tx, id, revision, &record)?;
        provider_event(
            tx,
            stream,
            now,
            (
                "execution.timeout.passed",
                subject(id),
                revision,
                json!({"timeout": "queue"}),
            ),
        )?;
        return provider_event(
            tx,
            stream,
            now,
            (
                "execution.admission.changed",
                subject(id),
                revision,
                json!({"admission": "refused", "reason": "queue_timeout"}),
            ),
        );
    }
    if record["admission"] != "admitted" {
        return Ok(());
    }
    let started = started_at(&record);
    let mut due = Vec::new();
    if record["delivery"] == "pending" && due_at(Some(&started), "delivery") {
        due.push("delivery");
    }
    if record["runtime"] != "exited" && due_at(Some(&started), "execution_deadline") {
        due.push("execution_deadline");
    }
    if record["runtime"] != "exited" && due_at(record["last_observation_at"].as_str(), "inactivity")
    {
        due.push("inactivity");
    }
    if record["delivery"] == "ambiguous"
        && due_at(record["ambiguous_since"].as_str(), "reconciliation")
    {
        due.push("reconciliation");
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
        push(&mut record, "timeouts_passed", json!(name));
        drafts.push((
            "execution.timeout.passed",
            subject(id),
            revision,
            json!({"timeout": name}),
        ));
        if matches!(name, "delivery" | "reconciliation") && effect.is_object() {
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
        }
        // The wait for delivery evidence ended without a known outcome (owner decision 3).
        if name == "delivery"
            && effect.is_object()
            && record["delivery"] == "pending"
            && !mutants.on("pending-forever")
        {
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
        if name == "reconciliation" && mutants.on("reconciliation-timeout-resolves") {
            let evidence = json!({"class": "reconciliation_timeout", "source": "reconciliation timeout passed"});
            determine(
                &mut record,
                "not_delivered",
                evidence.clone(),
                None,
                now,
                true,
            );
            observe_effect(
                &mut effect,
                "failed",
                "reconciliation_timeout",
                "reconciliation timeout passed",
                now,
            );
        }
        if name == "inactivity" && mutants.on("inactivity-marks-exited") {
            record["runtime"] = json!("exited");
            record["exit"] = json!("forced_termination");
        }
        if name == "execution_deadline" {
            if mutants.on("deadline-marks-effect-failed") && effect.is_object() {
                observe_effect(
                    &mut effect,
                    "failed",
                    "timeout",
                    "execution deadline passed",
                    now,
                );
            }
            if mutants.on("refunds-on-timeout") && record["budget"].is_object() {
                record["budget"]["reservation"] = json!("released");
                record["usage"]["liability_override"] = json!("resolved");
            }
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

/// Remove executor-internal bookkeeping from listed entries.
fn public_entries(list: &Value) -> Value {
    let mut list = list.clone();
    for entry in list.as_array_mut().into_iter().flatten() {
        if let Some(map) = entry.as_object_mut() {
            map.remove("dispatched");
        }
    }
    list
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
    for key in [
        "predecessor",
        "correlation",
        "finalized_by",
        "cancellation",
        "reason",
        "alternative",
        "queue_reason",
        "runtime_detail",
        "continuation",
        "context",
    ] {
        if let Some(value) = record.get(key) {
            result[key] = value.clone();
        }
    }
    if let Some(cancellation) = result
        .get_mut("cancellation")
        .and_then(Value::as_object_mut)
    {
        cancellation.remove("effect");
    }
    for key in ["steering", "actions"] {
        if record[key].as_array().is_some_and(|list| !list.is_empty()) {
            result[key] = public_entries(&record[key]);
        }
    }
    if record["workspace"].is_object() {
        result["workspace"] = json!({
            "lease": record["workspace"]["lease"],
            "checkpoints": record["workspace"]["checkpoints"],
        });
    }
    if record["usage"].is_object() || record["budget"].is_object() {
        let mut usage = json!({
            "observations": record["usage"]["observations"].as_array().cloned().unwrap_or_default(),
            "liability": record["usage"]["liability_override"].as_str().unwrap_or(liability(&record)),
        });
        if record["budget"].is_object() {
            usage["budget"] = record["budget"].clone();
        }
        result["usage"] = usage;
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
        "revision": effect["revision"].as_i64().unwrap_or(1),
        "status": status,
        "observations": effect["observations"],
        "attempts": effect["attempts"].as_array().cloned().unwrap_or_default(),
        "obligations": effect["obligations"],
    })))
}

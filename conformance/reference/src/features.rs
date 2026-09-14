//! Optional `execution/1` feature commands and `core.effects.abort_obligation` (EXECUTION section
//! 11, CORE section 19.4). Script steps that report harness behavior for these features are in
//! `execution.rs`.

use rusqlite::Transaction;
use serde_json::{Value, json};

use crate::execution::{
    Applied, CONTROLLER_KIND, Context, Draft, KIND, load, load_effect, new_effect, observe_effect,
    push, save, save_effect, subject,
};

const STEERING_ALTERNATIVE: &str =
    "cancel this execution and submit a successor that names it as predecessor";

/// `execution.steer`: a recorded request with its own receipt. Native acknowledgment and observed
/// behavior are later, separate facts (EXE-9).
pub fn steer(
    tx: &Transaction,
    id: &str,
    payload: &Value,
    sequence: i64,
    ctx: &Context,
) -> rusqlite::Result<Applied> {
    let (revision, mut record) = load(tx, id)?.unwrap_or((0, json!({})));
    let number = record["steering"].as_array().map_or(0, Vec::len) + 1;
    let steer_id = format!("{id}.steer-{number}");
    let live = ctx.executor["adapter"]["steering"] == "live"
        || ctx.mutants.on("steer-unsupported-accepted");
    let mut entry = json!({"steer_id": steer_id, "recorded_at": ctx.now});
    let mut outcome = json!({"steer_id": steer_id});
    let mut effect_refs = Vec::new();
    if live {
        let delivery_id = format!("{steer_id}.delivery");
        let effect = new_effect(
            &delivery_id,
            "execution.steering_delivery",
            id,
            &payload["message"]["digest"],
            &record,
            "non_repeatable",
            &json!(format!("op-{sequence}")),
            Some(("evidence", "steering delivery evidence", Value::Null)),
            &ctx.now,
            "execution.steer owner transaction",
        );
        save_effect(tx, &delivery_id, id, &effect)?;
        push(&mut record, "effects", json!(delivery_id));
        entry["request"] = json!("recorded");
        entry["delivery_id"] = json!(delivery_id);
        entry["delivery"] = json!("pending");
        entry["behavior"] = json!("not_observed");
        if ctx.mutants.on("steer-claims-delivery") {
            entry["delivery"] = json!("acknowledged");
        }
        outcome["request"] = json!("recorded");
        outcome["delivery_id"] = json!(delivery_id);
        effect_refs.push(delivery_id);
    } else {
        entry["request"] = json!("not_supported");
        entry["alternative"] = json!(STEERING_ALTERNATIVE);
        outcome["request"] = json!("not_supported");
        outcome["alternative"] = json!(STEERING_ALTERNATIVE);
    }
    push(&mut record, "steering", entry);
    let revision = revision + 1;
    save(tx, id, revision, &record)?;
    let events: Vec<Draft> = vec![(
        "execution.steer.requested",
        subject(id),
        revision,
        json!({"steer_id": steer_id, "request": outcome["request"]}),
    )];
    Ok((revision, outcome, events, effect_refs))
}

/// The execution whose pending native action request has this ID, if the command may answer it.
/// Action IDs are scoped to their execution (EXE-14).
pub fn action_owner(
    tx: &Transaction,
    id: &str,
    action_id: &str,
    global_namespace: bool,
) -> rusqlite::Result<Option<String>> {
    let pending = |record: &Value| {
        record["actions"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|a| a["action_id"] == action_id && a["state"] == "pending")
    };
    if let Some((_, record)) = load(tx, id)?
        && pending(&record)
    {
        return Ok(Some(id.to_string()));
    }
    if global_namespace {
        for other in crate::execution::execution_ids(tx)? {
            if let Some((_, record)) = load(tx, &other)?
                && pending(&record)
            {
                return Ok(Some(other));
            }
        }
    }
    Ok(None)
}

/// `execution.respond_action`: records the answer and the response effect in the owner
/// transaction. The response reaches the harness later (EXE-4, EXE-14).
pub fn respond_action(
    tx: &Transaction,
    id: &str,
    payload: &Value,
    sequence: i64,
    ctx: &Context,
) -> rusqlite::Result<Applied> {
    let action_id = payload["action_id"].as_str().unwrap_or_default();
    let owner = action_owner(tx, id, action_id, ctx.mutants.on("global-action-namespace"))?
        .unwrap_or_else(|| id.to_string());
    let (revision, mut record) = load(tx, &owner)?.unwrap_or((0, json!({})));
    let effect_id = format!(
        "{owner}.response-{}",
        record["effects"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|e| e.contains(".response-"))
            .count()
            + 1
    );
    let effect = new_effect(
        &effect_id,
        "execution.action_response",
        &owner,
        &payload["response"]["digest"],
        &record,
        "non_repeatable",
        &json!(format!("op-{sequence}")),
        Some(("evidence", "action response delivery evidence", Value::Null)),
        &ctx.now,
        "execution.respond_action owner transaction",
    );
    save_effect(tx, &effect_id, &owner, &effect)?;
    push(&mut record, "effects", json!(effect_id));
    for action in record["actions"].as_array_mut().into_iter().flatten() {
        if action["action_id"] == action_id && action["state"] == "pending" {
            action["state"] = json!("answered");
            action["answered_at"] = json!(ctx.now);
            action["response_effect"] = json!(effect_id);
        }
    }
    let revision = revision + 1;
    save(tx, &owner, revision, &record)?;
    let outcome =
        json!({"action_id": action_id, "state": "answered", "response_effect": effect_id});
    let events: Vec<Draft> = vec![(
        "execution.action.answered",
        subject(&owner),
        revision,
        json!({"action_id": action_id, "response_effect": effect_id}),
    )];
    Ok((revision, outcome, events, vec![effect_id]))
}

/// `execution.controller.claim`: advances the controller authority epoch of one host (EXE-13).
pub fn claim_controller(
    tx: &Transaction,
    host: &str,
    principal: &str,
) -> rusqlite::Result<Applied> {
    let current: i64 = tx
        .query_row(
            "SELECT revision FROM subjects WHERE kind=?1 AND id=?2",
            rusqlite::params![CONTROLLER_KIND, host],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let epoch = current + 1;
    tx.execute(
        "INSERT INTO subjects VALUES (?1, ?2, ?3, ?4, 1)
         ON CONFLICT (kind, id) DO UPDATE SET revision=excluded.revision, value=excluded.value,
         applied_count=applied_count+1",
        rusqlite::params![
            CONTROLLER_KIND,
            host,
            epoch,
            json!({"controller": principal}).to_string()
        ],
    )?;
    let controller = json!({"kind": CONTROLLER_KIND, "id": host});
    let events: Vec<Draft> = vec![(
        "execution.controller.claimed",
        controller,
        epoch,
        json!({"epoch": epoch, "controller": principal}),
    )];
    Ok((epoch, json!({"epoch": epoch}), events, Vec::new()))
}

/// `execution.workspace.checkpoint`: records the coverage the executor probed itself. An
/// agent-reported commit is an annotation, never the checkpoint's head (EXE-15).
pub fn checkpoint(tx: &Transaction, id: &str, ctx: &Context) -> rusqlite::Result<Applied> {
    let (revision, mut record) = load(tx, id)?.unwrap_or((0, json!({})));
    let workspace = &record["workspace"];
    let probe = &workspace["probe"];
    let probed = |area: &str| {
        probe["probes"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|p| p == area)
    };
    let mut coverage = json!({});
    for area in ["tracked", "dirty", "untracked"] {
        coverage[area] = json!(if probed(area) { "probed" } else { "not_probed" });
    }
    let complete = ["tracked", "dirty", "untracked"]
        .iter()
        .all(|area| probed(area))
        || ctx.mutants.on("incomplete-checkpoint-complete");
    let number = workspace["checkpoints"].as_array().map_or(0, Vec::len) + 1;
    let checkpoint_id = format!("{id}.checkpoint-{number}");
    let annotations = workspace["annotations"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut outcome = json!({
        "checkpoint_id": checkpoint_id,
        "lease_epoch": workspace["lease"]["lease_epoch"],
        "recorded_at": ctx.now,
        "coverage": coverage,
        "complete": complete,
        "annotations": annotations,
    });
    if let Some(at) = probe.get("probed_at") {
        outcome["probed_at"] = at.clone();
    }
    if probed("tracked") {
        outcome["head"] = probe["head"].clone();
    }
    if ctx.mutants.on("agent-commit-as-receipt")
        && let Some(reported) = annotations.last()
    {
        outcome["head"] = reported["value"].clone();
        outcome["coverage"]["tracked"] = json!("probed");
    }
    if probed("dirty") {
        outcome["dirty_paths"] = probe["dirty_paths"].clone();
    }
    if probed("untracked") {
        outcome["untracked_paths"] = probe["untracked_paths"].clone();
    }
    push(&mut record["workspace"], "checkpoints", outcome.clone());
    let revision = revision + 1;
    save(tx, id, revision, &record)?;
    let events: Vec<Draft> = vec![(
        "execution.workspace.checkpointed",
        subject(id),
        revision,
        json!({"checkpoint_id": checkpoint_id, "complete": complete}),
    )];
    Ok((revision, outcome, events, Vec::new()))
}

/// Whether an effect has an obligation with this ID that is still waiting.
pub fn obligation_waiting(effect: &Value, obligation: &str) -> bool {
    effect["obligations"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|o| o["id"] == obligation && matches!(o["state"].as_str(), Some("open" | "overdue")))
}

/// `core.effects.abort_obligation`: closes a wait without changing the effect's status (EFF-4).
pub fn abort_obligation(
    tx: &Transaction,
    effect_id: &str,
    payload: &Value,
    ctx: &Context,
) -> rusqlite::Result<Applied> {
    let mut effect = load_effect(tx, effect_id)?.unwrap_or(Value::Null);
    let obligation = payload["obligation"].as_str().unwrap_or_default();
    for entry in effect["obligations"].as_array_mut().into_iter().flatten() {
        if entry["id"] == obligation {
            entry["state"] = json!("aborted");
        }
    }
    if ctx.mutants.on("abort-marks-effect-failed") {
        observe_effect(
            &mut effect,
            "failed",
            "obligation_aborted",
            "core.effects.abort_obligation",
            &ctx.now,
        );
    }
    let execution: String = tx
        .query_row(
            "SELECT execution FROM effects WHERE id=?1",
            [effect_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    let revision = save_effect(tx, effect_id, &execution, &effect)?;
    let status = effect["observations"]
        .as_array()
        .and_then(|o| o.last())
        .map_or(json!("pending"), |o| o["status"].clone());
    let outcome = json!({"effect": effect_id, "obligation": {"id": obligation, "state": "aborted"}, "status": status});
    let events: Vec<Draft> = vec![(
        "core.effect.obligation.aborted",
        json!({"kind": "core.effect", "id": effect_id}),
        revision,
        json!({"effect": effect_id, "obligation": obligation, "target": {"kind": KIND, "id": execution}}),
    )];
    Ok((revision, outcome, events, Vec::new()))
}

fn tri_state(value: &Value) -> &'static str {
    match value.as_str() {
        Some("yes") => "yes",
        Some("no") => "no",
        _ => "unknown",
    }
}

/// `execution.discovery.list`: scripted installations with separate facts. Only an installation
/// with every fact positive and verified is offered as usable (EXE-12).
pub fn discovery(executor: &Value, mutants: &crate::mutants::Mutants) -> Value {
    let mut installations = Vec::new();
    for scripted in executor["installations"].as_array().into_iter().flatten() {
        let detected = scripted["detected"].as_bool().unwrap_or(false);
        let authentication = match scripted["authentication"].as_str() {
            Some("authenticated") => "authenticated",
            Some("unauthenticated") => "unauthenticated",
            Some(_) | None if mutants.on("discovery-unknown-as-yes") => "authenticated",
            _ => "unknown",
        };
        let mut facts = json!({
            "installation_id": scripted["installation_id"],
            "harness": scripted["harness"],
            "detected": detected,
            "adapter_recognized": tri_state(&scripted["adapter_recognized"]),
            "version_supported": tri_state(&scripted["version_supported"]),
            "authentication": authentication,
            "reachable": tri_state(&scripted["reachable"]),
        });
        if mutants.on("discovery-unknown-as-yes") {
            for key in ["adapter_recognized", "version_supported", "reachable"] {
                if facts[key] == "unknown" {
                    facts[key] = json!("yes");
                }
            }
        }
        for key in ["version", "last_verified"] {
            if let Some(value) = scripted.get(key) {
                facts[key] = value.clone();
            }
        }
        let usable = if mutants.on("detected-offered-as-usable") {
            detected
        } else {
            detected
                && facts["adapter_recognized"] == "yes"
                && facts["version_supported"] == "yes"
                && facts["authentication"] == "authenticated"
                && facts["reachable"] == "yes"
                && facts.get("last_verified").is_some()
        };
        facts["usable"] = json!(usable);
        installations.push(facts);
    }
    json!({"installations": installations})
}

//! Reference provider for `knowledge/1` (KNOWLEDGE draft, owner decisions M5-Q1 to M5-Q4, M5-Q8).
//!
//! Claims, decisions, evaluations, conflicts and authority bindings are ordinary subjects, so Core
//! revisions, preconditions and events apply unchanged. Every comparison is structural: equality
//! of identifiers, qualifier values, trees and canonical JSON. Where structure cannot settle a
//! question the answer is uncertain, never guessed.

use rusqlite::Transaction;
use serde_json::{Value, json};

use crate::execution::{Applied, Draft};
use crate::json::{CanonicalFlaws, canonical, sha256_digest};
use crate::mutants::Mutants;

pub const CLAIM: &str = "knowledge.claim";
pub const DECISION: &str = "knowledge.decision";
pub const EVALUATION: &str = "knowledge.evaluation";
pub const CONFLICT: &str = "knowledge.conflict";
pub const AUTHORITY: &str = "knowledge.authority";
pub const CLAIM_FORMAT: &str = "combraton-knowledge-claim/1";
const CONDITION_KINDS: [&str; 3] = ["repository_tree", "dirty_snapshot", "environment_digest"];

/// What a command needs besides the store.
pub struct Context<'a> {
    pub now: String,
    pub principal: &'a str,
    pub provider_id: &'a str,
    pub config: &'a Value,
    pub order: i64,
    pub mutants: &'a Mutants,
}

/// A step 7 refusal: code and details.
pub type Refusal = (&'static str, Value);

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

fn all(tx: &Transaction, kind: &str) -> rusqlite::Result<Vec<(String, Value)>> {
    let mut statement = tx.prepare("SELECT id, value FROM subjects WHERE kind=?1")?;
    let rows = statement.query_map([kind], |r| {
        let id: String = r.get(0)?;
        let value: String = r.get(1)?;
        Ok((id, serde_json::from_str(&value).unwrap_or(Value::Null)))
    })?;
    let mut out: Vec<(String, Value)> = rows.collect::<rusqlite::Result<_>>()?;
    out.sort_by_key(|(_, value)| value["order"].as_i64().unwrap_or(0));
    Ok(out)
}

fn digest_of(value: &Value) -> String {
    sha256_digest(&canonical(value, CanonicalFlaws::default()))
}

fn canonical_eq(a: &Value, b: &Value) -> bool {
    canonical(a, CanonicalFlaws::default()) == canonical(b, CanonicalFlaws::default())
}

// ---------------------------------------------------------------------------------------------
// Step 2: payload semantics a schema cannot express.

/// Returns the offending path and reason.
pub fn validate(
    operation: &str,
    params: &Value,
    mutants: &Mutants,
) -> Result<(), (String, &'static str)> {
    let payload = &params["payload"];
    let bad = |path: &str, reason: &'static str| Err((path.to_string(), reason));
    match operation {
        "knowledge.claim.propose" | "knowledge.claim.revise" => {
            if payload.get("health").is_some() && payload["plane"] != "observed" {
                return bad(
                    "/payload/health",
                    "health is reported only by observed claims",
                );
            }
            if canonical(&payload["statement"]["value"], CanonicalFlaws::default()).len() > 4096 {
                return bad(
                    "/payload/statement/value",
                    "a statement value is at most 4096 bytes",
                );
            }
            let validity = &payload["validity"];
            if let (Some(from), Some(until)) =
                (validity["from"].as_str(), validity["until"].as_str())
                && from >= until
            {
                return bad("/payload/validity", "from must be before until");
            }
            let mut seen = Vec::new();
            for (index, entry) in payload["support"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
            {
                if seen.contains(&entry["support_id"]) {
                    return bad("/payload/support", "duplicate support_id");
                }
                seen.push(entry["support_id"].clone());
                let unknown = entry["ancestry"]["completeness"] == "unknown";
                let declared = entry["ancestry"]["roots"]
                    .as_array()
                    .is_some_and(|r| !r.is_empty());
                if unknown && declared {
                    return Err((
                        format!("/payload/support/{index}/ancestry/roots"),
                        "unknown ancestry declares no roots",
                    ));
                }
                if !unknown && !declared && !mutants.on("empty-roots-accepted") {
                    return Err((
                        format!("/payload/support/{index}/ancestry/roots"),
                        "complete or partial ancestry declares at least one root",
                    ));
                }
            }
            let mut seen = Vec::new();
            for condition in payload["conditions"].as_array().into_iter().flatten() {
                if seen.contains(&condition["condition_id"]) {
                    return bad("/payload/conditions", "duplicate condition_id");
                }
                seen.push(condition["condition_id"].clone());
            }
            Ok(())
        }
        "knowledge.conflict.open" => {
            let revisions = payload["revisions"].as_array().cloned().unwrap_or_default();
            if revisions.len() == 2 && revisions[0] == revisions[1] {
                return bad(
                    "/payload/revisions",
                    "a conflict needs two different revisions",
                );
            }
            Ok(())
        }
        "knowledge.decision.record" | "knowledge.conflict.resolve" => {
            if params.get("authority_epoch").is_none() && !mutants.on("decision-epoch-optional") {
                return bad(
                    "/authority_epoch",
                    "deciding needs the binding's authority epoch",
                );
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

// ---------------------------------------------------------------------------------------------
// Claims and references.

/// The revision entry `{ revision, record, digest, recorded_at }` of a claim, if it exists.
pub fn revision_entry(
    tx: &Transaction,
    claim: &str,
    revision: i64,
) -> rusqlite::Result<Option<Value>> {
    let Some((_, value)) = load(tx, CLAIM, claim)? else {
        return Ok(None);
    };
    Ok(value["revisions"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| entry["revision"] == revision)
        .cloned())
}

fn current_entry(tx: &Transaction, claim: &str) -> rusqlite::Result<Option<Value>> {
    let Some((_, value)) = load(tx, CLAIM, claim)? else {
        return Ok(None);
    };
    Ok(value["revisions"]
        .as_array()
        .and_then(|r| r.last())
        .cloned())
}

fn reference_of(entry: &Value) -> Value {
    let record = &entry["record"];
    json!({"provider": record["provider"], "claim": record["claim"], "revision": record["revision"], "digest": entry["digest"]})
}

fn same_revision(a: &Value, b: &Value) -> bool {
    a["claim"] == b["claim"] && a["revision"] == b["revision"]
}

/// Exact reference identity: provider, claim, revision and digest (KNOWLEDGE section 2).
fn same_reference(a: &Value, b: &Value) -> bool {
    same_revision(a, b) && a["provider"] == b["provider"] && a["digest"] == b["digest"]
}

/// Resolve a reference: `Ok(entry)`, or the refusal for a revision this provider does not hold
/// (another provider, or no such claim and revision) or a digest mismatch.
fn resolve(
    tx: &Transaction,
    reference: &Value,
    provider_id: &str,
    path: &str,
    mutants: &Mutants,
) -> rusqlite::Result<Result<Value, Refusal>> {
    if reference["provider"] != provider_id && !mutants.on("reference-provider-unchecked") {
        return Ok(Err(("not_found", json!({}))));
    }
    let claim = reference["claim"].as_str().unwrap_or_default();
    let revision = reference["revision"].as_i64().unwrap_or(0);
    let Some(entry) = revision_entry(tx, claim, revision)? else {
        return Ok(Err(("not_found", json!({}))));
    };
    if entry["digest"] != reference["digest"] {
        return Ok(Err((
            "invalid_envelope",
            json!({"path": path, "reason": "the digest differs from the revision's digest"}),
        )));
    }
    Ok(Ok(entry))
}

fn build_record(ctx: &Context, claim: &str, revision: i64, payload: &Value) -> Value {
    let or = |name: &str, default: Value| payload.get(name).cloned().unwrap_or(default);
    let mut validity = or("validity", Value::Null);
    if ctx.mutants.on("validity-filled-from-recorded") && validity.get("from").is_none() {
        if validity.is_null() {
            validity = json!({});
        }
        validity["from"] = json!(ctx.now);
    }
    json!({
        "format": CLAIM_FORMAT,
        "provider": ctx.provider_id,
        "claim": claim,
        "revision": revision,
        "producer": ctx.principal,
        "plane": payload["plane"],
        "statement": payload["statement"],
        "scope": payload["scope"],
        "validity": validity,
        "basis": or("basis", Value::Null),
        "support": payload["support"],
        "derivation": payload["derivation"],
        "dependencies": or("dependencies", json!([])),
        "conditions": or("conditions", json!([])),
        "health": or("health", json!("not_applicable")),
        "supersedes": or("supersedes", Value::Null),
    })
}

// ---------------------------------------------------------------------------------------------
// Authority (KNOWLEDGE section 6).

pub fn binding(tx: &Transaction, scope: &str) -> rusqlite::Result<Option<(i64, Value)>> {
    load(tx, AUTHORITY, scope)
}

/// Checks 2 to 4 of section 6 for a scope: binding, bound principal, epoch.
fn authority_checks(
    tx: &Transaction,
    scope: &str,
    principal: &str,
    epoch: Option<i64>,
    record: Option<&Value>,
    mutants: &Mutants,
) -> rusqlite::Result<Result<(), Refusal>> {
    let denied = || Err(("permission_denied", json!({"reason": "not_authority"})));
    let Some((_, bound)) = binding(tx, scope)? else {
        return Ok(denied());
    };
    let labelled_human = record
        .is_some_and(|r| r["derivation"]["kind"] == "human" && r["producer"] == principal)
        && mutants.on("derivation-label-authorizes");
    if bound["authority"] != principal && !mutants.on("decide-by-grant-right") && !labelled_human {
        return Ok(denied());
    }
    let current = bound["epoch"].as_i64().unwrap_or(1);
    match epoch {
        Some(e) if e < current => Ok(Err((
            "stale_authority_epoch",
            json!({"current_epoch": current}),
        ))),
        Some(e) if e > current => Ok(Err(("unknown_authority_epoch", json!({})))),
        _ => Ok(Ok(())),
    }
}

/// The latest decision about a revision, in recorded order.
fn latest_decision(
    tx: &Transaction,
    reference: &Value,
    mutants: &Mutants,
) -> rusqlite::Result<Option<(String, Value)>> {
    let mut latest = None;
    let current_epoch = if mutants.on("transfer-resets-reliance") {
        let scope = revision_entry(
            tx,
            reference["claim"].as_str().unwrap_or_default(),
            reference["revision"].as_i64().unwrap_or(0),
        )?
        .map(|e| {
            e["record"]["scope"]["id"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .unwrap_or_default();
        binding(tx, &scope)?.map(|(_, b)| b["epoch"].clone())
    } else {
        None
    };
    for (id, decision) in all(tx, DECISION)? {
        if !same_reference(&decision["claim"], reference) {
            continue;
        }
        if let Some(epoch) = &current_epoch
            && &decision["epoch"] != epoch
        {
            continue;
        }
        latest = Some((id, decision));
    }
    Ok(latest)
}

// ---------------------------------------------------------------------------------------------
// Conflicts (KNOWLEDGE section 7).

/// Structural comparison: `Ok((kind, status, uncertain))` or the refusal reason.
pub fn compare(
    a: &Value,
    b: &Value,
    mutants: &Mutants,
) -> Result<(&'static str, &'static str, Vec<&'static str>), &'static str> {
    let (sa, sb) = (&a["statement"], &b["statement"]);
    if sa["subject"] != sb["subject"] {
        return Err("subject_differs");
    }
    if sa["predicate"] != sb["predicate"] {
        return Err("predicate_differs");
    }
    if a["scope"]["id"] != b["scope"]["id"] {
        return Err("scope_differs");
    }
    let mut uncertain = Vec::new();
    let identity_only = mutants.on("conflict-on-identity-only");
    // 4. Qualifiers.
    let (qa, qb) = (&a["scope"]["qualifiers"], &b["scope"]["qualifiers"]);
    if let (Some(ma), Some(mb)) = (qa.as_object(), qb.as_object()) {
        if !identity_only && ma.iter().any(|(k, v)| mb.get(k).is_some_and(|w| w != v)) {
            return Err("qualifiers_disjoint");
        }
        if ma != mb {
            uncertain.push("qualifiers");
        }
    }
    // 5. Basis.
    let (ba, bb) = (&a["basis"], &b["basis"]);
    let mut basis_certain = ba.is_object() && bb.is_object();
    if basis_certain {
        let trees = |basis: &Value| -> Vec<(String, String)> {
            let mut out: Vec<(String, String)> = basis["repositories"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|r| {
                    (
                        r["id"].as_str().unwrap_or_default().to_string(),
                        r["tree"].as_str().unwrap_or_default().to_string(),
                    )
                })
                .collect();
            out.sort();
            out
        };
        let (ta, tb) = (trees(ba), trees(bb));
        if !identity_only {
            for (id, tree) in &ta {
                if tb.iter().any(|(other, t)| other == id && t != tree) {
                    return Err("basis_disjoint");
                }
            }
            for member in ["environment", "build"] {
                if let (Some(x), Some(y)) = (ba[member].as_str(), bb[member].as_str())
                    && x != y
                {
                    return Err("basis_disjoint");
                }
            }
        }
        basis_certain = ta == tb
            && ba["environment"] == bb["environment"]
            && ba["build"] == bb["build"]
            && ba["completeness"] == "complete"
            && bb["completeness"] == "complete";
    }
    if !basis_certain {
        uncertain.push("basis");
    }
    // 6. Validity.
    let known = |v: &Value| v["from"].is_string() && v["until"].is_string();
    let (va, vb) = (&a["validity"], &b["validity"]);
    if known(va) && known(vb) {
        let overlap = va["from"].as_str() < vb["until"].as_str()
            && vb["from"].as_str() < va["until"].as_str();
        if !overlap && !identity_only {
            return Err("validity_disjoint");
        }
    } else {
        uncertain.push("validity");
    }
    // 7. Cardinality.
    let (ca, cb) = (&sa["cardinality"], &sb["cardinality"]);
    if (ca == "multiple" || cb == "multiple") && !identity_only {
        return Err("multiple_values_permitted");
    }
    if !(ca == "single" && cb == "single") {
        uncertain.push("cardinality");
    }
    // 8. Values.
    if canonical_eq(&sa["value"], &sb["value"]) {
        return Err("values_equal");
    }
    let planes = (
        a["plane"].as_str().unwrap_or_default(),
        b["plane"].as_str().unwrap_or_default(),
    );
    let drift = matches!(
        planes,
        ("normative", "observed") | ("observed", "normative")
    );
    if !(drift || planes.0 == planes.1) {
        uncertain.push("plane");
    }
    let kind = if drift && !mutants.on("drift-as-conflict") {
        "drift"
    } else {
        "conflict"
    };
    let status = if uncertain.is_empty() || mutants.on("potential-reported-demonstrated") {
        "demonstrated"
    } else {
        "potential"
    };
    Ok((kind, status, uncertain))
}

// ---------------------------------------------------------------------------------------------
// Support and declared ancestry (KNOWLEDGE section 5).

fn roots_of(entry: &Value, mutants: &Mutants) -> (Vec<Value>, String) {
    let completeness = entry["ancestry"]["completeness"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let mut roots: Vec<Value> = entry["ancestry"]["roots"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if mutants.on("support-entries-as-origins") && roots.is_empty() {
        let evidence = &entry["evidence"];
        roots.push(json!({"kind": "evidence", "provider": evidence["provider"], "artifact": evidence["artifact"], "digest": evidence["digest"]}));
        return (roots, "complete".to_string());
    }
    (roots, completeness)
}

/// `(class, unknown_ancestry)` for a revision's support entries.
pub fn support_class(support: &Value, mutants: &Mutants) -> (&'static str, Vec<Value>) {
    let entries: Vec<&Value> = support.as_array().into_iter().flatten().collect();
    let unknown: Vec<Value> = entries
        .iter()
        .filter(|e| e["ancestry"]["completeness"] == "unknown")
        .map(|e| e["support_id"].clone())
        .collect();
    if entries.is_empty() {
        return ("unsupported", unknown);
    }
    let described: Vec<(Vec<Value>, String)> =
        entries.iter().map(|e| roots_of(e, mutants)).collect();
    let relation = |i: usize, j: usize| -> &'static str {
        let (ri, ci) = &described[i];
        let (rj, cj) = &described[j];
        if ri.iter().any(|x| rj.contains(x)) {
            return "shared";
        }
        let digest_equal = !mutants.on("equal-digest-roots-disjoint")
            && ri
                .iter()
                .any(|x| rj.iter().any(|y| x["digest"] == y["digest"]));
        if ci == "complete" && cj == "complete" && !digest_equal {
            "disjoint"
        } else {
            "undetermined"
        }
    };
    let mut any_disjoint = false;
    let mut all_shared = true;
    for i in 0..described.len() {
        for j in i + 1..described.len() {
            match relation(i, j) {
                "disjoint" => {
                    any_disjoint = true;
                    all_shared = false;
                }
                "undetermined" => all_shared = false,
                _ => {}
            }
        }
    }
    let any_unknown = described.iter().any(|(_, c)| c == "unknown");
    if any_unknown && !any_disjoint {
        return ("undetermined", unknown);
    }
    let common = described[0]
        .0
        .iter()
        .any(|root| described.iter().all(|(roots, _)| roots.contains(root)));
    if common {
        return ("single_lineage", unknown);
    }
    if any_disjoint {
        return ("multiple_lineages", unknown);
    }
    if all_shared {
        let class = if mutants.on("overlap-as-multiple") {
            "multiple_lineages"
        } else {
            "overlapping_lineages"
        };
        return (class, unknown);
    }
    ("undetermined", unknown)
}

fn availability(
    tx: &Transaction,
    support: &Value,
    ctx_provider: &str,
    evidence_config: &Value,
    mutants: &Mutants,
) -> rusqlite::Result<Value> {
    let (mut available, mut unavailable, mut purged, mut unknown) = (0, 0, 0, 0);
    for entry in support.as_array().into_iter().flatten() {
        let evidence = &entry["evidence"];
        if evidence["provider"] != ctx_provider {
            unknown += 1;
            continue;
        }
        let id = evidence["artifact"]["id"].as_str().unwrap_or_default();
        match load(tx, crate::evidence::ARTIFACT, id)? {
            Some((_, record)) if record["descriptor"]["digest"] == evidence["digest"] => {
                let observed =
                    crate::evidence::observed(tx, id, &record, evidence_config, mutants)?;
                match observed["state"].as_str() {
                    Some("available") => available += 1,
                    Some("purged") => purged += 1,
                    _ => unavailable += 1,
                }
            }
            _ => unavailable += 1,
        }
    }
    let total = available + unavailable + purged + unknown;
    let state = if total == 0 {
        "unknown"
    } else if available == total {
        "complete"
    } else if available > 0 {
        "partial"
    } else if unknown > 0 {
        "unknown"
    } else if total > 0 && purged == total {
        "purged"
    } else {
        "unavailable"
    };
    Ok(
        json!({"state": state, "counts": {"available": available, "unavailable": unavailable, "purged": purged, "unknown": unknown}}),
    )
}

// ---------------------------------------------------------------------------------------------
// Applicability (KNOWLEDGE section 8).

/// The latest evaluation of exactly `reference` (provider, claim, revision and digest) for a target
/// equal to `target`.
fn latest_evaluation(
    tx: &Transaction,
    reference: &Value,
    target: &Value,
    mutants: &Mutants,
) -> rusqlite::Result<Option<(String, Value)>> {
    let mut latest = None;
    for (id, evaluation) in all(tx, EVALUATION)? {
        let recorded = &evaluation["claim"];
        let identity = same_revision(recorded, reference)
            && (recorded["digest"] == reference["digest"]
                || mutants.on("dependency-digest-ignored"))
            && (recorded["provider"] == reference["provider"]
                || mutants.on("dependency-provider-ignored"));
        if identity && canonical_eq(&evaluation["target"], target) {
            latest = Some((id, evaluation));
        }
    }
    Ok(latest)
}

/// `(result, findings)` for a revision record against a target.
pub fn evaluate(
    tx: &Transaction,
    record: &Value,
    target: &Value,
    config: &Value,
    provider_id: &str,
    mutants: &Mutants,
) -> rusqlite::Result<(&'static str, Vec<Value>)> {
    let supported: Vec<String> = match config["evaluator"]["condition_kinds"].as_array() {
        Some(kinds) => kinds
            .iter()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect(),
        None => CONDITION_KINDS.iter().map(|k| k.to_string()).collect(),
    };
    let mut findings = Vec::new();
    let repository = |id: &Value| {
        target["repositories"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|r| &r["id"] == id)
            .cloned()
    };
    for condition in record["conditions"].as_array().into_iter().flatten() {
        let kind = condition["kind"].as_str().unwrap_or_default();
        let finding = if !supported.iter().any(|k| k == kind) {
            "unsupported"
        } else {
            let observed = match kind {
                "repository_tree" => {
                    repository(&condition["repository"]).map(|r| r["tree"].clone())
                }
                "dirty_snapshot" if mutants.on("dirty-snapshot-as-tree") => {
                    repository(&condition["repository"]).map(|_| condition["expected"].clone())
                }
                "dirty_snapshot" => repository(&condition["repository"])
                    .and_then(|r| r["dirty"]["snapshot_digest"].as_str().map(|d| json!(d))),
                _ => target["environment"].as_str().map(|e| json!(e)),
            };
            match observed {
                None if mutants.on("incomplete-coverage-applicable") => "match",
                None => "missing_anchor",
                Some(value) if value == condition["expected"] => "match",
                Some(_) => "mismatch",
            }
        };
        findings.push(json!({"condition_id": condition["condition_id"], "finding": finding}));
    }
    // Dependencies resolve only as exact local references; nothing else can satisfy them.
    let own = json!({"claim": record["claim"], "revision": record["revision"]});
    for dependency in record["dependencies"].as_array().into_iter().flatten() {
        let (finding, reason) =
            dependency_finding(tx, dependency, &own, target, provider_id, mutants)?;
        let mut entry = json!({"dependency": dependency, "finding": finding});
        if let Some(reason) = reason {
            entry["reason"] = json!(reason);
        }
        findings.push(entry);
    }
    let has = |names: &[&str]| {
        findings
            .iter()
            .any(|f| names.iter().any(|n| f["finding"] == *n))
    };
    let mismatch = has(&["mismatch"]);
    let unknown = has(&["missing_anchor", "unsupported"]);
    let result = if mutants.on("unknown-outranks-mismatch") && unknown {
        "unknown"
    } else if mismatch {
        "invalid_for_target"
    } else if unknown {
        "unknown"
    } else if has(&["unchecked"]) {
        "needs_check"
    } else if findings.is_empty() {
        "unknown"
    } else {
        "applicable"
    };
    Ok((result, findings))
}

/// One dependency's finding and, for `unchecked`, why (KNOWLEDGE section 8).
fn dependency_finding(
    tx: &Transaction,
    dependency: &Value,
    own: &Value,
    target: &Value,
    provider_id: &str,
    mutants: &Mutants,
) -> rusqlite::Result<(&'static str, Option<&'static str>)> {
    if same_revision(dependency, own) {
        return Ok(("unchecked", Some("self_reference")));
    }
    if dependency["provider"] != provider_id && !mutants.on("dependency-provider-ignored") {
        return Ok(("unchecked", Some("remote_dependency")));
    }
    let held = revision_entry(
        tx,
        dependency["claim"].as_str().unwrap_or_default(),
        dependency["revision"].as_i64().unwrap_or(0),
    )?
    .is_some_and(|entry| {
        entry["digest"] == dependency["digest"] || mutants.on("dependency-digest-ignored")
    });
    if !held {
        return Ok(if mutants.on("missing-dependency-satisfied") {
            ("match", None)
        } else {
            ("unchecked", Some("unresolved"))
        });
    }
    Ok(match latest_evaluation(tx, dependency, target, mutants)? {
        Some((_, evaluation)) => match evaluation["result"].as_str() {
            Some("applicable") => ("match", None),
            Some("invalid_for_target") => ("mismatch", None),
            _ => ("unchecked", Some("not_established")),
        },
        None => ("unchecked", Some("not_evaluated")),
    })
}

// ---------------------------------------------------------------------------------------------
// Step 7.

pub fn check(
    tx: &Transaction,
    operation: &str,
    params: &Value,
    principal: &str,
    provider_id: &str,
    mutants: &Mutants,
) -> rusqlite::Result<Result<(), Refusal>> {
    let payload = &params["payload"];
    let id = params["subject"]["id"].as_str().unwrap_or_default();
    let epoch = params["authority_epoch"].as_i64();
    let invalid = |path: &str, reason: &str| {
        Err(("invalid_envelope", json!({"path": path, "reason": reason})))
    };
    match operation {
        "knowledge.claim.revise" => {
            if mutants.on("revise-base-unchecked") {
                return Ok(Ok(()));
            }
            let Some(current) = current_entry(tx, id)? else {
                return Ok(Err(("not_found", json!({}))));
            };
            if payload["supersedes"]["revision"] != current["revision"] {
                return Ok(invalid(
                    "/payload/supersedes/revision",
                    "not the lineage's current revision",
                ));
            }
            if payload["supersedes"]["digest"] != current["digest"] {
                return Ok(invalid(
                    "/payload/supersedes/digest",
                    "not the current revision's digest",
                ));
            }
            Ok(Ok(()))
        }
        "knowledge.decision.record" => {
            let reference = &payload["claim"];
            let local =
                reference["provider"] == provider_id || mutants.on("reference-provider-unchecked");
            let Some(entry) = revision_entry(
                tx,
                reference["claim"].as_str().unwrap_or_default(),
                reference["revision"].as_i64().unwrap_or(0),
            )?
            .filter(|_| local) else {
                return Ok(Err(("not_found", json!({}))));
            };
            let record = &entry["record"];
            let scope = record["scope"]["id"].as_str().unwrap_or_default();
            let receipts_cited = payload["validation_basis"]["receipts"]
                .as_array()
                .is_some_and(|r| !r.is_empty());
            if !(receipts_cited && mutants.on("receipt-authorizes-decision"))
                && let Err(refusal) =
                    authority_checks(tx, scope, principal, epoch, Some(record), mutants)?
            {
                return Ok(Err(refusal));
            }
            if entry["digest"] != reference["digest"] {
                return Ok(invalid(
                    "/payload/claim/digest",
                    "the digest differs from the revision's digest",
                ));
            }
            let latest = latest_decision(tx, reference, &Mutants::default())?.map(|(id, _)| id);
            let named = payload["supersedes_decision"].as_str().map(String::from);
            if latest != named && !mutants.on("decision-supersession-unchecked") {
                return Ok(Err((
                    "precondition_failed",
                    json!({"latest_decision": latest}),
                )));
            }
            Ok(Ok(()))
        }
        "knowledge.conflict.open" => {
            let mut records = Vec::new();
            for (index, reference) in payload["revisions"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
            {
                match resolve(
                    tx,
                    reference,
                    provider_id,
                    &format!("/payload/revisions/{index}/digest"),
                    mutants,
                )? {
                    Ok(entry) => records.push(entry["record"].clone()),
                    Err(refusal) => return Ok(Err(refusal)),
                }
            }
            match compare(&records[0], &records[1], mutants) {
                Ok(_) => Ok(Ok(())),
                Err(reason) => Ok(Err(("claims_not_comparable", json!({"reason": reason})))),
            }
        }
        "knowledge.conflict.resolve" => {
            let Some((_, conflict)) = load(tx, CONFLICT, id)? else {
                return Ok(Err(("not_found", json!({}))));
            };
            if conflict["state"] != "open" {
                return Ok(Err(("not_found", json!({}))));
            }
            let first = &conflict["revisions"][0];
            let record = revision_entry(
                tx,
                first["claim"].as_str().unwrap_or_default(),
                first["revision"].as_i64().unwrap_or(0),
            )?
            .map(|e| e["record"].clone())
            .unwrap_or(Value::Null);
            let scope = record["scope"]["id"].as_str().unwrap_or_default();
            if !mutants.on("resolve-without-authority")
                && let Err(refusal) = authority_checks(tx, scope, principal, epoch, None, mutants)?
            {
                return Ok(Err(refusal));
            }
            let select = payload["resolution"] == "select";
            let named = payload.get("selected");
            let valid = match named {
                Some(selected) => {
                    select
                        && conflict["revisions"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .any(|r| r == selected)
                }
                None => !select,
            };
            if !valid {
                return Ok(invalid(
                    "/payload/selected",
                    "select names one of the two revisions, and only select names one",
                ));
            }
            Ok(Ok(()))
        }
        "knowledge.applicability.evaluate" => {
            match resolve(
                tx,
                &payload["claim"],
                provider_id,
                "/payload/claim/digest",
                mutants,
            )? {
                Ok(_) => Ok(Ok(())),
                Err(refusal) => Ok(Err(refusal)),
            }
        }
        _ => Ok(Ok(())),
    }
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
        "knowledge.claim.propose" | "knowledge.claim.revise" => {
            let (revision, mut value) =
                load(tx, CLAIM, id)?.unwrap_or((0, json!({"revisions": []})));
            let revision = revision + 1;
            let record = build_record(ctx, id, revision, payload);
            let digest = digest_of(&record);
            let entry = json!({"revision": revision, "record": record, "digest": digest, "recorded_at": ctx.now});
            let reference = reference_of(&entry);
            if mutants.on("claim-revision-overwritten") {
                value["revisions"] = json!([entry]);
            } else if let Some(list) = value["revisions"].as_array_mut() {
                list.push(entry);
            }
            save(tx, CLAIM, id, revision, &value)?;
            let events: Vec<Draft> = vec![(
                "knowledge.claim.revised",
                subject(CLAIM, id),
                revision,
                json!({"reference": reference}),
            )];
            Ok((
                revision,
                json!({"reference": reference}),
                events,
                Vec::new(),
            ))
        }
        "knowledge.authority.bind" | "knowledge.authority.transfer" => {
            let (revision, previous) = load(tx, AUTHORITY, id)?.unwrap_or((0, Value::Null));
            let epoch = previous["epoch"].as_i64().unwrap_or(0) + 1;
            let revision = revision + 1;
            let value = json!({"scope": id, "authority": payload["authority"], "epoch": epoch});
            save(tx, AUTHORITY, id, revision, &value)?;
            let event = if operation.ends_with("bind") {
                "knowledge.authority.bound"
            } else {
                "knowledge.authority.transferred"
            };
            let events: Vec<Draft> = vec![(
                event,
                subject(AUTHORITY, id),
                revision,
                json!({"authority": payload["authority"], "epoch": epoch}),
            )];
            Ok((revision, value, events, Vec::new()))
        }
        "knowledge.decision.record" => {
            let reference = &payload["claim"];
            let entry = revision_entry(
                tx,
                reference["claim"].as_str().unwrap_or_default(),
                reference["revision"].as_i64().unwrap_or(0),
            )?
            .unwrap_or(Value::Null);
            let record = &entry["record"];
            let scope = record["scope"]["id"].as_str().unwrap_or_default();
            let epoch = binding(tx, scope)?
                .map(|(_, b)| b["epoch"].clone())
                .unwrap_or(json!(1));
            let author_is_decider =
                record["producer"] == ctx.principal && !mutants.on("self-adoption-unrecorded");
            let value = json!({
                "claim": reference,
                "value": payload["decision"],
                "permitted_use": payload.get("permitted_use").cloned().unwrap_or(Value::Null),
                "supersedes_decision": payload.get("supersedes_decision").cloned().unwrap_or(Value::Null),
                "validation_basis": payload["validation_basis"],
                "rationale": payload["rationale"],
                "decider": ctx.principal,
                "author_is_decider": author_is_decider,
                "epoch": epoch,
                "scope": scope,
                "recorded_at": ctx.now,
                "order": ctx.order,
            });
            save(tx, DECISION, id, 1, &value)?;
            let mut event = json!({"claim": reference, "decision": payload["decision"], "author_is_decider": author_is_decider, "supersedes_decision": value["supersedes_decision"]});
            let mut outcome = json!({"decision": subject(DECISION, id), "claim": reference, "value": payload["decision"], "author_is_decider": author_is_decider, "epoch": epoch});
            if let Some(used) = payload.get("permitted_use") {
                event["permitted_use"] = used.clone();
                outcome["permitted_use"] = used.clone();
            }
            let events: Vec<Draft> = vec![(
                "knowledge.decision.recorded",
                subject(DECISION, id),
                1,
                event,
            )];
            Ok((1, outcome, events, Vec::new()))
        }
        "knowledge.conflict.open" => {
            let mut records = Vec::new();
            for reference in payload["revisions"].as_array().into_iter().flatten() {
                let entry = revision_entry(
                    tx,
                    reference["claim"].as_str().unwrap_or_default(),
                    reference["revision"].as_i64().unwrap_or(0),
                )?
                .unwrap_or(Value::Null);
                records.push(entry["record"].clone());
            }
            let (kind, status, uncertain) = compare(&records[0], &records[1], mutants).unwrap_or((
                "conflict",
                "potential",
                Vec::new(),
            ));
            let value = json!({
                "kind": kind, "status": status, "uncertain": uncertain,
                "revisions": payload["revisions"],
                "values": [records[0]["statement"]["value"], records[1]["statement"]["value"]],
                "note": payload.get("note").cloned().unwrap_or(Value::Null),
                "state": "open", "resolution": Value::Null, "selected": Value::Null, "rationale": Value::Null,
                "recorded_at": ctx.now, "order": ctx.order,
            });
            save(tx, CONFLICT, id, 1, &value)?;
            let events: Vec<Draft> = vec![(
                "knowledge.conflict.opened",
                subject(CONFLICT, id),
                1,
                json!({"kind": kind, "status": status, "revisions": payload["revisions"]}),
            )];
            Ok((
                1,
                json!({"conflict": subject(CONFLICT, id), "kind": kind, "status": status, "uncertain": uncertain}),
                events,
                Vec::new(),
            ))
        }
        "knowledge.conflict.resolve" => {
            let (revision, mut value) = load(tx, CONFLICT, id)?.unwrap_or((0, json!({})));
            let revision = revision + 1;
            value["state"] = json!("resolved");
            value["resolution"] = payload["resolution"].clone();
            value["selected"] = payload.get("selected").cloned().unwrap_or(Value::Null);
            value["rationale"] = payload["rationale"].clone();
            value["resolved_by"] = json!(ctx.principal);
            value["resolved_at"] = json!(ctx.now);
            save(tx, CONFLICT, id, revision, &value)?;
            let events: Vec<Draft> = vec![(
                "knowledge.conflict.resolved",
                subject(CONFLICT, id),
                revision,
                json!({"resolution": payload["resolution"]}),
            )];
            Ok((
                revision,
                json!({"conflict": subject(CONFLICT, id), "state": "resolved", "resolution": payload["resolution"]}),
                events,
                Vec::new(),
            ))
        }
        "knowledge.applicability.evaluate" => {
            let reference = &payload["claim"];
            let entry = revision_entry(
                tx,
                reference["claim"].as_str().unwrap_or_default(),
                reference["revision"].as_i64().unwrap_or(0),
            )?
            .unwrap_or(Value::Null);
            let target = &payload["target"];
            let (result, findings) = evaluate(
                tx,
                &entry["record"],
                target,
                ctx.config,
                ctx.provider_id,
                mutants,
            )?;
            let evaluator = json!({
                "id": ctx.config["evaluator"]["id"].as_str().unwrap_or("reference-conditions"),
                "version": ctx.config["evaluator"]["version"].as_str().unwrap_or("1"),
            });
            let supersedes = latest_evaluation(tx, reference, target, &Mutants::default())?
                .map(|(id, _)| json!(id))
                .unwrap_or(Value::Null);
            let value = json!({
                "claim": reference, "target": target, "evaluator": evaluator, "result": result,
                "findings": findings, "supersedes_evaluation": supersedes,
                "recorded_at": ctx.now, "order": ctx.order,
            });
            save(tx, EVALUATION, id, 1, &value)?;
            let events: Vec<Draft> = vec![(
                "knowledge.evaluation.recorded",
                subject(EVALUATION, id),
                1,
                json!({"claim": reference, "result": result, "evaluator": evaluator, "supersedes_evaluation": supersedes}),
            )];
            let outcome = json!({"evaluation": subject(EVALUATION, id), "claim": reference, "result": result, "evaluator": evaluator, "findings": findings, "supersedes_evaluation": supersedes});
            Ok((1, outcome, events, Vec::new()))
        }
        _ => Ok((0, json!({}), Vec::new(), Vec::new())),
    }
}

// ---------------------------------------------------------------------------------------------
// Queries.

/// What a read needs besides the store.
pub struct Reader<'a> {
    pub provider_id: &'a str,
    pub config: &'a Value,
    pub evidence_config: &'a Value,
    pub mutants: &'a Mutants,
}

/// `knowledge.claim.inspect` (KNOWLEDGE section 4).
pub fn inspect(
    tx: &Transaction,
    payload: &Value,
    reader: &Reader,
) -> rusqlite::Result<Option<Value>> {
    let mutants = reader.mutants;
    let claim = payload["claim"].as_str().unwrap_or_default();
    let Some(current) = current_entry(tx, claim)? else {
        return Ok(None);
    };
    let entry = match payload["revision"].as_i64() {
        Some(revision) => match revision_entry(tx, claim, revision)? {
            Some(entry) => entry,
            None => return Ok(None),
        },
        None => current.clone(),
    };
    let reference = reference_of(&entry);
    let mut record = entry["record"].clone();
    let decision = latest_decision(tx, &reference, mutants)?;
    let mut reliance = match &decision {
        Some((id, d)) => {
            let mut r = json!({"state": d["value"], "decision": id, "author_is_decider": d["author_is_decider"]});
            if d["permitted_use"].is_string() {
                r["permitted_use"] = d["permitted_use"].clone();
            }
            r
        }
        None => json!({"state": "proposed"}),
    };
    if decision.is_none()
        && mutants.on("normative-implies-binding")
        && record["plane"] == "normative"
    {
        reliance = json!({"state": "accepted_for_use", "permitted_use": "binding"});
    }
    let mut applicability: Vec<Value> = Vec::new();
    for (id, evaluation) in all(tx, EVALUATION)? {
        if !same_reference(&evaluation["claim"], &reference) {
            continue;
        }
        let item = json!({"evaluation": id, "target": evaluation["target"], "result": evaluation["result"], "evaluator": evaluation["evaluator"]});
        match applicability
            .iter_mut()
            .find(|a| canonical_eq(&a["target"], &evaluation["target"]))
        {
            Some(slot) => *slot = item,
            None => applicability.push(item),
        }
    }
    if mutants.on("single-status-field") && reliance["state"] == "accepted_for_use" {
        for item in &mut applicability {
            item["result"] = json!("applicable");
        }
    }
    let (class, unknown_ancestry) = support_class(&record["support"], mutants);
    let mut conflicts = Vec::new();
    for (id, conflict) in all(tx, CONFLICT)? {
        if conflict["revisions"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|r| same_reference(r, &reference))
        {
            conflicts.push(json!({"conflict": id, "kind": conflict["kind"], "status": conflict["status"], "state": conflict["state"]}));
        }
    }
    let available = availability(
        tx,
        &record["support"],
        reader.provider_id,
        reader.evidence_config,
        mutants,
    )?;
    if reader.config["serve_altered_claims"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|c| c == claim)
    {
        record["statement"]["value"] = json!({"altered": record["statement"]["value"]});
    }
    Ok(Some(json!({
        "reference": reference,
        "record": record,
        "current_revision": current["revision"],
        "reliance": reliance,
        "applicability": applicability,
        "health": entry["record"]["health"],
        "availability": available,
        "support": {"class": class, "unknown_ancestry": unknown_ancestry},
        "conflicts": conflicts,
    })))
}

fn position(
    tx: &Transaction,
    event_type: &str,
    event_subject: &Value,
    revision: i64,
) -> rusqlite::Result<Value> {
    let mut statement =
        tx.prepare("SELECT epoch, sequence, record FROM events ORDER BY epoch, sequence")?;
    let rows = statement.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (epoch, sequence, text) = row?;
        let record: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if record["type"] == event_type
            && &record["subject"] == event_subject
            && record["revision"] == revision
        {
            return Ok(json!({"epoch": epoch, "sequence": sequence}));
        }
    }
    Ok(Value::Null)
}

/// `knowledge.claim.history` (KNOWLEDGE section 9).
pub fn history(
    tx: &Transaction,
    claim: &str,
    mutants: &Mutants,
) -> rusqlite::Result<Option<Value>> {
    let Some((_, value)) = load(tx, CLAIM, claim)? else {
        return Ok(None);
    };
    let mut entries: Vec<Value> = value["revisions"].as_array().cloned().unwrap_or_default();
    if mutants.on("history-drops-superseded") && entries.len() > 1 {
        entries = entries.split_off(entries.len() - 1);
    }
    let mut revisions = Vec::new();
    for entry in &entries {
        let revision = entry["revision"].as_i64().unwrap_or(0);
        revisions.push(json!({
            "reference": reference_of(entry),
            "producer": entry["record"]["producer"],
            "supersedes": entry["record"]["supersedes"],
            "recorded_at": entry["recorded_at"],
            "position": position(tx, "knowledge.claim.revised", &subject(CLAIM, claim), revision)?,
        }));
    }
    let mut decisions = Vec::new();
    for (id, d) in all(tx, DECISION)? {
        if d["claim"]["claim"] != claim {
            continue;
        }
        decisions.push(json!({
            "decision": id, "claim": d["claim"], "value": d["value"], "permitted_use": d["permitted_use"],
            "decider": d["decider"], "author_is_decider": d["author_is_decider"], "epoch": d["epoch"],
            "supersedes_decision": d["supersedes_decision"], "recorded_at": d["recorded_at"],
            "position": position(tx, "knowledge.decision.recorded", &subject(DECISION, &id), 1)?,
        }));
    }
    let mut evaluations = Vec::new();
    for (id, e) in all(tx, EVALUATION)? {
        if e["claim"]["claim"] != claim {
            continue;
        }
        evaluations.push(json!({
            "evaluation": id, "claim": e["claim"], "target": e["target"], "result": e["result"],
            "evaluator": e["evaluator"], "supersedes_evaluation": e["supersedes_evaluation"],
            "recorded_at": e["recorded_at"],
            "position": position(tx, "knowledge.evaluation.recorded", &subject(EVALUATION, &id), 1)?,
        }));
    }
    let mut conflicts = Vec::new();
    for (id, c) in all(tx, CONFLICT)? {
        if !c["revisions"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|r| r["claim"] == claim)
        {
            continue;
        }
        conflicts.push(json!({
            "conflict": id, "kind": c["kind"], "status": c["status"], "revisions": c["revisions"],
            "state": c["state"], "resolution": c["resolution"], "recorded_at": c["recorded_at"],
            "position": position(tx, "knowledge.conflict.opened", &subject(CONFLICT, &id), 1)?,
        }));
    }
    Ok(Some(
        json!({"claim": claim, "revisions": revisions, "decisions": decisions, "evaluations": evaluations, "conflicts": conflicts}),
    ))
}

/// `knowledge.authority.get`.
pub fn authority(tx: &Transaction, scope: &str) -> rusqlite::Result<Option<Value>> {
    Ok(binding(tx, scope)?.map(|(revision, b)| json!({"scope": scope, "authority": b["authority"], "epoch": b["epoch"], "revision": revision})))
}

/// The authority scope a deciding command acts in, for error details.
pub fn scope_of(tx: &Transaction, operation: &str, params: &Value) -> rusqlite::Result<String> {
    let reference = if operation == "knowledge.conflict.resolve" {
        let id = params["subject"]["id"].as_str().unwrap_or_default();
        load(tx, CONFLICT, id)?
            .map(|(_, c)| c["revisions"][0].clone())
            .unwrap_or(Value::Null)
    } else {
        params["payload"]["claim"].clone()
    };
    Ok(revision_entry(
        tx,
        reference["claim"].as_str().unwrap_or_default(),
        reference["revision"].as_i64().unwrap_or(0),
    )?
    .and_then(|e| e["record"]["scope"]["id"].as_str().map(String::from))
    .unwrap_or_default())
}

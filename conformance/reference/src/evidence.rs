//! Reference provider for `evidence/1` (EVIDENCE draft, owner decisions M4-Q1, Q5, Q6, Q7).
//!
//! Artifacts and holds are ordinary subjects (kinds `evidence.artifact` and `evidence.hold`) so
//! Core revisions and preconditions apply unchanged; bytes live in their own table. The scripted
//! store (`evidence_store` in launch configuration) injects corruption, staging timeouts and
//! deferred physical deletion; it is test environment, never protocol.

use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use crate::execution::{Applied, Draft, provider_event};
use crate::mutants::Mutants;
use crate::store::Store;

pub const ARTIFACT: &str = "evidence.artifact";
pub const HOLD: &str = "evidence.hold";
pub const MANIFEST_MEDIA_TYPE: &str = "application/vnd.combraton.evidence-manifest+json";
pub const MANIFEST_FORMAT: &str = "combraton-evidence-manifest/1";
/// Envelope bytes an append needs besides its base64 data, declared to producers.
pub const CHUNK_OVERHEAD_BYTES: i64 = 2048;
/// Dependency kinds this provider tracks for proof-loss reports.
pub const TRACKED_DEPENDENCIES: [&str; 2] = ["evidence.hold", "evidence.manifest_child"];

/// What a command needs besides the store.
pub struct Context<'a> {
    pub now: String,
    pub principal: &'a str,
    pub chunk_limit: i64,
    pub store_config: &'a Value,
    pub mutants: &'a Mutants,
}

pub fn subject(kind: &str, id: &str) -> Value {
    json!({"kind": kind, "id": id})
}

pub fn load(tx: &Transaction, kind: &str, id: &str) -> rusqlite::Result<Option<(i64, Value)>> {
    let row: Option<(i64, String)> = tx
        .query_row(
            "SELECT revision, value FROM subjects WHERE kind=?1 AND id=?2",
            params![kind, id],
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

fn bytes(tx: &Transaction, id: &str) -> rusqlite::Result<Vec<u8>> {
    Ok(tx
        .query_row("SELECT data FROM evidence_bytes WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .optional()?
        .unwrap_or_default())
}

fn set_bytes(tx: &Transaction, id: &str, data: &[u8]) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO evidence_bytes VALUES (?1, ?2) ON CONFLICT (id) DO UPDATE SET data=excluded.data",
        params![id, data],
    )?;
    Ok(())
}

fn ids(tx: &Transaction, kind: &str) -> rusqlite::Result<Vec<String>> {
    let mut statement = tx.prepare("SELECT id FROM subjects WHERE kind=?1 ORDER BY id")?;
    let rows = statement.query_map([kind], |r| r.get(0))?;
    rows.collect()
}

/// The largest decoded chunk that fits the string, payload and frame limits once base64-encoded
/// with the declared envelope overhead (EVIDENCE section 4).
pub fn chunk_limit(limits: &crate::provider::Limits, mutants: &Mutants) -> i64 {
    if mutants.on("chunk-limit-ignores-overhead") {
        return limits.max_string_bytes;
    }
    let by_string = limits.max_string_bytes * 3 / 4;
    let by_payload = (limits.max_payload_bytes - CHUNK_OVERHEAD_BYTES).max(0) * 3 / 4;
    let by_frame = (limits.max_frame_bytes - CHUNK_OVERHEAD_BYTES).max(0) * 3 / 4;
    by_string.min(by_payload).min(by_frame).max(1)
}

pub fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some(u32::from(c - b'A')),
            b'a'..=b'z' => Some(u32::from(c - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(c - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let input = text.as_bytes();
    if !input.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    for chunk in input.chunks(4) {
        let pad = chunk.iter().rev().take_while(|c| **c == b'=').count();
        let mut n = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            let v = if *c == b'=' && i >= 4 - pad {
                0
            } else {
                value(*c)?
            };
            n = (n << 6) | v;
        }
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}

/// A locator that carries credentials: user information, or credential-like query parameters.
pub fn locator_has_credentials(locator: &str) -> bool {
    let after_scheme = locator.split_once("://").map_or(locator, |(_, rest)| rest);
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    if authority.contains('@') {
        return true;
    }
    let Some((_, query)) = locator.split_once('?') else {
        return false;
    };
    query.split('&').any(|pair| {
        let name = pair
            .split('=')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        [
            "token",
            "sig",
            "signature",
            "key",
            "credential",
            "password",
            "secret",
            "x-amz-",
            "access_token",
        ]
        .iter()
        .any(|marker| name.contains(marker))
    })
}

/// Profile checks at CORE section 10 step 7, after authorization and preconditions.
pub fn check(
    tx: &Transaction,
    operation: &str,
    params: &Value,
    mutants: &Mutants,
    chunk_limit: i64,
) -> rusqlite::Result<Result<(), (&'static str, Value)>> {
    let id = params["subject"]["id"].as_str().unwrap_or_default();
    let payload = &params["payload"];
    let artifact = load(tx, ARTIFACT, id)?;
    let found = |state: &str| artifact.as_ref().is_some_and(|(_, r)| r["state"] == state);
    Ok(match operation {
        "evidence.upload.append"
        | "evidence.seal"
        | "evidence.upload.abandon"
        | "evidence.purge"
            if artifact.is_none() =>
        {
            Err(("not_found", json!({})))
        }
        "evidence.upload.append" => {
            if !found("staged") {
                return Ok(Err(("not_found", json!({}))));
            }
            let record = &artifact.as_ref().unwrap().1;
            let received = record["received"].as_i64().unwrap_or(0);
            let size = record["descriptor"]["size"].as_i64().unwrap_or(0);
            let data = decode_base64(payload["data_base64"].as_str().unwrap_or_default())
                .unwrap_or_default();
            let offset = payload["offset"].as_i64().unwrap_or(-1);
            if data.len() as i64 > chunk_limit {
                Err((
                    "limit_exceeded",
                    json!({"limit": "chunk_limit", "maximum": chunk_limit}),
                ))
            } else if mutants.on("duplicate-chunk-accepted")
                && offset < received
                && bytes(tx, id)?
                    .get(offset as usize..(offset as usize + data.len()).min(received as usize))
                    == Some(&data[..])
            {
                Ok(())
            } else if offset != received && !mutants.on("append-overwrites-received") {
                Err(("upload_offset_mismatch", json!({"received": received})))
            } else if offset + data.len() as i64 > size {
                Err((
                    "upload_size_exceeded",
                    json!({"received": received, "size": size}),
                ))
            } else {
                Ok(())
            }
        }
        "evidence.seal" => {
            let record = &artifact.as_ref().unwrap().1;
            match record["state"].as_str() {
                Some("sealed") => Ok(()),
                Some("staged") => {
                    let received = record["received"].as_i64().unwrap_or(0);
                    let size = record["descriptor"]["size"].as_i64().unwrap_or(0);
                    if mutants.on("seal-unverified") {
                        Ok(())
                    } else if received < size {
                        Err((
                            "upload_incomplete",
                            json!({"received": received, "size": size}),
                        ))
                    } else {
                        let computed = crate::json::sha256_digest(&bytes(tx, id)?);
                        if record["descriptor"]["digest"] != computed.as_str() {
                            Err(("content_digest_mismatch", json!({"computed": computed})))
                        } else if record["descriptor"]["media_type"] == MANIFEST_MEDIA_TYPE
                            && manifest_children(&bytes(tx, id)?).is_none()
                        {
                            Err((
                                "invalid_envelope",
                                json!({"path": "/payload", "reason": "content is not a supported evidence manifest"}),
                            ))
                        } else {
                            Ok(())
                        }
                    }
                }
                _ => Err(("not_found", json!({}))),
            }
        }
        "evidence.upload.abandon" => {
            if found("staged") {
                Ok(())
            } else {
                Err(("not_found", json!({})))
            }
        }
        "evidence.hold" => {
            let target = payload["artifact"]["id"].as_str().unwrap_or_default();
            match load(tx, ARTIFACT, target)? {
                Some((_, record)) if record["state"] == "sealed" => Ok(()),
                _ => Err(("not_found", json!({}))),
            }
        }
        "evidence.release" => match load(tx, HOLD, id)? {
            Some((_, hold)) if hold["state"] == "active" => Ok(()),
            _ => Err(("not_found", json!({}))),
        },
        "evidence.purge" => {
            let record = &artifact.as_ref().unwrap().1;
            if record["state"] != "sealed" {
                return Ok(Err(("not_found", json!({}))));
            }
            if record.get("loss").is_some() {
                // Already requested: the purge is idempotent (EVIDENCE section 9).
                return Ok(Ok(()));
            }
            let named: Vec<String> = payload["release_holds"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect();
            for hold_id in &named {
                match load(tx, HOLD, hold_id)? {
                    Some((_, hold))
                        if hold["artifact"]["id"] == id && hold["state"] == "active" => {}
                    _ => {
                        return Ok(Err((
                            "invalid_envelope",
                            json!({"path": "/payload/release_holds", "reason": "not an active hold on this artifact"}),
                        )));
                    }
                }
            }
            let remaining = active_holds(tx, id)?
                .into_iter()
                .filter(|(hold_id, _)| !named.contains(hold_id))
                .collect::<Vec<_>>();
            if remaining.is_empty() || mutants.on("purge-bypasses-holds") {
                Ok(())
            } else {
                Err((
                    "hold_active",
                    json!({"holds": remaining.iter().map(|(h, _)| h.clone()).collect::<Vec<_>>()}),
                ))
            }
        }
        _ => Ok(()),
    })
}

pub fn active_holds(tx: &Transaction, artifact: &str) -> rusqlite::Result<Vec<(String, Value)>> {
    let mut out = Vec::new();
    for hold_id in ids(tx, HOLD)? {
        if let Some((_, hold)) = load(tx, HOLD, &hold_id)?
            && hold["artifact"]["id"] == artifact
            && hold["state"] == "active"
        {
            out.push((hold_id, hold));
        }
    }
    Ok(out)
}

fn manifest_children(content: &[u8]) -> Option<Vec<Value>> {
    let value: Value = serde_json::from_slice(content).ok()?;
    if value["format"] != MANIFEST_FORMAT {
        return None;
    }
    let children = value["children"].as_array()?;
    for child in children {
        let reference = &child["evidence"];
        if !(child["role"].is_string()
            && child["required"].is_boolean()
            && reference["artifact"]["kind"] == ARTIFACT
            && reference["artifact"]["id"].is_string()
            && reference["digest"].is_string())
        {
            return None;
        }
    }
    Some(children.clone())
}

/// Apply one evidence command inside the Core owner transaction.
pub fn apply(
    tx: &Transaction,
    operation: &str,
    id: &str,
    payload: &Value,
    ctx: &Context,
) -> rusqlite::Result<Applied> {
    let mutants = ctx.mutants;
    let artifact_subject = subject(ARTIFACT, id);
    match operation {
        "evidence.upload.prepare" => {
            let mut descriptor = payload.clone();
            let declared = payload["producer"]["principal"].as_str();
            let principal = match declared {
                Some(p) if mutants.on("producer-principal-from-payload") => p.to_string(),
                _ => ctx.principal.to_string(),
            };
            descriptor["producer"]["principal"] = json!(principal);
            let record = json!({
                "descriptor": descriptor,
                "state": "staged",
                "received": 0,
                "availability": {"state": "available"},
                "prepared_at": ctx.now,
            });
            set_bytes(tx, id, &[])?;
            save(tx, ARTIFACT, id, 1, &record)?;
            let outcome = json!({"artifact": artifact_subject, "state": "staged", "received": 0,
                "chunk_limit": ctx.chunk_limit, "chunk_overhead_bytes": CHUNK_OVERHEAD_BYTES});
            let events: Vec<Draft> = vec![(
                "evidence.artifact.staged",
                artifact_subject,
                1,
                json!({"descriptor": descriptor}),
            )];
            Ok((1, outcome, events, Vec::new()))
        }
        "evidence.upload.append" => {
            let (revision, mut record) = load(tx, ARTIFACT, id)?.unwrap_or((0, json!({})));
            let data = decode_base64(payload["data_base64"].as_str().unwrap_or_default())
                .unwrap_or_default();
            let offset = payload["offset"].as_i64().unwrap_or(0) as usize;
            let mut stored = bytes(tx, id)?;
            if mutants.on("duplicate-chunk-accepted")
                && stored.get(offset..offset + data.len()) == Some(&data[..])
            {
                let outcome = json!({"artifact": artifact_subject, "received": stored.len()});
                return Ok((revision, outcome, Vec::new(), Vec::new()));
            }
            stored.truncate(offset.min(stored.len()));
            stored.extend_from_slice(&data);
            set_bytes(tx, id, &stored)?;
            record["received"] = json!(stored.len());
            let revision = revision + 1;
            save(tx, ARTIFACT, id, revision, &record)?;
            let outcome = json!({"artifact": artifact_subject, "received": stored.len()});
            let events: Vec<Draft> = vec![(
                "evidence.artifact.appended",
                artifact_subject,
                revision,
                json!({"received": stored.len()}),
            )];
            Ok((revision, outcome, events, Vec::new()))
        }
        "evidence.seal" => {
            let (revision, mut record) = load(tx, ARTIFACT, id)?.unwrap_or((0, json!({})));
            let digest = record["descriptor"]["digest"].clone();
            let size = record["descriptor"]["size"].clone();
            if record["state"] == "sealed" && !mutants.on("seal-twice-appends-event") {
                let outcome = json!({"artifact": artifact_subject, "state": "sealed", "digest": digest, "size": size, "already_sealed": true});
                return Ok((revision, outcome, Vec::new(), Vec::new()));
            }
            let already = record["state"] == "sealed";
            if record["descriptor"]["media_type"] == MANIFEST_MEDIA_TYPE
                && let Some(children) = manifest_children(&bytes(tx, id)?)
            {
                record["children"] = json!(children);
            }
            record["state"] = json!("sealed");
            record["sealed_at"] = json!(ctx.now);
            let revision = revision + 1;
            save(tx, ARTIFACT, id, revision, &record)?;
            let outcome = json!({"artifact": artifact_subject, "state": "sealed", "digest": digest, "size": size, "already_sealed": already});
            let events: Vec<Draft> = vec![(
                "evidence.artifact.sealed",
                artifact_subject,
                revision,
                json!({"digest": digest, "size": size}),
            )];
            Ok((revision, outcome, events, Vec::new()))
        }
        "evidence.upload.abandon" => {
            let (revision, mut record) = load(tx, ARTIFACT, id)?.unwrap_or((0, json!({})));
            record["state"] = json!("abandoned");
            record["abandoned_reason"] = json!("abandoned_by_producer");
            set_bytes(tx, id, &[])?;
            let revision = revision + 1;
            save(tx, ARTIFACT, id, revision, &record)?;
            let events: Vec<Draft> = vec![(
                "evidence.artifact.abandoned",
                artifact_subject.clone(),
                revision,
                json!({"reason": "abandoned_by_producer"}),
            )];
            Ok((
                revision,
                json!({"artifact": artifact_subject, "state": "abandoned"}),
                events,
                Vec::new(),
            ))
        }
        "evidence.hold" => {
            let hold = json!({
                "artifact": payload["artifact"],
                "holder_ref": payload["holder_ref"],
                "reason": payload["reason"],
                "owner": ctx.principal,
                "state": "active",
                "placed_at": ctx.now,
            });
            let mut hold = hold;
            if let Some(expiry) = payload.get("expires_at") {
                hold["expires_at"] = expiry.clone();
            }
            save(tx, HOLD, id, 1, &hold)?;
            let hold_subject = subject(HOLD, id);
            let events: Vec<Draft> = vec![(
                "evidence.hold.placed",
                hold_subject.clone(),
                1,
                json!({"artifact": payload["artifact"], "holder_ref": payload["holder_ref"]}),
            )];
            Ok((
                1,
                json!({"hold": hold_subject, "state": "active", "owner": ctx.principal}),
                events,
                Vec::new(),
            ))
        }
        "evidence.release" => {
            let (revision, hold) = load(tx, HOLD, id)?.unwrap_or((0, json!({})));
            let (revision, events) = release_hold(tx, id, revision, hold, ctx)?;
            Ok((
                revision,
                json!({"hold": subject(HOLD, id), "state": "released"}),
                events,
                Vec::new(),
            ))
        }
        "evidence.purge" => {
            let (revision, mut record) = load(tx, ARTIFACT, id)?.unwrap_or((0, json!({})));
            if record.get("loss").is_some() {
                let outcome = json!({"artifact": artifact_subject, "availability": record["availability"]["state"], "released_holds": []});
                return Ok((revision, outcome, Vec::new(), Vec::new()));
            }
            let mut events: Vec<Draft> = Vec::new();
            let mut released = Vec::new();
            for hold_id in payload["release_holds"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                if let Some((hold_revision, hold)) = load(tx, HOLD, hold_id)? {
                    let (_, mut drafts) = release_hold(tx, hold_id, hold_revision, hold, ctx)?;
                    events.append(&mut drafts);
                    released.push(json!(subject(HOLD, hold_id)));
                }
            }
            let affected = dependencies(tx, id)?;
            let deferred = ctx.store_config["deletion_delay_seconds"]
                .as_i64()
                .is_some_and(|s| s > 0);
            let immediate = !deferred || mutants.on("purged-before-confirmation");
            let mut loss = json!({
                "artifact": artifact_subject,
                "digest": record["descriptor"]["digest"],
                "requested_at": ctx.now,
                "released_holds": released,
                "affected": affected,
                "coverage": {"tracked": TRACKED_DEPENDENCIES},
            });
            let revision = revision + 1;
            events.push((
                "evidence.artifact.purge_requested",
                artifact_subject.clone(),
                revision,
                json!({"requested_at": ctx.now}),
            ));
            if immediate {
                set_bytes(tx, id, &[])?;
                loss["confirmed_at"] = json!(ctx.now);
                record["availability"] = json!({"state": "purged"});
                events.push((
                    "evidence.artifact.purged",
                    artifact_subject.clone(),
                    revision,
                    json!({"confirmed_at": ctx.now}),
                ));
            } else {
                record["availability"] = json!({"state": "purge_pending"});
            }
            record["loss"] = loss;
            save(tx, ARTIFACT, id, revision, &record)?;
            let availability = record["availability"]["state"].clone();
            Ok((
                revision,
                json!({"artifact": artifact_subject, "availability": availability, "released_holds": released}),
                events,
                Vec::new(),
            ))
        }
        _ => Ok((0, json!({}), Vec::new(), Vec::new())),
    }
}

fn release_hold(
    tx: &Transaction,
    id: &str,
    revision: i64,
    mut hold: Value,
    ctx: &Context,
) -> rusqlite::Result<(i64, Vec<Draft>)> {
    hold["state"] = json!("released");
    hold["released_at"] = json!(ctx.now);
    hold["released_by"] = json!(ctx.principal);
    let revision = revision + 1;
    save(tx, HOLD, id, revision, &hold)?;
    Ok((
        revision,
        vec![(
            "evidence.hold.released",
            subject(HOLD, id),
            revision,
            json!({"artifact": hold["artifact"], "holder_ref": hold["holder_ref"]}),
        )],
    ))
}

/// Known dependencies of an artifact: holds on it and manifests listing it as a child.
fn dependencies(tx: &Transaction, artifact: &str) -> rusqlite::Result<Vec<Value>> {
    let mut out = Vec::new();
    for hold_id in ids(tx, HOLD)? {
        if let Some((_, hold)) = load(tx, HOLD, &hold_id)?
            && hold["artifact"]["id"] == artifact
        {
            out.push(json!({"kind": "evidence.hold", "subject": subject(HOLD, &hold_id)}));
        }
    }
    for other in ids(tx, ARTIFACT)? {
        if let Some((_, record)) = load(tx, ARTIFACT, &other)?
            && record["children"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|c| c["evidence"]["artifact"]["id"] == artifact)
        {
            out.push(
                json!({"kind": "evidence.manifest_child", "subject": subject(ARTIFACT, &other)}),
            );
        }
    }
    Ok(out)
}

/// Scripted store maintenance: staging timeouts and confirmation of deferred deletion.
pub fn tick(store: &mut Store, now: &str, config: &Value) -> rusqlite::Result<()> {
    let stream = store.stream_id()?;
    let tx = store.transaction()?;
    for id in ids(&tx, ARTIFACT)? {
        let Some((revision, mut record)) = load(&tx, ARTIFACT, &id)? else {
            continue;
        };
        if record["state"] == "staged"
            && let Some(timeout) = config["staging_timeout_seconds"].as_i64()
            && now
                >= crate::execution::add_seconds(
                    record["prepared_at"].as_str().unwrap_or(now),
                    timeout,
                )
                .as_str()
        {
            record["state"] = json!("abandoned");
            record["abandoned_reason"] = json!("staging_expired");
            set_bytes(&tx, &id, &[])?;
            save(&tx, ARTIFACT, &id, revision + 1, &record)?;
            provider_event(
                &tx,
                &stream,
                now,
                (
                    "evidence.artifact.abandoned",
                    subject(ARTIFACT, &id),
                    revision + 1,
                    json!({"reason": "staging_expired"}),
                ),
            )?;
            continue;
        }
        if record["availability"]["state"] == "purge_pending"
            && let Some(delay) = config["deletion_delay_seconds"].as_i64()
            && now
                >= crate::execution::add_seconds(
                    record["loss"]["requested_at"].as_str().unwrap_or(now),
                    delay,
                )
                .as_str()
        {
            set_bytes(&tx, &id, &[])?;
            record["availability"] = json!({"state": "purged"});
            record["loss"]["confirmed_at"] = json!(now);
            save(&tx, ARTIFACT, &id, revision + 1, &record)?;
            provider_event(
                &tx,
                &stream,
                now,
                (
                    "evidence.artifact.purged",
                    subject(ARTIFACT, &id),
                    revision + 1,
                    json!({"confirmed_at": now}),
                ),
            )?;
        }
    }
    tx.commit()
}

/// Availability as observed now: the recorded state, the scripted store's `unavailable` list, and
/// integrity of the stored bytes. Queries record nothing, so a read-time failure is reported only.
fn observed(
    tx: &Transaction,
    id: &str,
    record: &Value,
    config: &Value,
    mutants: &Mutants,
) -> rusqlite::Result<Value> {
    let recorded = record["availability"].clone();
    if record["state"] != "sealed" || recorded["state"] != "available" {
        return Ok(recorded);
    }
    if config["unavailable"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|u| u == id)
    {
        return Ok(json!({"state": "unavailable", "reason": "store_unavailable"}));
    }
    if crate::json::sha256_digest(&stored(tx, id, config)?)
        != record["descriptor"]["digest"].as_str().unwrap_or_default()
        && !mutants.on("corrupted-bytes-served")
    {
        return Ok(json!({"state": "unavailable", "reason": "integrity_failed"}));
    }
    Ok(recorded)
}

/// Stored bytes as the scripted store returns them, with injected corruption.
fn stored(tx: &Transaction, id: &str, config: &Value) -> rusqlite::Result<Vec<u8>> {
    let mut data = bytes(tx, id)?;
    if config["corrupt"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|c| c == id)
        && !data.is_empty()
    {
        data[0] ^= 0xff;
    }
    Ok(data)
}

/// Present a proof-loss record to one reader: only dependencies it may inspect (EVIDENCE 9).
fn present_loss(loss: &Value, can_read: &dyn Fn(&Value) -> bool, mutants: &Mutants) -> Value {
    let mut out = loss.clone();
    let mut filtered = false;
    let visible: Vec<Value> = loss["affected"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| {
            let ok = mutants.on("loss-report-unfiltered") || can_read(&d["subject"]);
            filtered |= !ok;
            ok
        })
        .cloned()
        .collect();
    let holds: Vec<Value> = loss["released_holds"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|h| {
            let ok = mutants.on("loss-report-unfiltered") || can_read(h);
            filtered |= !ok;
            ok
        })
        .cloned()
        .collect();
    out["affected"] = json!(visible);
    out["released_holds"] = json!(holds);
    out["coverage"]["filtered"] = json!(filtered);
    out
}

/// Per-reader manifest completeness (EVIDENCE section 7).
fn completeness(
    tx: &Transaction,
    record: &Value,
    provider_id: &str,
    config: &Value,
    can_read: &dyn Fn(&Value) -> bool,
    mutants: &Mutants,
) -> rusqlite::Result<Value> {
    let mut children = Vec::new();
    let mut any_missing = false;
    let mut all_present = true;
    for child in record["children"].as_array().into_iter().flatten() {
        let reference = &child["evidence"];
        let required = child["required"].as_bool().unwrap_or(false);
        let child_subject = &reference["artifact"];
        let other_provider = reference["provider"]
            .as_str()
            .is_some_and(|p| p != provider_id);
        let state = if other_provider {
            "unverified"
        } else if !can_read(child_subject) && !mutants.on("withheld-child-reported-missing") {
            "withheld"
        } else {
            let mut child_id = child_subject["id"].as_str().unwrap_or_default().to_string();
            let mut target = load(tx, ARTIFACT, &child_id)?;
            if target.is_none() && mutants.on("manifest-children-by-digest") {
                for other in ids(tx, ARTIFACT)? {
                    if let Some((r, rec)) = load(tx, ARTIFACT, &other)?
                        && rec["descriptor"]["digest"] == reference["digest"]
                    {
                        target = Some((r, rec));
                        child_id = other;
                        break;
                    }
                }
            }
            match target {
                Some((_, rec))
                    if rec["state"] == "sealed"
                        && rec["descriptor"]["digest"] == reference["digest"] =>
                {
                    match observed(tx, &child_id, &rec, config, mutants)?["state"].as_str() {
                        Some("available") => "present",
                        Some("purged" | "purge_pending") => "missing",
                        _ => "unverified",
                    }
                }
                _ => "missing",
            }
        };
        if required {
            any_missing |= state == "missing";
            all_present &= state == "present";
        }
        children.push(json!({"role": child["role"], "evidence": reference, "required": required, "state": state}));
    }
    let overall = if all_present || mutants.on("manifest-incomplete-reported-complete") {
        "complete"
    } else if any_missing {
        "incomplete"
    } else {
        "undetermined"
    };
    Ok(json!({"state": overall, "children": children}))
}

/// `evidence.inspect`.
pub fn inspect(
    store: &Store,
    id: &str,
    provider_id: &str,
    config: &Value,
    manifests: bool,
    can_read: &dyn Fn(&Value) -> bool,
    mutants: &Mutants,
) -> rusqlite::Result<Option<Value>> {
    let tx = store.reader()?;
    let Some((revision, record)) = load(&tx, ARTIFACT, id)? else {
        return Ok(None);
    };
    let mut result = json!({
        "artifact": subject(ARTIFACT, id),
        "revision": revision,
        "descriptor": record["descriptor"],
        "state": record["state"],
        "availability": observed(&tx, id, &record, config, mutants)?,
    });
    if record["state"] == "staged" {
        result["received"] = record["received"].clone();
    }
    if let Some(reason) = record.get("abandoned_reason") {
        result["abandoned_reason"] = reason.clone();
    }
    let holds: Vec<Value> = ids(&tx, HOLD)?
        .into_iter()
        .filter_map(|hold_id| load(&tx, HOLD, &hold_id).ok().flatten().map(|(rev, h)| (hold_id, rev, h)))
        .filter(|(_, _, h)| h["artifact"]["id"] == id)
        .filter(|(hold_id, _, _)| can_read(&subject(HOLD, hold_id)))
        .map(|(hold_id, rev, h)| json!({"hold": subject(HOLD, &hold_id), "revision": rev, "state": h["state"], "owner": h["owner"], "holder_ref": h["holder_ref"]}))
        .collect();
    result["holds"] = json!(holds);
    if let Some(loss) = record.get("loss") {
        result["loss"] = present_loss(loss, can_read, mutants);
    }
    if manifests && record.get("children").is_some() {
        result["completeness"] =
            completeness(&tx, &record, provider_id, config, can_read, mutants)?;
    }
    Ok(Some(result))
}

/// `evidence.fetch`: exact sealed bytes within bounds, never corrupted or mismatched bytes.
pub fn fetch(
    store: &Store,
    id: &str,
    payload: &Value,
    config: &Value,
    frame_budget: usize,
    mutants: &Mutants,
) -> rusqlite::Result<Result<Value, &'static str>> {
    let tx = store.reader()?;
    let Some((_, record)) = load(&tx, ARTIFACT, id)? else {
        return Ok(Err("not_found"));
    };
    if record["state"] != "sealed" {
        return Ok(Err("not_found"));
    }
    let digest = record["descriptor"]["digest"].clone();
    if payload["digest"] != digest && !mutants.on("mismatched-reference-serves-bytes") {
        return Ok(Err("artifact_digest_mismatch"));
    }
    let size = record["descriptor"]["size"].as_i64().unwrap_or(0);
    let availability = observed(&tx, id, &record, config, mutants)?;
    let mut data = stored(&tx, id, config)?;
    if mutants.on("locator-as-identity")
        && let Some(locator) = record["descriptor"]["locator"].as_str()
    {
        for other in ids(&tx, ARTIFACT)? {
            if let Some((_, rec)) = load(&tx, ARTIFACT, &other)?
                && rec["state"] == "sealed"
                && rec["descriptor"]["locator"] == locator
            {
                data = stored(&tx, &other, config)?;
                break;
            }
        }
    }
    let offset = payload["offset"].as_i64().unwrap_or(0).clamp(0, size) as usize;
    let mut take = payload["max_bytes"].as_u64().unwrap_or(65_536) as usize;
    if availability["state"] != "available" {
        take = 0;
    }
    take = take.min(data.len().saturating_sub(offset));
    // Keep the encoded response within the caller's receive limit, returning at least one byte.
    while take > 1 && (take.div_ceil(3) * 4) + 1024 > frame_budget {
        take /= 2;
    }
    let slice = &data[offset.min(data.len())..offset.min(data.len()) + take];
    Ok(Ok(json!({
        "artifact": subject(ARTIFACT, id),
        "digest": digest,
        "availability": availability,
        "offset": offset,
        "data_base64": crate::json::base64(slice),
        "next_offset": offset + take,
        "size": size,
    })))
}

/// `evidence.query`: readable artifacts matching the filters, bounded. `filtered` reports that the
/// reader's view is restricted, never whether an unreadable artifact matched (EVIDENCE section 5).
pub fn query(
    store: &Store,
    payload: &Value,
    config: &Value,
    restricted: bool,
    can_read: &dyn Fn(&Value) -> bool,
    mutants: &Mutants,
) -> rusqlite::Result<Value> {
    let tx = store.reader()?;
    let limit = payload["limit"].as_u64().unwrap_or(100) as usize;
    let after = payload["cursor"]
        .as_str()
        .and_then(|c| c.strip_prefix("evq1:"))
        .unwrap_or("");
    let filters = &payload["filters"];
    let mut items = Vec::new();
    let mut last = String::new();
    for id in ids(&tx, ARTIFACT)? {
        if id.as_str() <= after {
            continue;
        }
        let Some((_, record)) = load(&tx, ARTIFACT, &id)? else {
            continue;
        };
        let d = &record["descriptor"];
        let matches = [
            ("producer_principal", &d["producer"]["principal"]),
            ("media_type", &d["media_type"]),
            ("digest", &d["digest"]),
            ("scope", &d["scope"]),
        ]
        .iter()
        .all(|(key, value)| filters.get(*key).is_none_or(|f| f == *value))
            && filters.get("source").is_none_or(|f| *f == d["source"])
            && filters.get("work").is_none_or(|f| *f == d["work"]);
        if !matches {
            continue;
        }
        if !can_read(&subject(ARTIFACT, &id)) && !mutants.on("query-returns-unreadable") {
            continue;
        }
        if items.len() == limit {
            break;
        }
        last = id.clone();
        items.push(json!({"artifact": subject(ARTIFACT, &id), "digest": d["digest"], "state": record["state"], "availability": observed(&tx, &id, &record, config, mutants)?}));
    }
    let mut result = json!({"items": items, "filtered": restricted});
    if !last.is_empty() {
        result["next_cursor"] = json!(format!("evq1:{last}"));
    }
    Ok(result)
}

/// A hold record, read without opening a transaction so authorization checks can run inside one.
fn hold_record(store: &Store, id: &str) -> rusqlite::Result<Option<Value>> {
    Ok(store
        .subject(HOLD, id)?
        .map(|(_, value, _)| serde_json::from_str(&value).unwrap_or(Value::Null)))
}

/// The owner of a hold, if it exists.
pub fn hold_owner(store: &Store, id: &str) -> rusqlite::Result<Option<String>> {
    Ok(hold_record(store, id)?.and_then(|h| h["owner"].as_str().map(String::from)))
}

/// The artifact a hold is on, if it exists.
pub fn hold_artifact(store: &Store, id: &str) -> rusqlite::Result<Option<Value>> {
    Ok(hold_record(store, id)?.map(|h| h["artifact"].clone()))
}

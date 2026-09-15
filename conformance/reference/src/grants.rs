//! Grant evaluation (CORE section 15): coverage, delegation bounds and the provider clock.

use serde_json::Value;

/// Rights needed by an operation: (right, subject) pairs. `None` means the operation has
/// its own authorization rule (grant issue/revoke/get) or needs none.
pub fn required_rights(operation: &str, params: &Value) -> Option<Vec<(String, Value)>> {
    match operation {
        "core-test.subject.put" => {
            let mut needed = vec![("core-test.write".to_string(), params["subject"].clone())];
            for entry in params["preconditions"].as_array().into_iter().flatten() {
                if entry["subject"] != params["subject"] {
                    needed.push(("core-test.read".to_string(), entry["subject"].clone()));
                }
            }
            Some(needed)
        }
        "core-test.authority.claim" => Some(vec![(
            "core-test.claim".to_string(),
            params["subject"].clone(),
        )]),
        "execution.submit" => Some(vec![(
            "execution.submit".to_string(),
            params["subject"].clone(),
        )]),
        "execution.cancel" => Some(vec![(
            "execution.cancel".to_string(),
            params["subject"].clone(),
        )]),
        "execution.steer"
        | "execution.respond_action"
        | "execution.controller.claim"
        | "execution.workspace.checkpoint" => Some(vec![(
            params["operation"].as_str().unwrap_or_default().to_string(),
            params["subject"].clone(),
        )]),
        "execution.discovery.list" => Some(vec![(
            "execution.discovery.list".to_string(),
            serde_json::json!({"kind": "execution.discovery", "id": "installations"}),
        )]),
        "evidence.upload.prepare"
        | "evidence.upload.append"
        | "evidence.seal"
        | "evidence.upload.abandon" => Some(vec![(
            "evidence.publish".to_string(),
            params["subject"].clone(),
        )]),
        "context.request.submit" | "context.request.cancel" => Some(vec![(
            "context.request".to_string(),
            params["subject"].clone(),
        )]),
        "context.request.inspect" => Some(vec![(
            "context.read".to_string(),
            serde_json::json!({"kind": "context.request", "id": params["payload"]["request"]}),
        )]),
        "context.packet.inspect" | "context.expand" => Some(vec![(
            "context.packet.read".to_string(),
            serde_json::json!({"kind": "context.packet", "id": params["payload"]["packet"]}),
        )]),
        "evidence.hold" => Some(vec![(
            "evidence.hold".to_string(),
            serde_json::json!({"kind": "evidence.artifact", "id": params["payload"]["artifact"]["id"]}),
        )]),
        "evidence.inspect" | "evidence.fetch" => Some(vec![(
            "evidence.read".to_string(),
            serde_json::json!({"kind": "evidence.artifact", "id": params["payload"]["artifact"]["id"]}),
        )]),
        "execution.inspect" | "execution.output.read" => Some(vec![(
            "execution.read".to_string(),
            serde_json::json!({"kind": crate::execution::KIND, "id": params["payload"]["execution"]}),
        )]),
        "core-test.subject.get" | "core-test.subject.applied_count" => Some(vec![(
            "core-test.read".to_string(),
            params["payload"]["subject"].clone(),
        )]),
        "knowledge.claim.propose" | "knowledge.claim.revise" => Some(vec![(
            "knowledge.propose".to_string(),
            params["subject"].clone(),
        )]),
        "knowledge.conflict.open" | "knowledge.applicability.evaluate" => {
            let mut needed = vec![("knowledge.propose".to_string(), params["subject"].clone())];
            let named: Vec<&Value> = match params["payload"]["revisions"].as_array() {
                Some(revisions) => revisions.iter().collect(),
                None => vec![&params["payload"]["claim"]],
            };
            for reference in named {
                needed.push((
                    "knowledge.read".to_string(),
                    serde_json::json!({"kind": "knowledge.claim", "id": reference["claim"]}),
                ));
            }
            Some(needed)
        }
        "knowledge.decision.record" => Some(vec![
            ("knowledge.decide".to_string(), params["subject"].clone()),
            (
                "knowledge.read".to_string(),
                serde_json::json!({"kind": "knowledge.claim", "id": params["payload"]["claim"]["claim"]}),
            ),
        ]),
        "knowledge.conflict.resolve" => Some(vec![(
            "knowledge.decide".to_string(),
            params["subject"].clone(),
        )]),
        "knowledge.claim.inspect" | "knowledge.claim.history" => Some(vec![(
            "knowledge.read".to_string(),
            serde_json::json!({"kind": "knowledge.claim", "id": params["payload"]["claim"]}),
        )]),
        "knowledge.authority.get" => Some(vec![(
            "knowledge.read".to_string(),
            serde_json::json!({"kind": "knowledge.authority", "id": params["payload"]["scope"]}),
        )]),
        _ => None,
    }
}

pub fn resource_covers_subject(resource: &Value, subject: &Value) -> bool {
    if resource["kind"] != subject["kind"] {
        return false;
    }
    let id = subject["id"].as_str().unwrap_or_default();
    if let Some(exact) = resource.get("id").and_then(Value::as_str) {
        return exact == id;
    }
    if let Some(prefix) = resource.get("id_prefix").and_then(Value::as_str) {
        return id.starts_with(prefix);
    }
    true
}

/// Whether a parent resource covers a child resource (for delegation).
pub fn resource_covers_resource(parent: &Value, child: &Value) -> bool {
    if parent["kind"] != child["kind"] {
        return false;
    }
    match (
        parent.get("id").and_then(Value::as_str),
        parent.get("id_prefix").and_then(Value::as_str),
    ) {
        (None, None) => true,
        (Some(exact), _) => child.get("id").and_then(Value::as_str) == Some(exact),
        (None, Some(prefix)) => {
            child
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| id.starts_with(prefix))
                || child
                    .get("id_prefix")
                    .and_then(Value::as_str)
                    .is_some_and(|p| p.starts_with(prefix))
        }
    }
}

pub fn grant_covers(grant: &Value, right: &str, subject: &Value) -> Result<(), &'static str> {
    let has_right = grant["rights"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|r| r == right);
    if !has_right {
        return Err("right_missing");
    }
    let covered = grant["resources"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|resource| resource_covers_subject(resource, subject));
    if !covered {
        return Err("out_of_scope");
    }
    Ok(())
}

/// Delegation bounds of CORE section 15.3. Returns true when `child` stays within `parent`.
pub fn within_parent(parent: &Value, child: &Value) -> bool {
    let rights_ok = child["rights"].as_array().into_iter().flatten().all(|r| {
        parent["rights"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|p| p == r)
    });
    let resources_ok = child["resources"]
        .as_array()
        .into_iter()
        .flatten()
        .all(|c| {
            parent["resources"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|p| resource_covers_resource(p, c))
        });
    let expiry_ok = match parent.get("expires_at").and_then(Value::as_str) {
        None => true,
        Some(parent_expiry) => child
            .get("expires_at")
            .and_then(Value::as_str)
            .is_some_and(|c| c <= parent_expiry),
    };
    let parent_depth = parent["delegation"]["max_depth"].as_i64().unwrap_or(0);
    let depth_ok = child["delegation"]["max_depth"].as_i64().unwrap_or(0) < parent_depth;
    // A delegated grant keeps every constraint of its parent (CORE section 15.3).
    let constraints_ok = parent["constraints"]
        .as_array()
        .into_iter()
        .flatten()
        .all(|c| {
            child["constraints"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|d| d == c)
        });
    let binding_ok = match parent.get("authority_binding") {
        None => true,
        Some(binding) => child.get("authority_binding") == Some(binding),
    };
    rights_ok && resources_ok && expiry_ok && depth_ok && binding_ok && constraints_ok
}

/// Current system instant as `YYYY-MM-DDTHH:MM:SSZ` (used by `clock::Clock::System`).
pub fn now(fixed: Option<&str>) -> String {
    if let Some(fixed) = fixed {
        return fixed.to_string();
    }
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let days = seconds.div_euclid(86_400);
    let rem = seconds.rem_euclid(86_400);
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn coverage_and_delegation() {
        let parent = json!({"rights": ["core-test.read", "core-test.write"], "resources": [{"kind": "core-test.subject", "id_prefix": "s-"}],
                            "delegation": {"allowed": true, "max_depth": 1}, "expires_at": "2030-01-02T00:00:00Z"});
        let ok = json!({"rights": ["core-test.read"], "resources": [{"kind": "core-test.subject", "id": "s-1"}],
                        "delegation": {"allowed": false, "max_depth": 0}, "expires_at": "2030-01-01T00:00:00Z"});
        assert!(within_parent(&parent, &ok));
        let wider = json!({"rights": ["core-test.read"], "resources": [{"kind": "core-test.subject"}],
                           "delegation": {"allowed": false, "max_depth": 0}, "expires_at": "2030-01-01T00:00:00Z"});
        assert!(!within_parent(&parent, &wider));
        let no_expiry = json!({"rights": ["core-test.read"], "resources": [{"kind": "core-test.subject", "id": "s-1"}],
                               "delegation": {"allowed": false, "max_depth": 0}});
        assert!(!within_parent(&parent, &no_expiry));
        assert_eq!(
            grant_covers(
                &parent,
                "core-test.claim",
                &json!({"kind": "core-test.subject", "id": "s-1"})
            ),
            Err("right_missing")
        );
        assert_eq!(
            grant_covers(
                &parent,
                "core-test.read",
                &json!({"kind": "core-test.subject", "id": "t-1"})
            ),
            Err("out_of_scope")
        );
        assert!(now(None).len() == 20 && now(None).ends_with('Z'));
    }
}

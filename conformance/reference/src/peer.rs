//! Protocol client for provider-to-provider calls (owner decision M4-Q3).
//!
//! A reference participant reaches another participant only through its public Unix-socket
//! binding: it authenticates with its own credential, negotiates, and sends ordinary commands and
//! queries under grants issued by that participant. Nothing is shared but the protocol.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use serde_json::{Value, json};

use crate::json::{CanonicalFlaws, canonical, sha256_digest};

/// A peer as launch configuration names it: `{ provider_id, socket, credential, grant? }`.
pub struct Peer {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    next_id: i64,
    generation: Value,
    pub provider_id: String,
}

/// Why a peer call did not succeed.
pub enum Failure {
    /// The peer could not be reached, or the connection failed mid-call.
    Unreachable(String),
    /// The peer answered with a protocol error; the value is its error data.
    Refused(Value),
}

impl Failure {
    pub fn describe(&self) -> String {
        match self {
            Failure::Unreachable(reason) => format!("unreachable: {reason}"),
            Failure::Refused(data) => format!("refused: {data}"),
        }
    }
}

impl Peer {
    /// Connect, authenticate and negotiate `profiles` (a `core.negotiate` profiles array).
    pub fn connect(peer: &Value, profiles: Value) -> Result<Self, Failure> {
        let socket = peer["socket"]
            .as_str()
            .ok_or_else(|| Failure::Unreachable("peer has no socket".into()))?;
        let stream =
            UnixStream::connect(socket).map_err(|e| Failure::Unreachable(e.to_string()))?;
        stream
            .set_read_timeout(Some(Duration::from_millis(3000)))
            .map_err(|e| Failure::Unreachable(e.to_string()))?;
        let writer = stream
            .try_clone()
            .map_err(|e| Failure::Unreachable(e.to_string()))?;
        let mut client = Peer {
            reader: BufReader::new(stream),
            writer,
            next_id: 0,
            generation: json!(1),
            provider_id: peer["provider_id"].as_str().unwrap_or_default().to_string(),
        };
        client.call(
            "core.authenticate",
            json!({"operation": "core.authenticate", "message_id": "peer-authenticate", "payload": {"credential": peer["credential"]}}),
        )?;
        let negotiated = client.call(
            "core.negotiate",
            json!({"operation": "core.negotiate", "message_id": "peer-negotiate", "payload": {
                "caller": {"name": "combraton-reference-peer", "version": "0.1.0"},
                "receive_limits": {"max_frame_bytes": 1_048_576},
                "profiles": profiles,
            }}),
        )?;
        client.generation = negotiated["dedupe_window"]["current"].clone();
        Ok(client)
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value, Failure> {
        self.next_id += 1;
        let id = self.next_id;
        let frame = crate::json::encode_frame(
            &json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}),
        );
        self.writer
            .write_all(&frame)
            .map_err(|e| Failure::Unreachable(e.to_string()))?;
        loop {
            let mut line = String::new();
            let read = self
                .reader
                .read_line(&mut line)
                .map_err(|e| Failure::Unreachable(e.to_string()))?;
            if read == 0 {
                return Err(Failure::Unreachable("peer closed the connection".into()));
            }
            let Ok(frame) = serde_json::from_str::<Value>(&line) else {
                return Err(Failure::Unreachable("peer sent an unparsable frame".into()));
            };
            if frame["id"] != id {
                continue; // a notification
            }
            if let Some(error) = frame.get("error") {
                return Err(Failure::Refused(error["data"].clone()));
            }
            return Ok(frame["result"].clone());
        }
    }

    #[allow(dead_code)] // used by executor packet checks (M4 step 5)
    pub fn query(
        &mut self,
        operation: &str,
        payload: Value,
        grant: Option<&str>,
    ) -> Result<Value, Failure> {
        self.next_id += 1;
        let mut params = json!({"operation": operation, "message_id": format!("peer-query-{}", self.next_id), "payload": payload});
        if let Some(grant) = grant {
            params["grant"] = json!(grant);
        }
        self.call(operation, params)
    }

    pub fn command(
        &mut self,
        operation: &str,
        command_id: &str,
        subject: Value,
        preconditions: Value,
        payload: Value,
        grant: Option<&str>,
    ) -> Result<Value, Failure> {
        let intent = json!({
            "operation": operation,
            "subject": subject,
            "preconditions": preconditions,
            "requires": [],
            "payload": payload,
            "extensions": {},
        });
        let digest = sha256_digest(&canonical(&intent, CanonicalFlaws::default()));
        let mut params = json!({
            "operation": operation,
            "message_id": format!("peer-command-{command_id}"),
            "command_id": command_id,
            "dedupe_generation": self.generation,
            "subject": subject,
            "preconditions": preconditions,
            "requires": [],
            "payload": payload,
            "command_digest": digest,
        });
        if let Some(grant) = grant {
            params["grant"] = json!(grant);
        }
        self.call(operation, params)
    }
}

/// Core and Evidence profiles for a peer that publishes or reads evidence.
pub fn evidence_profiles(grants: bool) -> Value {
    let mut core = vec!["core.events"];
    if grants {
        core.push("core.grants");
    }
    json!([
        {"name": "core", "majors": [1], "required": true, "required_features": core, "optional_features": []},
        {"name": "evidence", "majors": [1], "required": true, "required_features": [], "optional_features": []},
    ])
}

/// Publish provider-produced bytes as one sealed artifact at an evidence peer, with deterministic
/// command IDs so a retried publication replays instead of duplicating (CORE section 6).
pub fn publish_artifact(
    peer: &Value,
    artifact: &str,
    descriptor: Value,
    content: &[u8],
) -> Result<String, Failure> {
    let grant = peer["grant"].as_str();
    let mut client = Peer::connect(peer, evidence_profiles(grant.is_some()))?;
    let subject = json!({"kind": crate::evidence::ARTIFACT, "id": artifact});
    let prepared = client.command(
        "evidence.upload.prepare",
        &format!("publish.{artifact}.prepare"),
        subject.clone(),
        json!([{"subject": subject, "revision": 0}]),
        descriptor,
        grant,
    )?;
    let chunk = prepared["outcome"]["chunk_limit"]
        .as_u64()
        .unwrap_or(4096)
        .max(1) as usize;
    let mut revision = 1;
    for (index, part) in content.chunks(chunk).enumerate() {
        let offset = index * chunk;
        client.command(
            "evidence.upload.append",
            &format!("publish.{artifact}.append.{offset}"),
            subject.clone(),
            json!([{"subject": subject, "revision": revision}]),
            json!({"offset": offset, "data_base64": crate::json::base64(part)}),
            grant,
        )?;
        revision += 1;
    }
    client.command(
        "evidence.seal",
        &format!("publish.{artifact}.seal"),
        subject.clone(),
        json!([{"subject": subject, "revision": revision}]),
        json!({}),
        grant,
    )?;
    Ok(client.provider_id.clone())
}

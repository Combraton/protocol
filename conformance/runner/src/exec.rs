//! Executing one fixture against one participant.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use crate::matcher::{Vars, matches, render};
use crate::participant::{Descriptor, Launch, Received, Session};
use crate::schemas::{Schemas, jsonrpc_code, retry_class};
use crate::strict;

const RESPONSE_TIMEOUT: Duration = Duration::from_millis(5000);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail,
    NotApplicable,
    Timeout,
    HarnessError,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Pass => "pass",
            Outcome::Fail => "fail",
            Outcome::NotApplicable => "not_applicable",
            Outcome::Timeout => "timeout",
            Outcome::HarnessError => "harness_error",
        }
    }
}

pub struct CaseResult {
    pub outcome: Outcome,
    pub step: Option<usize>,
    pub reason: Option<String>,
    pub transcript: Vec<Value>,
}

enum StepError {
    Fail(String),
    Timeout(String),
    Harness(String),
}

use StepError::{Fail, Harness, Timeout};

pub struct Context<'a> {
    pub descriptor: &'a Descriptor,
    pub schemas: &'a Schemas,
    pub repo: &'a Path,
    pub mutant: Option<&'a str>,
    pub work_dir: PathBuf,
}

struct State<'a> {
    ctx: &'a Context<'a>,
    fixture: &'a Value,
    vars: Vars,
    counter: u64,
    next_id: i64,
    session: Option<Session>,
    notifications: VecDeque<Value>,
    transcript: Vec<Value>,
    started: Instant,
}

pub fn run_fixture(fixture: &Value, ctx: &Context) -> CaseResult {
    for profile in fixture["profiles"].as_array().into_iter().flatten() {
        let wanted = (
            profile["name"].as_str().unwrap_or_default().to_string(),
            profile["major"].as_i64().unwrap_or_default(),
        );
        if !ctx.descriptor.profiles.contains(&wanted) {
            return CaseResult {
                outcome: Outcome::NotApplicable,
                step: None,
                reason: Some(format!(
                    "participant does not claim {}/{}",
                    wanted.0, wanted.1
                )),
                transcript: vec![],
            };
        }
    }
    let claimed_features: Vec<&str> = ctx.descriptor.raw["claims"]["features"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    for feature in fixture["features"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if !claimed_features.contains(&feature) {
            return CaseResult {
                outcome: Outcome::NotApplicable,
                step: None,
                reason: Some(format!("participant does not claim feature {feature}")),
                transcript: vec![],
            };
        }
    }
    if let Err(error) = std::fs::create_dir_all(ctx.work_dir.join("data")) {
        return CaseResult {
            outcome: Outcome::HarnessError,
            step: None,
            reason: Some(error.to_string()),
            transcript: vec![],
        };
    }
    let mut state = State {
        ctx,
        fixture,
        vars: Vars::new(),
        counter: 0,
        next_id: 0,
        session: None,
        notifications: VecDeque::new(),
        transcript: vec![],
        started: Instant::now(),
    };
    let mut failure = None;
    for (index, step) in fixture["steps"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        if let Err(error) = state.step(step) {
            failure = Some((index, error));
            break;
        }
    }
    if let Some(session) = state.session.take() {
        state.transcript.extend(session.finish());
    }
    let (outcome, step, reason) = match failure {
        None => (Outcome::Pass, None, None),
        Some((index, Fail(reason))) => (Outcome::Fail, Some(index), Some(reason)),
        Some((index, Timeout(reason))) => (Outcome::Timeout, Some(index), Some(reason)),
        Some((index, Harness(reason))) => (Outcome::HarnessError, Some(index), Some(reason)),
    };
    CaseResult {
        outcome,
        step,
        reason,
        transcript: state.transcript,
    }
}

fn deep_merge(base: &mut Value, overlay: &Value) {
    if overlay.is_null() {
        return;
    }
    match (base, overlay) {
        (Value::Object(base), Value::Object(overlay)) => {
            for (key, value) in overlay {
                deep_merge(base.entry(key.clone()).or_insert(Value::Null), value);
            }
        }
        (base, overlay) => *base = overlay.clone(),
    }
}

impl State<'_> {
    fn session(&mut self) -> Result<&mut Session, StepError> {
        self.session
            .as_mut()
            .ok_or_else(|| Harness("no participant session; add a start step".into()))
    }

    fn render(&mut self, template: &Value) -> Result<Value, StepError> {
        render(template, &self.vars, &mut self.counter).map_err(Harness)
    }

    fn step(&mut self, step: &Value) -> Result<(), StepError> {
        let kind = step["step"].as_str().unwrap_or_default();
        match kind {
            "start" => self.start(step),
            "stop" => self.stop(),
            "describe" => {
                let params = json!({"operation": "core.describe", "message_id": self.unique("msg"), "payload": {}});
                let frame = self.call("core.describe", params, step)?;
                if let Some(result) = frame.get("result") {
                    self.vars.insert("describe".into(), result.clone());
                }
                Ok(())
            }
            "negotiate" => self.negotiate(step),
            "request" => {
                let params = self.render(&step["params"])?;
                let method = step["method"].as_str().unwrap_or_default().to_string();
                self.call(&method, params, step).map(|_| ())
            }
            "command" => {
                let params = self.command_envelope(step)?;
                let method = step["method"]
                    .as_str()
                    .map(String::from)
                    .unwrap_or_else(|| {
                        params["operation"].as_str().unwrap_or_default().to_string()
                    });
                self.call(&method, params, step).map(|_| ())
            }
            "query" => {
                let params = self.query_envelope(step)?;
                let method = step["method"]
                    .as_str()
                    .map(String::from)
                    .unwrap_or_else(|| {
                        params["operation"].as_str().unwrap_or_default().to_string()
                    });
                self.call(&method, params, step).map(|_| ())
            }
            "notify" => {
                let params = self.command_envelope(&step["command"])?;
                let method = params["operation"].as_str().unwrap_or_default().to_string();
                let bytes = frame_bytes(
                    &json!({"jsonrpc": "2.0", "method": method, "params": params}),
                    true,
                );
                self.session()?.send(&bytes);
                Ok(())
            }
            "raw" => self.raw(step),
            "expect_frame" => {
                let frame = self.receive_frame(duration(step, RESPONSE_TIMEOUT))?;
                self.check(&frame, &step["expect"], None)?;
                self.capture(&frame, step)
            }
            "expect_close" => {
                let timeout = duration(step, Duration::from_millis(3000));
                match self.session()?.receive(timeout) {
                    Received::Closed => Ok(()),
                    Received::Timeout => Err(Fail("connection was not closed".into())),
                    Received::Frame(frame) => Err(Fail(format!(
                        "expected the connection to close, received {}",
                        String::from_utf8_lossy(&frame[..frame.len().min(300)])
                    ))),
                }
            }
            "expect_notification" => {
                let frame = self.next_notification(duration(step, RESPONSE_TIMEOUT))?;
                if let Some(pattern) = step["expect"].get("params") {
                    matches(pattern, frame.get("params"), &self.vars, "params").map_err(Fail)?;
                }
                if let Some(pattern) = step["expect"].get("frame") {
                    matches(pattern, Some(&frame), &self.vars, "frame").map_err(Fail)?;
                }
                self.capture(&frame, step)
            }
            "expect_no_notification" => {
                // Barrier: a describe round trip flushes any notification owed before it.
                let params = json!({"operation": "core.describe", "message_id": self.unique("barrier"), "payload": {}});
                self.call("core.describe", params, &json!({}))?;
                match self.notifications.pop_front() {
                    None => Ok(()),
                    Some(frame) => Err(Fail(format!("unexpected notification {frame}"))),
                }
            }
            "close_input" => {
                self.session()?.close_input();
                Ok(())
            }
            other => Err(Harness(format!("unknown step {other:?}"))),
        }
    }

    fn unique(&mut self, prefix: &str) -> String {
        self.counter += 1;
        format!("{prefix}-{}", self.counter)
    }

    fn start(&mut self, step: &Value) -> Result<(), StepError> {
        if self.session.is_some() {
            return Err(Harness("participant already started".into()));
        }
        let mut config =
            json!({"format": "combraton-conformance-config/1", "principal": "conformance-caller"});
        deep_merge(&mut config, &self.fixture["config"]);
        if step.get("config").is_some() {
            deep_merge(&mut config, &step["config"]);
        }
        if let Some(error) = self.ctx.schemas.launch_config.iter_errors(&config).next() {
            return Err(Harness(format!(
                "fixture launch configuration is invalid at {}: {error}",
                error.instance_path()
            )));
        }
        let config_file = self.ctx.work_dir.join("config.json");
        std::fs::write(&config_file, serde_json::to_vec_pretty(&config).unwrap())
            .map_err(|e| Harness(e.to_string()))?;
        let data_dir = self.ctx.work_dir.join("data");
        let launch = Launch {
            descriptor: self.ctx.descriptor,
            repo: self.ctx.repo,
            data_dir: &data_dir,
            config_file: &config_file,
            stderr_file: self.ctx.work_dir.join("participant-stderr.log"),
            mutant: self.ctx.mutant,
        };
        self.session = Some(Session::start(&launch, self.started).map_err(Harness)?);
        self.vars.remove("negotiation");
        Ok(())
    }

    fn stop(&mut self) -> Result<(), StepError> {
        let session = self.session()?;
        session.close_input();
        match session.receive(Duration::from_millis(5000)) {
            Received::Closed => {}
            Received::Timeout => {
                return Err(Fail(
                    "participant did not close its output after input ended".into(),
                ));
            }
            Received::Frame(_) => return Err(Fail("unexpected frame after input ended".into())),
        }
        if session.wait_exit(Duration::from_millis(5000)).is_none() {
            return Err(Fail("participant did not exit after input ended".into()));
        }
        let session = self.session.take().unwrap();
        self.transcript.extend(session.finish());
        Ok(())
    }

    fn negotiate(&mut self, step: &Value) -> Result<(), StepError> {
        let payload = if step.get("payload").is_some() {
            self.render(&step["payload"])?
        } else {
            let profiles: Vec<Value> = self.fixture["profiles"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|p| json!({"name": p["name"], "majors": [p["major"]], "required": true, "required_features": [], "optional_features": []}))
                .collect();
            json!({
                "caller": {"name": "combraton-conformance", "version": env!("CARGO_PKG_VERSION")},
                "receive_limits": {"max_frame_bytes": 1_048_576},
                "profiles": profiles,
            })
        };
        let params = json!({"operation": "core.negotiate", "message_id": self.unique("msg"), "payload": payload});
        let frame = self.call("core.negotiate", params, step)?;
        if let Some(result) = frame.get("result") {
            self.vars.insert(
                "generation".into(),
                result["dedupe_window"]["current"].clone(),
            );
            self.vars.insert("negotiation".into(), result.clone());
        }
        Ok(())
    }

    fn apply_overrides(&mut self, envelope: &mut Value, step: &Value) -> Result<(), StepError> {
        if let Some(set) = step.get("set").and_then(Value::as_object) {
            for (key, value) in set.clone() {
                let rendered = self.render(&value)?;
                envelope[key] = rendered;
            }
        }
        for key in step
            .get("remove")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            envelope
                .as_object_mut()
                .unwrap()
                .remove(key.as_str().unwrap_or_default());
        }
        Ok(())
    }

    fn command_envelope(&mut self, step: &Value) -> Result<Value, StepError> {
        let generation = match step.get("dedupe_generation") {
            Some(value) => self.render(value)?,
            None => self.vars.get("generation").cloned().ok_or_else(|| {
                Harness("command before negotiation; set dedupe_generation".into())
            })?,
        };
        let mut envelope = Map::new();
        envelope.insert("operation".into(), step["operation"].clone());
        envelope.insert("message_id".into(), json!(self.unique("msg")));
        envelope.insert("command_id".into(), self.render(&step["command_id"])?);
        envelope.insert("dedupe_generation".into(), generation);
        envelope.insert("subject".into(), self.render(&step["subject"])?);
        envelope.insert("preconditions".into(), self.render(&step["preconditions"])?);
        if let Some(epoch) = step.get("authority_epoch") {
            envelope.insert("authority_epoch".into(), self.render(epoch)?);
        }
        envelope.insert(
            "requires".into(),
            step.get("requires")
                .map_or(Ok(json!([])), |r| self.render(r))?,
        );
        if let Some(extensions) = step.get("extensions") {
            envelope.insert("extensions".into(), self.render(extensions)?);
        }
        envelope.insert("payload".into(), self.render(&step["payload"])?);
        let mut envelope = Value::Object(envelope);
        self.apply_overrides(&mut envelope, step)?;
        let digest = match step.get("digest") {
            Some(Value::String(literal)) => json!(literal),
            _ => json!(strict::intent_digest(&envelope)),
        };
        if envelope.get("command_digest").is_none() && step.get("digest") != Some(&json!(false)) {
            envelope["command_digest"] = digest;
        }
        Ok(envelope)
    }

    fn query_envelope(&mut self, step: &Value) -> Result<Value, StepError> {
        let mut envelope = json!({"operation": step["operation"], "message_id": self.unique("msg"), "payload": self.render(&step["payload"])?});
        self.apply_overrides(&mut envelope, step)?;
        Ok(envelope)
    }

    fn raw(&mut self, step: &Value) -> Result<(), StepError> {
        let terminate = step["terminate"].as_bool().unwrap_or(true);
        let mut bytes = if let Some(text) = step["text"].as_str() {
            text.as_bytes().to_vec()
        } else if let Some(hex_text) = step["hex"].as_str() {
            hex::decode(hex_text).map_err(|e| Harness(e.to_string()))?
        } else if step.get("command").is_some() {
            let params = self.command_envelope(&step["command"])?;
            self.next_id += 1;
            let request = json!({"jsonrpc": "2.0", "id": self.next_id, "method": params["operation"], "params": params});
            frame_bytes(&request, false)
        } else if let Some(padded) = step.get("padded") {
            let message = self.render(&padded["message"])?;
            let target = padded["frame_bytes"].as_u64().unwrap_or_default() as usize;
            let compact = frame_bytes(&message, false);
            if compact.len() > target || compact.first() != Some(&b'{') {
                return Err(Harness(format!(
                    "padded message is {} bytes, longer than {target}",
                    compact.len()
                )));
            }
            let mut padded_bytes = vec![b'{'];
            padded_bytes.extend(std::iter::repeat_n(b' ', target - compact.len()));
            padded_bytes.extend_from_slice(&compact[1..]);
            padded_bytes
        } else {
            return Err(Harness(
                "raw step needs text, hex, command or padded".into(),
            ));
        };
        if terminate {
            bytes.push(b'\n');
        }
        self.session()?.send(&bytes);
        Ok(())
    }

    /// Next frame that is not a provider notification; notifications are validated and queued.
    fn receive_frame(&mut self, timeout: Duration) -> Result<Value, StepError> {
        let deadline = Instant::now() + timeout;
        loop {
            let frame =
                self.receive_raw_frame(deadline.saturating_duration_since(Instant::now()))?;
            if frame.get("id").is_none() && frame.get("method").is_some() {
                self.ctx.schemas.check_notification(&frame).map_err(Fail)?;
                self.notifications.push_back(frame);
                continue;
            }
            return Ok(frame);
        }
    }

    fn next_notification(&mut self, timeout: Duration) -> Result<Value, StepError> {
        if let Some(frame) = self.notifications.pop_front() {
            return Ok(frame);
        }
        let frame = self.receive_raw_frame(timeout)?;
        if frame.get("id").is_none() && frame.get("method").is_some() {
            self.ctx.schemas.check_notification(&frame).map_err(Fail)?;
            return Ok(frame);
        }
        Err(Fail(format!("expected a notification, received {frame}")))
    }

    fn receive_raw_frame(&mut self, timeout: Duration) -> Result<Value, StepError> {
        let frame = match self.session()?.receive(timeout) {
            Received::Frame(frame) => frame,
            Received::Closed => return Err(Fail("participant closed the connection".into())),
            Received::Timeout => {
                return Err(Timeout(format!(
                    "no frame within {} ms",
                    timeout.as_millis()
                )));
            }
        };
        strict::parse(&frame).map_err(|e| {
            Fail(format!(
                "participant sent a frame outside the value domain: {e}"
            ))
        })
    }

    fn call(&mut self, method: &str, params: Value, step: &Value) -> Result<Value, StepError> {
        let id = match step.get("id") {
            Some(id) => self.render(id)?,
            None => {
                self.next_id += 1;
                json!(self.next_id)
            }
        };
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let bytes = frame_bytes(&request, true);
        self.session()?.send(&bytes);
        let frame = self.receive_frame(duration(step, RESPONSE_TIMEOUT))?;
        if frame.get("id") != Some(&id) {
            return Err(Fail(format!(
                "response id {} does not match request id {id}",
                frame.get("id").unwrap_or(&Value::Null)
            )));
        }
        self.check(
            &frame,
            step.get("expect").unwrap_or(&json!({"ok": {}})),
            Some(method),
        )?;
        self.capture(&frame, step)?;
        Ok(frame)
    }

    fn check(&self, frame: &Value, expect: &Value, method: Option<&str>) -> Result<(), StepError> {
        self.ctx
            .schemas
            .check_response(frame, method)
            .map_err(Fail)?;
        if let Some(pattern) = expect.get("ok") {
            let result = frame.get("result").ok_or_else(|| {
                Fail(format!(
                    "expected success, received error {}",
                    frame["error"]["data"]
                ))
            })?;
            return matches(pattern, Some(result), &self.vars, "result").map_err(Fail);
        }
        if let Some(code) = expect.get("error").and_then(Value::as_str) {
            let error = frame.get("error").ok_or_else(|| {
                Fail(format!(
                    "expected error {code}, received success {}",
                    frame["result"]
                ))
            })?;
            let data = &error["data"];
            if data["code"] != code {
                return Err(Fail(format!(
                    "expected error {code}, received {} ({})",
                    data["code"], data["details"]
                )));
            }
            if error["code"] != jsonrpc_code(code) {
                return Err(Fail(format!(
                    "error {code} must use JSON-RPC code {}, received {}",
                    jsonrpc_code(code),
                    error["code"]
                )));
            }
            if data["retry"] != retry_class(code) {
                return Err(Fail(format!(
                    "error {code} must have retry {}, received {}",
                    retry_class(code),
                    data["retry"]
                )));
            }
            if let Some(details) = expect.get("details") {
                matches(details, Some(&data["details"]), &self.vars, "details").map_err(Fail)?;
            }
            if expect.get("id_null") == Some(&json!(true)) && frame["id"] != Value::Null {
                return Err(Fail("error must have id null".into()));
            }
            return Ok(());
        }
        if let Some(pattern) = expect.get("frame") {
            return matches(pattern, Some(frame), &self.vars, "frame").map_err(Fail);
        }
        Ok(())
    }

    fn capture(&mut self, frame: &Value, step: &Value) -> Result<(), StepError> {
        for (name, pointer) in step
            .get("capture")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            let pointer = pointer.as_str().unwrap_or_default();
            let value = frame
                .pointer(pointer)
                .ok_or_else(|| Fail(format!("capture {name}: nothing at {pointer}")))?;
            self.vars.insert(name.clone(), value.clone());
        }
        Ok(())
    }
}

fn frame_bytes(message: &Value, terminate: bool) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(message).expect("serializable");
    if terminate {
        bytes.push(b'\n');
    }
    bytes
}

fn duration(step: &Value, default: Duration) -> Duration {
    step.get("within_ms")
        .and_then(Value::as_u64)
        .map_or(default, Duration::from_millis)
}

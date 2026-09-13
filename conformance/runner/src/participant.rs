//! Launching a participant under test over the stdio binding, with a transcript.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const RUNNER_FRAME_LIMIT: usize = 64 * 1024 * 1024;

pub struct Descriptor {
    pub raw: Value,
    pub name: String,
    pub version: String,
    pub argv: Vec<String>,
    pub mutant_argv: Vec<String>,
    pub mutants: Vec<String>,
    pub profiles: Vec<(String, i64)>,
    pub digest: String,
}

impl Descriptor {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let raw = crate::strict::parse(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        if raw["format"] != "combraton-conformance-participant/1" {
            return Err(format!(
                "{}: format must be combraton-conformance-participant/1",
                path.display()
            ));
        }
        if raw["binding"] != "stdio" || raw["role"] != "provider" {
            return Err("only stdio providers are supported in M1".into());
        }
        let strings = |value: &Value| -> Vec<String> {
            value
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        };
        Ok(Self {
            name: raw["name"].as_str().unwrap_or_default().into(),
            version: raw["version"].as_str().unwrap_or_default().into(),
            argv: strings(&raw["launch"]["argv"]),
            mutant_argv: strings(&raw["mutants"]["argv"]),
            mutants: strings(&raw["mutants"]["names"]),
            profiles: raw["claims"]["profiles"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|p| {
                    (
                        p["name"].as_str().unwrap_or_default().to_string(),
                        p["major"].as_i64().unwrap_or_default(),
                    )
                })
                .collect(),
            digest: crate::strict::sha256(&crate::strict::canonical(&raw)),
            raw,
        })
    }
}

pub enum Received {
    Frame(Vec<u8>),
    Closed,
    Timeout,
}

enum Event {
    Frame(Vec<u8>),
    Eof,
}

pub struct Session {
    child: Child,
    stdin: Option<ChildStdin>,
    events: Receiver<Event>,
    closed: bool,
    started: Instant,
    transcript: Vec<Value>,
}

pub struct Launch<'a> {
    pub descriptor: &'a Descriptor,
    pub repo: &'a Path,
    pub data_dir: &'a Path,
    pub config_file: &'a Path,
    pub stderr_file: PathBuf,
    pub mutant: Option<&'a str>,
}

impl Session {
    pub fn start(launch: &Launch, started: Instant) -> Result<Self, String> {
        let substitute = |arg: &str| {
            arg.replace("{repo}", &launch.repo.display().to_string())
                .replace("{data_dir}", &launch.data_dir.display().to_string())
                .replace("{config_file}", &launch.config_file.display().to_string())
                .replace("{mutant}", launch.mutant.unwrap_or_default())
        };
        let mut argv: Vec<String> = launch
            .descriptor
            .argv
            .iter()
            .map(|a| substitute(a))
            .collect();
        if launch.mutant.is_some() {
            argv.extend(launch.descriptor.mutant_argv.iter().map(|a| substitute(a)));
        }
        let (program, rest) = argv
            .split_first()
            .ok_or("participant launch argv is empty")?;
        let stderr = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&launch.stderr_file)
            .map_err(|e| e.to_string())?;
        let mut child = Command::new(program)
            .args(rest)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(|e| format!("cannot launch {program}: {e}"))?;
        let mut stdout = child.stdout.take().ok_or("no stdout")?;
        let stdin = child.stdin.take();
        let (sender, events) = channel();
        thread::spawn(move || {
            let mut buffer = Vec::new();
            let mut chunk = [0u8; 65_536];
            loop {
                match stdout.read(&mut chunk) {
                    Ok(0) | Err(_) => {
                        let _ = sender.send(Event::Eof);
                        return;
                    }
                    Ok(read) => {
                        buffer.extend_from_slice(&chunk[..read]);
                        while let Some(newline) = buffer.iter().position(|b| *b == b'\n') {
                            let frame: Vec<u8> = buffer.drain(..=newline).collect();
                            if sender
                                .send(Event::Frame(frame[..frame.len() - 1].to_vec()))
                                .is_err()
                            {
                                return;
                            }
                        }
                        if buffer.len() > RUNNER_FRAME_LIMIT {
                            let _ = sender.send(Event::Eof);
                            return;
                        }
                    }
                }
            }
        });
        let mut session = Self {
            child,
            stdin,
            events,
            closed: false,
            started,
            transcript: Vec::new(),
        };
        session.note("start", &json!({"argv": argv}));
        Ok(session)
    }

    fn elapsed(&self) -> u128 {
        self.started.elapsed().as_millis()
    }

    pub fn note(&mut self, event: &str, detail: &Value) {
        let entry = json!({"t_ms": self.elapsed() as u64, "event": event, "detail": detail});
        self.transcript.push(entry);
    }

    fn record_bytes(&mut self, direction: &str, bytes: &[u8]) {
        const SHOWN: usize = 2048;
        let shown = &bytes[..bytes.len().min(SHOWN)];
        let body = match std::str::from_utf8(shown) {
            Ok(text) => json!({"text": text}),
            Err(_) => json!({"hex": hex::encode(shown)}),
        };
        let mut detail = json!({"bytes": bytes.len()});
        detail
            .as_object_mut()
            .unwrap()
            .extend(body.as_object().unwrap().clone());
        if bytes.len() > SHOWN {
            detail["truncated"] = json!(true);
        }
        self.note(direction, &detail);
    }

    /// Write bytes; a peer that has already closed is not a runner error.
    pub fn send(&mut self, bytes: &[u8]) {
        self.record_bytes("send", bytes);
        let failed = match self.stdin.as_mut() {
            Some(stdin) => stdin.write_all(bytes).and_then(|()| stdin.flush()).is_err(),
            None => true,
        };
        if failed {
            self.note("send_failed", &json!({"reason": "peer input closed"}));
        }
    }

    pub fn close_input(&mut self) {
        self.note("close_input", &json!({}));
        self.stdin = None;
    }

    pub fn receive(&mut self, timeout: Duration) -> Received {
        if self.closed {
            return Received::Closed;
        }
        match self.events.recv_timeout(timeout) {
            Ok(Event::Frame(frame)) => {
                self.record_bytes("recv", &frame);
                Received::Frame(frame)
            }
            Ok(Event::Eof) | Err(RecvTimeoutError::Disconnected) => {
                self.closed = true;
                self.note("peer_closed", &json!({}));
                Received::Closed
            }
            Err(RecvTimeoutError::Timeout) => Received::Timeout,
        }
    }

    pub fn wait_exit(&mut self, timeout: Duration) -> Option<i32> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Ok(Some(status)) = self.child.try_wait() {
                self.note("exit", &json!({"code": status.code()}));
                return Some(status.code().unwrap_or(-1));
            }
            thread::sleep(Duration::from_millis(10));
        }
        None
    }

    /// Stop the process and return the transcript.
    pub fn finish(mut self) -> Vec<Value> {
        self.stdin = None;
        if self.wait_exit(Duration::from_millis(2000)).is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
            self.note(
                "killed",
                &json!({"reason": "did not exit after input closed"}),
            );
        }
        std::mem::take(&mut self.transcript)
    }
}

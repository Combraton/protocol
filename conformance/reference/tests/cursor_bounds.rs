//! Reference-provider-specific cursor checks.
//!
//! Cursors are opaque in the protocol (CORE section 16.4), so the universal fixtures never
//! construct one. A cursor past the head of the current epoch therefore cannot be reached by a
//! portable fixture. This test knows the reference provider's private cursor format
//! (`ev1:<stream>:<epoch>:<sequence>`) and checks that such cursors are refused. It is evidence
//! for this implementation only, not a format other providers must use.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

struct Provider {
    child: std::process::Child,
    input: std::process::ChildStdin,
    output: BufReader<std::process::ChildStdout>,
    next_id: i64,
    _dir: tempfile::TempDir,
}

impl Provider {
    fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.json");
        std::fs::write(
            &config,
            json!({"format": "combraton-conformance-config/1", "principal": "owner"}).to_string(),
        )
        .unwrap();
        let data = dir.path().join("data");
        std::fs::create_dir_all(&data).unwrap();
        let schemas = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
        let mut child = Command::new(env!("CARGO_BIN_EXE_combraton-reference-provider"))
            .arg("--data-dir")
            .arg(&data)
            .arg("--config")
            .arg(&config)
            .arg("--schemas")
            .arg(&schemas)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            input,
            output,
            next_id: 0,
            _dir: dir,
        }
    }

    fn query(&mut self, operation: &str, payload: Value) -> Value {
        self.next_id += 1;
        let request = json!({"jsonrpc": "2.0", "id": self.next_id, "method": operation,
            "params": {"operation": operation, "message_id": format!("m-{}", self.next_id), "payload": payload}});
        writeln!(self.input, "{request}").unwrap();
        self.input.flush().unwrap();
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }
}

impl Drop for Provider {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn constructed_cursors_past_the_head_are_refused() {
    let mut provider = Provider::start();
    let negotiated = provider.query(
        "core.negotiate",
        json!({"caller": {"name": "cursor-test", "version": "1"}, "receive_limits": {"max_frame_bytes": 1_048_576},
               "profiles": [{"name": "core", "majors": [1], "required": true, "required_features": ["core.events"], "optional_features": []}]}),
    );
    assert!(negotiated.get("result").is_some(), "{negotiated}");
    let now = provider.query("core.events.read", json!({"from": "now", "limit": 10}));
    let stream = now["result"]["stream"]["id"].as_str().unwrap().to_string();
    assert_eq!(
        now["result"]["next_cursor"],
        json!(format!("ev1:{stream}:1:0"))
    );

    let read = |provider: &mut Provider, cursor: String| {
        provider.query("core.events.read", json!({"cursor": cursor, "limit": 10}))
    };
    let at_head = read(&mut provider, format!("ev1:{stream}:1:0"));
    assert_eq!(at_head["result"]["items"], json!([]), "{at_head}");
    for cursor in [
        format!("ev1:{stream}:1:1"),
        format!("ev1:{stream}:2:0"),
        format!("ev1:{stream}:0:0"),
        format!("ev1:{stream}:1:-1"),
    ] {
        let refused = read(&mut provider, cursor.clone());
        assert_eq!(
            refused["error"]["data"]["code"], "invalid_cursor",
            "{cursor}: {refused}"
        );
    }
}

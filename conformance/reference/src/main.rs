//! Reference Core provider for Combraton Protocol conformance (draft 0.1).
//!
//! Not a product. It speaks the stdio form of the local stream binding and
//! implements core/1 plus the conformance-only core-test/1 profile.

mod frames;
mod json;
mod mutants;
mod provider;
mod store;

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::{Value, json};

use frames::{FrameReader, Next};
use json::{FrameFault, Laxness};
use provider::{Limits, Provider, error_response};

const USAGE: &str = "usage: combraton-reference-provider --data-dir DIR --config FILE --schemas DIR [--mutant NAME]... | --list-mutants";

struct Args {
    data_dir: PathBuf,
    config: PathBuf,
    schemas: PathBuf,
    mutants: Vec<String>,
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut data_dir = None;
    let mut config = None;
    let mut schemas = None;
    let mut mutants = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--list-mutants" => {
                for (name, description) in mutants::ALL {
                    println!("{name}\t{description}");
                }
                return Ok(None);
            }
            "--data-dir" => data_dir = args.next().map(PathBuf::from),
            "--config" => config = args.next().map(PathBuf::from),
            "--schemas" => schemas = args.next().map(PathBuf::from),
            "--mutant" => mutants.push(args.next().ok_or("--mutant needs a name")?),
            other => return Err(format!("unexpected argument {other}")),
        }
    }
    match (data_dir, config, schemas) {
        (Some(data_dir), Some(config), Some(schemas)) => Ok(Some(Args {
            data_dir,
            config,
            schemas,
            mutants,
        })),
        _ => Err(USAGE.to_string()),
    }
}

fn load_validators(
    schemas: &std::path::Path,
) -> Result<HashMap<&'static str, jsonschema::Validator>, String> {
    let mut resources = Vec::new();
    let mut stack = vec![schemas.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "json") {
                let value: Value =
                    serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
                        .map_err(|e| format!("{}: {e}", path.display()))?;
                let id = value["$id"]
                    .as_str()
                    .ok_or_else(|| format!("{} has no $id", path.display()))?
                    .to_string();
                resources.push((id, value));
            }
        }
    }
    let registry = jsonschema::Registry::new()
        .extend(resources)
        .map_err(|e| e.to_string())?
        .prepare()
        .map_err(|e| e.to_string())?;
    let mut validators = HashMap::new();
    for operation in Provider::operation_names() {
        let profile = operation.split('.').next().unwrap_or_default();
        let id = format!(
            "https://github.com/Combraton/protocol/schemas/{profile}/1/{operation}.params.schema.json"
        );
        let validator = jsonschema::options()
            .with_registry(&registry)
            .build(&json!({"$ref": id}))
            .map_err(|e| format!("{operation}: {e}"))?;
        validators.insert(operation, validator);
    }
    Ok(validators)
}

fn config_limits(config: &Value) -> Limits {
    let limit = |name: &str, default: i64| config["limits"][name].as_i64().unwrap_or(default);
    Limits {
        max_frame_bytes: limit("max_frame_bytes", 1_048_576),
        max_payload_bytes: limit("max_payload_bytes", 65_536),
        max_string_bytes: limit("max_string_bytes", 16_384),
        max_array_items: limit("max_array_items", 256),
        max_depth: limit("max_depth", 32),
    }
}

fn run(args: Args) -> Result<(), String> {
    let mutant_set = mutants::Mutants::parse(&args.mutants)?;
    let config: Value =
        serde_json::from_slice(&std::fs::read(&args.config).map_err(|e| format!("config: {e}"))?)
            .map_err(|e| format!("config: {e}"))?;
    if config["format"] != "combraton-conformance-config/1" {
        return Err("config format must be combraton-conformance-config/1".into());
    }
    std::fs::create_dir_all(&args.data_dir).map_err(|e| e.to_string())?;
    let mut store = store::Store::open(&args.data_dir, mutant_set.on("lose-dedupe-on-restart"))
        .map_err(|e| e.to_string())?;
    let advance = config["dedupe"]["advance_on_start"].as_i64().unwrap_or(0);
    let retain = config["dedupe"]["retain_generations"]
        .as_i64()
        .unwrap_or(1_000_000);
    store
        .apply_retention(advance, retain.max(1))
        .map_err(|e| e.to_string())?;
    let principal = config["principal"]
        .as_str()
        .unwrap_or("conformance-caller")
        .to_string();
    let limits = config_limits(&config);
    let validators = load_validators(&args.schemas)?;

    let lax = Laxness {
        duplicate_members: mutant_set.on("accept-duplicate-members"),
        numbers: mutant_set.on("lax-numbers"),
    };
    let skip_invalid = mutant_set.on("skip-invalid-frames");
    let process_notifications = mutant_set.on("process-notifications");
    let close_on_invalid_request = mutant_set.on("close-on-invalid-request");
    let mut reader = FrameReader::new(std::io::stdin().lock());
    reader.unbounded = mutant_set.on("unbounded-frames");
    reader.parse_unterminated = mutant_set.on("parse-unterminated");
    reader.off_by_one = mutant_set.on("strict-off-by-one-limit");
    let mut provider = Provider::new(store, mutant_set, principal, limits, validators);
    let mut out = std::io::stdout().lock();

    let mut send = |value: &Value| -> bool {
        out.write_all(&json::encode_frame(value))
            .and_then(|()| out.flush())
            .is_ok()
    };

    loop {
        if provider.negotiated() {
            reader.limit = provider.frame_limit();
        }
        let frame = match reader.next().map_err(|e| e.to_string())? {
            Next::End => return Ok(()),
            Next::TooLarge => {
                send(&error_response(Value::Null, "frame_too_large", json!({})));
                return Ok(());
            }
            Next::Frame(frame) => frame,
        };
        if json::is_blank(&frame) {
            continue;
        }
        let message = match json::parse_frame(&frame, lax) {
            Ok(message) => message,
            Err(fault) => {
                let code = if fault == FrameFault::InvalidUtf8 {
                    "invalid_utf8"
                } else {
                    "parse_error"
                };
                if !send(&error_response(Value::Null, code, json!({}))) || !skip_invalid {
                    return Ok(());
                }
                continue;
            }
        };
        let Value::Object(object) = &message else {
            if !send(&error_response(
                Value::Null,
                "invalid_request",
                json!({"reason": "frame is not a JSON-RPC object"}),
            )) || close_on_invalid_request
            {
                return Ok(());
            }
            continue;
        };
        let Some(id) = object.get("id").cloned() else {
            // Notification: never processed or answered (STREAM section 3).
            if process_notifications
                && let (Some(method), Some(params)) = (
                    object.get("method").and_then(Value::as_str),
                    object.get("params"),
                )
            {
                provider.handle(Value::Null, method, params.clone());
            }
            continue;
        };
        let id_valid = match &id {
            Value::String(text) => (1..=128).contains(&text.len()),
            Value::Number(number) => number.is_i64(),
            _ => false,
        };
        let shape_valid = object.get("jsonrpc") == Some(&json!("2.0"))
            && object
                .get("method")
                .and_then(Value::as_str)
                .is_some_and(|m| (1..=128).contains(&m.len()))
            && object.get("params").is_some_and(Value::is_object)
            && object
                .keys()
                .all(|key| matches!(key.as_str(), "jsonrpc" | "id" | "method" | "params"));
        if !id_valid || !shape_valid {
            let reply_id = if id_valid { id } else { Value::Null };
            if !send(&error_response(
                reply_id,
                "invalid_request",
                json!({"reason": "not a valid JSON-RPC 2.0 request"}),
            )) || close_on_invalid_request
            {
                return Ok(());
            }
            continue;
        }
        let method = object["method"].as_str().unwrap_or_default().to_string();
        let response = provider.handle(id, &method, object["params"].clone());
        if !send(&response) {
            return Ok(());
        }
    }
}

fn main() -> ExitCode {
    match parse_args() {
        Ok(None) => ExitCode::SUCCESS,
        Ok(Some(args)) => match run(args) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("combraton-reference-provider: {error}");
                ExitCode::from(2)
            }
        },
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

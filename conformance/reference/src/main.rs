//! Reference Core provider for Combraton Protocol conformance (draft 0.1).
//!
//! Not a product. It speaks the stdio form of the local stream binding and
//! implements core/1 plus the conformance-only core-test/1 profile.

mod barriers;
mod clock;
mod execution;
mod features;
mod frames;
mod grants;
mod json;
mod mutants;
mod outbox;
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

const USAGE: &str = "usage: combraton-reference-provider --data-dir DIR --config FILE --schemas DIR [--socket PATH] [--mutant NAME]... | --list-mutants";

struct Args {
    socket: Option<PathBuf>,
    data_dir: PathBuf,
    config: PathBuf,
    schemas: PathBuf,
    mutants: Vec<String>,
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut data_dir = None;
    let mut config = None;
    let mut schemas = None;
    let mut socket = None;
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
            "--socket" => socket = args.next().map(PathBuf::from),
            "--mutant" => mutants.push(args.next().ok_or("--mutant needs a name")?),
            other => return Err(format!("unexpected argument {other}")),
        }
    }
    match (data_dir, config, schemas) {
        (Some(data_dir), Some(config), Some(schemas)) => Ok(Some(Args {
            socket,
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
        max_pending_notification_bytes: config["events"]["max_pending_notification_bytes"]
            .as_i64()
            .unwrap_or(8_388_608),
        backpressure_notice_ms: config["events"]["backpressure_notice_ms"]
            .as_i64()
            .unwrap_or(1000),
    }
}

/// Everything a session needs that is shared across connections of one process.
struct Shared {
    data_dir: PathBuf,
    mutant_names: Vec<String>,
    identity: provider::Identity,
    limits: Limits,
    validators: std::sync::Arc<HashMap<&'static str, jsonschema::Validator>>,
    credentials: std::sync::Arc<Vec<provider::Credential>>,
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
        .apply_retention(advance, retain.max(1), mutant_set.on("retain-off-by-one"))
        .map_err(|e| e.to_string())?;
    store
        .apply_event_config(
            config["events"]["new_epoch_on_start"]
                .as_bool()
                .unwrap_or(false),
            config["events"]["unvouched_last"].as_i64().unwrap_or(0),
            config["events"]["retain_last"].as_i64(),
            mutant_set.on("volatile-events"),
        )
        .map_err(|e| e.to_string())?;
    let principal = config["principal"]
        .as_str()
        .unwrap_or("conformance-caller")
        .to_string();
    let authorities = match config["authority_principals"].as_array() {
        Some(list) => list
            .iter()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect(),
        None => vec![principal.clone()],
    };
    let provider_id = config["provider_id"]
        .as_str()
        .unwrap_or("conformance-provider")
        .to_string();
    let clock = std::sync::Arc::new(clock::Clock::from_config(&config, &mutant_set)?);
    barriers::init(&config);
    let writes = config["capabilities"]["core-test.writes"]
        .as_str()
        .unwrap_or("supported");
    let capabilities = json!([{
        "name": "core-test.writes",
        "status": writes,
        "evidence": {"source": if writes == "supported" { "reference-store" } else { "launch-configuration" }},
    }]);
    store
        .apply_capabilities(
            &provider_id,
            &capabilities,
            &clock.now(),
            mutant_set.on("capability-revision-static"),
            (
                mutant_set.on("capability-event-wrong-revision"),
                mutant_set.on("capability-event-wrong-subject"),
            ),
        )
        .map_err(|e| e.to_string())?;
    execution::recover(
        &mut store,
        &execution::Recovery {
            now: clock.now(),
            executor: &config["executor"],
            mutants: &mutant_set,
            authorities: &authorities,
            journal_intact: !config["events"]["new_epoch_on_start"]
                .as_bool()
                .unwrap_or(false),
        },
    )
    .map_err(|e| e.to_string())?;
    let credentials: Vec<provider::Credential> = config["credentials"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let credential = entry["credential"].as_str()?;
            let principal = credential.split('.').nth(1)?.to_string();
            Some(provider::Credential {
                principal,
                digest: json::sha256_digest(credential.as_bytes()),
                revoked: entry["revoked"].as_bool().unwrap_or(false),
            })
        })
        .collect();
    let shared = Shared {
        data_dir: args.data_dir.clone(),
        mutant_names: args.mutants.clone(),
        identity: provider::Identity {
            provider_id,
            clock,
            executor: std::sync::Arc::new(config["executor"].clone()),
            capabilities,
            authorities,
            principal,
        },
        limits: config_limits(&config),
        validators: std::sync::Arc::new(load_validators(&args.schemas)?),
        credentials: std::sync::Arc::new(credentials),
    };
    match &args.socket {
        None => {
            let provider = Provider::new(
                store,
                mutant_set,
                shared.identity.clone(),
                shared.limits,
                shared.validators.clone(),
                false,
                shared.credentials.clone(),
            );
            serve(
                std::io::stdin().lock(),
                std::io::stdout(),
                None,
                provider,
                &shared,
                false,
            )
        }
        Some(socket) => {
            drop(store);
            serve_unix(socket, std::sync::Arc::new(shared))
        }
    }
}

/// Unix-socket form (STREAM section 6): checked directory, peer check, one thread per session.
/// The process lives until its standard input ends (conformance lifecycle).
fn serve_unix(socket: &std::path::Path, shared: std::sync::Arc<Shared>) -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::os::unix::io::AsRawFd;
    let mutant_set = mutants::Mutants::parse(&shared.mutant_names)?;
    let directory = socket.parent().ok_or("socket path has no directory")?;
    let metadata =
        std::fs::symlink_metadata(directory).map_err(|e| format!("socket directory: {e}"))?;
    // SAFETY: geteuid has no preconditions.
    let own_uid = unsafe { libc::geteuid() };
    let safe = metadata.is_dir()
        && !metadata.file_type().is_symlink()
        && metadata.uid() == own_uid
        && metadata.mode() & 0o077 == 0;
    if !safe && !mutant_set.on("unchecked-socket-directory") {
        return Err(format!(
            "unsafe socket directory {}: it must be a directory owned by this user with no group or other permissions",
            directory.display()
        ));
    }
    let _ = std::fs::remove_file(socket);
    let listener =
        std::os::unix::net::UnixListener::bind(socket).map_err(|e| format!("bind: {e}"))?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    std::thread::spawn(|| {
        let mut sink = Vec::new();
        let _ = std::io::Read::read_to_end(&mut std::io::stdin(), &mut sink);
        std::process::exit(0);
    });
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        if peer_uid(stream.as_raw_fd()) != Some(own_uid) && !mutant_set.on("skip-peer-check") {
            continue; // Different user: close without a frame.
        }
        let shared = shared.clone();
        std::thread::spawn(move || {
            let Ok(mutant_set) = mutants::Mutants::parse(&shared.mutant_names) else {
                return;
            };
            let Ok(store) =
                store::Store::open(&shared.data_dir, mutant_set.on("lose-dedupe-on-restart"))
            else {
                return;
            };
            let (Ok(writer), Ok(closer)) = (stream.try_clone(), stream.try_clone()) else {
                return;
            };
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(40)));
            let provider = Provider::new(
                store,
                mutant_set,
                shared.identity.clone(),
                shared.limits,
                shared.validators.clone(),
                true,
                shared.credentials.clone(),
            );
            let _ = serve(stream, writer, Some(closer), provider, &shared, true);
            barriers::signal(barriers::SESSION_CLOSED);
        });
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn peer_uid(fd: std::os::unix::io::RawFd) -> Option<u32> {
    let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: the buffer and length describe a valid ucred for SO_PEERCRED.
    let status = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credentials as *mut libc::ucred).cast(),
            &mut length,
        )
    };
    (status == 0).then_some(credentials.uid)
}

#[cfg(not(target_os = "linux"))]
fn peer_uid(fd: std::os::unix::io::RawFd) -> Option<u32> {
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;
    // SAFETY: getpeereid writes two integers through valid pointers.
    let status = unsafe { libc::getpeereid(fd, &mut uid, &mut gid) };
    (status == 0).then_some(uid)
}

/// How a session ended.
enum Ended {
    /// Input ended or the connection failed: flush what is queued.
    Normally,
    /// Pending output exceeded its bound (CORE section 16.5): the connection is already closed.
    Backpressure,
}

/// One session over one connection. Output goes through a bounded outbox written by its own
/// thread, so a consumer that stops reading loses its connection instead of stalling the provider.
fn serve<R: std::io::Read, W: Write + Send + 'static>(
    input: R,
    out: W,
    closer: Option<std::os::unix::net::UnixStream>,
    provider: Provider,
    shared: &Shared,
    poll: bool,
) -> Result<(), String> {
    let outbox = outbox::Outbox::start(out);
    let result = serve_frames(input, &outbox, closer.as_ref(), provider, shared, poll);
    if let Ok(Ended::Normally) = result {
        let queued = outbox.queued();
        outbox.wait_written(queued, Some(std::time::Duration::from_secs(30)));
    }
    outbox.close();
    result.map(|_| ())
}

/// End a connection whose pending output exceeded its bound (CORE section 16.5, TRN-4).
fn close_for_backpressure(
    provider: &mut Provider,
    outbox: &outbox::Outbox,
    closer: Option<&std::os::unix::net::UnixStream>,
    mutant_set: &mutants::Mutants,
) {
    barriers::signal(barriers::BACKPRESSURE_LIMIT_REACHED);
    if provider.backpressure_negotiated() || mutant_set.on("consumer-too-slow-to-older-consumers") {
        // The ending notice is attempted within the declared bound, never waited for forever.
        for notice in provider.end_subscriptions("consumer_too_slow") {
            outbox.push(json::encode_frame(&notice));
        }
        let bound = if mutant_set.on("notice-waits-indefinitely") {
            None
        } else {
            Some(std::time::Duration::from_millis(
                provider.backpressure_notice_ms(),
            ))
        };
        outbox.wait_written(outbox.queued(), bound);
    }
    // Closure: nothing more is written, whether or not the notice was.
    outbox.abandon();
    if let Some(stream) = closer {
        let _ = stream.shutdown(std::net::Shutdown::Both);
    }
    barriers::signal(barriers::BACKPRESSURE_CONNECTION_CLOSED);
}

fn serve_frames<R: std::io::Read>(
    input: R,
    outbox: &outbox::Outbox,
    closer: Option<&std::os::unix::net::UnixStream>,
    mut provider: Provider,
    shared: &Shared,
    poll: bool,
) -> Result<Ended, String> {
    let mutant_set = mutants::Mutants::parse(&shared.mutant_names)?;
    let lax = Laxness {
        duplicate_members: mutant_set.on("accept-duplicate-members"),
        numbers: mutant_set.on("lax-numbers"),
        surrogates: mutant_set.on("accept-lone-surrogates"),
        noncharacters: mutant_set.on("accept-noncharacters"),
    };
    let skip_invalid = mutant_set.on("skip-invalid-frames");
    let process_notifications = mutant_set.on("process-notifications");
    let close_on_invalid_request = mutant_set.on("close-on-invalid-request");
    let ignore_idless_garbage = mutant_set.on("ignore-idless-garbage");
    let lenient_shape = mutant_set.on("lenient-jsonrpc-shape");
    let exit_nonzero = mutant_set.on("exit-nonzero-at-end-of-input");
    let frame_limit_fixed = mutant_set.on("frame-limit-never-raised");
    let cross_session = !mutant_set.on("no-cross-session-delivery");
    let notify_first = mutant_set.on("notify-before-response");
    let mut reader = FrameReader::new(input);
    reader.unbounded = mutant_set.on("unbounded-frames");
    reader.parse_unterminated = mutant_set.on("parse-unterminated");
    reader.off_by_one = mutant_set.on("strict-off-by-one-limit");

    let bound = provider.pending_output_bound();
    let stall = std::time::Duration::from_millis(provider.backpressure_notice_ms());
    let unbounded = mutant_set.on("unbounded-pending-output");
    let drop_under_pressure = mutant_set.on("semantic-events-dropped-under-pressure");
    // A consumer that makes no room within the bound is too slow: close its connection.
    let make_room = |provider: &mut Provider| -> Result<(), Ended> {
        if outbox.wait_drained(stall) {
            Ok(())
        } else {
            close_for_backpressure(provider, outbox, closer, &mutant_set);
            Err(Ended::Backpressure)
        }
    };
    // Queue a response; `Err` means the connection must end now.
    let respond = |provider: &mut Provider, value: &Value| -> Result<(), Ended> {
        let frame = json::encode_frame(value);
        let pending = outbox.pending();
        if pending > 0 && pending + frame.len() > bound && !unbounded {
            make_room(provider)?;
        }
        if outbox.push(frame) {
            Ok(())
        } else {
            Err(Ended::Normally)
        }
    };
    // Produce and queue owed notifications without exceeding the bound (CORE section 16.5).
    let deliver = |provider: &mut Provider, idle: bool| -> Result<(), Ended> {
        loop {
            let pending = outbox.pending();
            let room = if unbounded || drop_under_pressure {
                None
            } else {
                Some((bound.saturating_sub(pending), pending == 0))
            };
            let (frames, withheld) = provider.drain_notifications(idle, room);
            for frame in frames {
                let bytes = json::encode_frame(&frame);
                let pending = outbox.pending();
                if drop_under_pressure && pending > 0 && pending + bytes.len() > bound {
                    barriers::signal(barriers::BACKPRESSURE_LIMIT_REACHED);
                    continue;
                }
                if !outbox.push(bytes) {
                    return Err(Ended::Normally);
                }
            }
            if !withheld {
                return Ok(());
            }
            make_room(provider)?;
        }
    };
    macro_rules! send {
        ($value:expr) => {
            respond(&mut provider, &$value).is_ok()
        };
    }
    macro_rules! notify_all {
        ($idle:expr) => {
            if let Err(ended) = deliver(&mut provider, $idle) {
                return Ok(ended);
            }
        };
    }

    loop {
        if provider.negotiated() && !frame_limit_fixed {
            reader.limit = provider.frame_limit();
        }
        let next = match reader.next() {
            Ok(next) => next,
            Err(error)
                if poll
                    && matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
            {
                if cross_session {
                    notify_all!(true);
                }
                continue;
            }
            Err(error) => return Err(error.to_string()),
        };
        let frame = match next {
            Next::End => {
                if exit_nonzero && !poll {
                    let queued = outbox.queued();
                    outbox.wait_written(queued, Some(std::time::Duration::from_secs(30)));
                    std::process::exit(3);
                }
                return Ok(Ended::Normally);
            }
            Next::TooLarge => {
                let _ = send!(error_response(Value::Null, "frame_too_large", json!({})));
                return Ok(Ended::Normally);
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
                if !send!(error_response(Value::Null, code, json!({}))) || !skip_invalid {
                    return Ok(Ended::Normally);
                }
                continue;
            }
        };
        let Value::Object(object) = &message else {
            if !send!(error_response(
                Value::Null,
                "invalid_request",
                json!({"reason": "frame is not a JSON-RPC object"}),
            )) || close_on_invalid_request
            {
                return Ok(Ended::Normally);
            }
            continue;
        };
        let Some(id) = object.get("id").cloned() else {
            // Notification: never processed or answered (STREAM section 3). Any other id-less
            // object is invalid_request with id null.
            let is_notification = object.get("jsonrpc") == Some(&json!("2.0"))
                && object.get("method").is_some_and(Value::is_string);
            if !is_notification && !ignore_idless_garbage {
                if !send!(error_response(Value::Null, "invalid_request", json!({})))
                    || close_on_invalid_request
                {
                    return Ok(Ended::Normally);
                }
                continue;
            }
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
            Value::String(text) => {
                (1..=128).contains(&text.chars().count()) || (lenient_shape && !text.is_empty())
            }
            Value::Number(number) => number.is_i64(),
            Value::Bool(_) => lenient_shape,
            _ => false,
        };
        let shape_valid = object.get("jsonrpc") == Some(&json!("2.0"))
            && object
                .get("method")
                .and_then(Value::as_str)
                .is_some_and(|m| (1..=128).contains(&m.len()))
            && object.get("params").is_some_and(Value::is_object)
            && (lenient_shape
                || object
                    .keys()
                    .all(|key| matches!(key.as_str(), "jsonrpc" | "id" | "method" | "params")));
        if !id_valid || !shape_valid {
            let reply_id = if id_valid { id } else { Value::Null };
            if !send!(error_response(
                reply_id,
                "invalid_request",
                json!({"reason": "not a valid JSON-RPC 2.0 request"}),
            )) || close_on_invalid_request
            {
                return Ok(Ended::Normally);
            }
            continue;
        }
        let method = object["method"].as_str().unwrap_or_default().to_string();
        let response = provider.handle(id, &method, object["params"].clone());
        if notify_first {
            notify_all!(false);
        }
        if let Err(ended) = respond(&mut provider, &response) {
            return Ok(ended);
        }
        notify_all!(false);
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

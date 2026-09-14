//! Implementation-specific barriers and signals for deterministic interleavings (decision 007).
//!
//! Inert unless launch configuration names `test_barriers`. A barrier pauses once, at its first
//! hit, until the runner creates `<name>.release`; a real-time watchdog continues after 30 s so a
//! vanished runner cannot leave the provider paused forever.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const RECHECK_AFTER_AUTHORIZATION: &str = "subscription.recheck.after_authorization";
pub const LOCK_CONTENDED: &str = "processing.lock.contended";
const WATCHDOG: Duration = Duration::from_secs(30);

struct Barriers {
    directory: PathBuf,
    enabled: BTreeSet<String>,
    used: Mutex<BTreeSet<String>>,
}

static BARRIERS: OnceLock<Barriers> = OnceLock::new();

pub fn init(config: &serde_json::Value) {
    let Some(directory) = config["test_barriers"]["directory"].as_str() else {
        return;
    };
    let enabled = config["test_barriers"]["enabled"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(String::from)
        .collect();
    let _ = BARRIERS.set(Barriers {
        directory: PathBuf::from(directory),
        enabled,
        used: Mutex::new(BTreeSet::new()),
    });
}

fn touch(path: &Path) {
    let temporary = path.with_extension("tmp");
    if std::fs::write(&temporary, b"").is_ok() {
        let _ = std::fs::rename(&temporary, path);
    }
}

/// Mark a point without pausing.
pub fn signal(name: &str) {
    if let Some(barriers) = BARRIERS.get() {
        touch(&barriers.directory.join(format!("{name}.signal")));
    }
}

/// Pause at an enabled barrier the first time it is reached.
pub fn pause(name: &str) {
    let Some(barriers) = BARRIERS.get() else {
        return;
    };
    if !barriers.enabled.contains(name) {
        return;
    }
    {
        let mut used = barriers
            .used
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !used.insert(name.to_string()) {
            return;
        }
    }
    touch(&barriers.directory.join(format!("{name}.reached")));
    let release = barriers.directory.join(format!("{name}.release"));
    let started = Instant::now();
    while !release.exists() {
        if started.elapsed() > WATCHDOG {
            eprintln!(
                "barrier {name}: no release within {} s; continuing",
                WATCHDOG.as_secs()
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

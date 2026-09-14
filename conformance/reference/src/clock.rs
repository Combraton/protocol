//! Provider clock (decision 007). Protocol-visible time comes from here; real-time watchdogs
//! and the idle re-check interval never do.

use std::path::PathBuf;
use std::sync::Mutex;

pub enum Clock {
    /// The system clock (no conformance clock configured).
    System,
    /// `clock.fixed`: one instant for the whole process.
    Fixed(String),
    /// `clock.file`: an instant the runner replaces atomically during a session.
    File {
        path: PathBuf,
        last: Mutex<String>,
        /// Mutant switches.
        ignore_updates: bool,
        follow_backward: bool,
        reset_on_malformed: bool,
        initial: String,
    },
}

/// Whether `text` is an instant in the fixed `YYYY-MM-DDTHH:MM:SSZ` form.
pub fn is_instant(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 20
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 => *byte == b'-',
            10 => *byte == b'T',
            13 | 16 => *byte == b':',
            19 => *byte == b'Z',
            _ => byte.is_ascii_digit(),
        })
}

fn read_instant(path: &PathBuf) -> Result<String, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let trimmed = text.trim();
    if is_instant(trimmed) {
        Ok(trimmed.to_string())
    } else {
        Err(format!("malformed instant in {}", path.display()))
    }
}

impl Clock {
    /// Build from launch configuration. A missing or malformed clock file refuses the launch.
    pub fn from_config(
        config: &serde_json::Value,
        mutants: &crate::mutants::Mutants,
    ) -> Result<Self, String> {
        if let Some(path) = config["clock"]["file"].as_str() {
            let path = PathBuf::from(path);
            let initial = match read_instant(&path) {
                Ok(instant) => instant,
                Err(_) if mutants.on("clock-file-start-unchecked") => {
                    "1970-01-01T00:00:00Z".to_string()
                }
                Err(error) => return Err(format!("clock file: {error}")),
            };
            return Ok(Clock::File {
                path,
                last: Mutex::new(initial.clone()),
                ignore_updates: mutants.on("clock-file-ignored"),
                follow_backward: mutants.on("clock-follows-backward-time"),
                reset_on_malformed: mutants.on("clock-malformed-resets"),
                initial,
            });
        }
        Ok(match config["clock"]["fixed"].as_str() {
            Some(fixed) => Clock::Fixed(fixed.to_string()),
            None => Clock::System,
        })
    }

    pub fn now(&self) -> String {
        match self {
            Clock::System => crate::grants::now(None),
            Clock::Fixed(instant) => instant.clone(),
            Clock::File {
                path,
                last,
                ignore_updates,
                follow_backward,
                reset_on_malformed,
                initial,
            } => {
                let mut last = last
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if *ignore_updates {
                    return initial.clone();
                }
                match read_instant(path) {
                    Ok(instant) if instant >= *last || *follow_backward => *last = instant,
                    Ok(instant) => eprintln!(
                        "clock: ignoring backward instant {instant}; virtual time stays at {}",
                        *last
                    ),
                    Err(_) if *reset_on_malformed => *last = initial.clone(),
                    Err(error) => eprintln!("clock: {error}; keeping {}", *last),
                }
                last.clone()
            }
        }
    }
}

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

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year.rem_euclid(400);
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Seconds since the Unix epoch for an instant in `YYYY-MM-DDTHH:MM:SSZ` form.
pub fn to_seconds(instant: &str) -> i64 {
    let number = |range: std::ops::Range<usize>| {
        instant
            .get(range)
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
    };
    days_from_civil(number(0..4), number(5..7), number(8..10)) * 86_400
        + number(11..13) * 3_600
        + number(14..16) * 60
        + number(17..19)
}

/// The instant `seconds` after the Unix epoch.
pub fn from_seconds(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let rem = seconds.rem_euclid(86_400);
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
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instant_arithmetic_round_trips() {
        for instant in [
            "1970-01-01T00:00:00Z",
            "2030-01-01T00:00:00Z",
            "2030-02-28T23:59:59Z",
            "2032-02-29T12:00:00Z",
        ] {
            assert_eq!(from_seconds(to_seconds(instant)), instant);
        }
        assert_eq!(
            from_seconds(to_seconds("2030-01-01T00:00:00Z") + 3_600),
            "2030-01-01T01:00:00Z"
        );
        assert!(is_instant("2030-01-01T00:00:00Z"));
        assert!(!is_instant("2030-01-01 00:00:00"));
    }
}

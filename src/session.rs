use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use bevy::prelude::Resource;
use sense_telemetry::TelemetryDb;
use sense_types::{
    AccelerationConfig, ConfigurationRecord, DisplayConfig, InputIntegrityReport, SessionRecord,
    ValidationResult,
};

use crate::{
    camera_ctrl::LiveInputStats,
    config::{ExperimentSettings, TelemetryBuffers, ValidationState},
    input_plugin::InputIntegrityTracker,
};

const APP_VERSION: &str = "0.1.0";
const EXPERIMENT_ID: &str = "validation_lab";
const EXPERIMENT_VERSION: &str = "0.1.0";
const DATABASE_PATH: &str = "data/sense_maxer.db";

#[derive(Resource, Debug, Default)]
pub struct ValidationSession {
    pub active_session_id: Option<String>,
    pub last_result: Option<ValidationResult>,
    pub status_message: Option<String>,
}

pub fn next_session_id(date: &str, seq: u32) -> String {
    format!("session_{date}_{seq:06}")
}

pub fn next_config_id(seq: u32) -> String {
    format!("config_{seq:06}")
}

pub fn database_path() -> PathBuf {
    PathBuf::from(DATABASE_PATH)
}

pub fn validation_result(
    sensitivity: f64,
    observed_net_counts: i64,
    observed_abs_path_counts: u64,
    integrity: InputIntegrityReport,
) -> ValidationResult {
    let expected_counts = sense_math::counts_per_360(sensitivity);
    let observed_net_counts = observed_net_counts as f64;
    let count_difference = observed_net_counts - expected_counts;

    ValidationResult {
        expected_counts,
        observed_net_counts,
        observed_abs_path_counts: observed_abs_path_counts as f64,
        expected_degrees: 360.0,
        observed_degrees: sense_math::yaw_delta_deg(observed_net_counts, sensitivity),
        count_difference,
        error_percent: (count_difference / expected_counts) * 100.0,
        integrity,
    }
}

pub fn start_validation(
    settings: &ExperimentSettings,
    width: u32,
    height: u32,
    refresh_hz: Option<f64>,
    session: &mut ValidationSession,
    validation: &mut ValidationState,
    live: &mut LiveInputStats,
    buffers: &mut TelemetryBuffers,
    integrity: &InputIntegrityTracker,
) -> Result<(), String> {
    if validation.is_running() {
        return Err("A validation session is already running.".into());
    }

    let db = open_database()?;
    let now_ms = unix_time_ms()?;
    let date = utc_date_from_unix_ms(now_ms);
    let config_sequence = next_sequence(
        &db,
        "SELECT id FROM configurations WHERE id LIKE 'config_%'",
        "config_",
    )?;
    let session_prefix = format!("session_{date}_");
    let session_sequence = next_sequence(
        &db,
        "SELECT id FROM sessions WHERE id LIKE ?1",
        &session_prefix,
    )?;
    let config_id = next_config_id(config_sequence);
    let session_id = next_session_id(&date, session_sequence);
    let sensitivity = settings.sensitivity_config();
    let configuration = ConfigurationRecord {
        id: config_id.clone(),
        edpi: sense_math::edpi(sensitivity.dpi, sensitivity.sensitivity),
        sensitivity,
        display: DisplayConfig {
            width,
            height,
            refresh_hz: refresh_hz.unwrap_or(0.0),
        },
        accel: AccelerationConfig {
            enabled: false,
            model: "none".into(),
        },
        polling_rate_hz: None,
    };
    let record = SessionRecord {
        id: session_id.clone(),
        app_version: APP_VERSION.into(),
        experiment_id: EXPERIMENT_ID.into(),
        experiment_version: EXPERIMENT_VERSION.into(),
        config_id,
        seed: 0,
        start_unix_ms: now_ms,
        end_unix_ms: None,
    };

    db.upsert_configuration(&configuration)?;
    db.insert_session_start(&record)?;

    integrity
        .0
        .lock()
        .map_err(|_| "input integrity tracker lock is poisoned".to_string())?
        .reset();
    buffers.0.clear();
    reset_counters(live);
    session.active_session_id = Some(session_id.clone());
    session.last_result = None;
    session.status_message = Some(format!("Validation running: {session_id}"));
    *validation = ValidationState::Running;
    Ok(())
}

pub fn end_validation(
    sensitivity: f64,
    session: &mut ValidationSession,
    validation: &mut ValidationState,
    live: &LiveInputStats,
    buffers: &TelemetryBuffers,
    integrity: &InputIntegrityTracker,
) -> Result<(), String> {
    if !validation.is_running() {
        return Err("No validation session is running.".into());
    }
    let session_id = session
        .active_session_id
        .as_deref()
        .ok_or_else(|| "Running validation has no active session ID.".to_string())?;
    let integrity_report = integrity
        .0
        .lock()
        .map_err(|_| "input integrity tracker lock is poisoned".to_string())?
        .report();
    let result = validation_result(sensitivity, live.net_dx, live.abs_dx, integrity_report);
    let db = open_database()?;

    db.flush_buffers(session_id, &buffers.0)?;
    db.insert_validation_result(session_id, &result)?;
    db.end_session(session_id, unix_time_ms()?)?;

    let completed_session_id = session_id.to_string();
    session.active_session_id = None;
    session.last_result = Some(result);
    session.status_message = Some(format!("Validation completed: {completed_session_id}"));
    *validation = ValidationState::Idle;
    Ok(())
}

pub fn reset_counters(live: &mut LiveInputStats) {
    live.net_dx = 0;
    live.net_dy = 0;
    live.abs_dx = 0;
    live.total_yaw_delta_deg = 0.0;
    live.samples_this_frame = 0;
}

fn open_database() -> Result<TelemetryDb, String> {
    let path = database_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let db = TelemetryDb::open(Path::new(&path))?;
    let schema_exists: bool = db
        .connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='sessions')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if !schema_exists {
        db.migrate()?;
    }
    Ok(db)
}

fn next_sequence(db: &TelemetryDb, sql: &str, prefix: &str) -> Result<u32, String> {
    let like_pattern = format!("{prefix}%");
    let mut statement = db
        .connection
        .prepare(sql)
        .map_err(|error| error.to_string())?;
    let mut rows = if sql.contains("?1") {
        statement
            .query([like_pattern])
            .map_err(|error| error.to_string())?
    } else {
        statement.query([]).map_err(|error| error.to_string())?
    };
    let mut highest = 0_u32;
    while let Some(row) = rows.next().map_err(|error| error.to_string())? {
        let id: String = row.get(0).map_err(|error| error.to_string())?;
        if let Some(sequence) = id
            .strip_prefix(prefix)
            .and_then(|value| value.parse::<u32>().ok())
        {
            highest = highest.max(sequence);
        }
    }
    highest
        .checked_add(1)
        .ok_or_else(|| format!("ID sequence exhausted for {prefix}"))
}

fn unix_time_ms() -> Result<i64, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    i64::try_from(millis).map_err(|_| "system time exceeds i64 milliseconds".into())
}

fn utc_date_from_unix_ms(unix_ms: i64) -> String {
    let days = unix_ms.div_euclid(86_400_000);
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!("{year:04}{month:02}{day:02}")
}

#[cfg(test)]
mod tests {
    use sense_types::InputIntegrityReport;

    use super::{next_config_id, next_session_id, utc_date_from_unix_ms, validation_result};

    #[test]
    fn ids_are_stable_and_zero_padded() {
        assert_eq!(next_session_id("20260917", 1), "session_20260917_000001");
        assert_eq!(next_config_id(42), "config_000042");
    }

    #[test]
    fn validation_uses_signed_net_and_separate_absolute_path() {
        let result = validation_result(
            0.5,
            -10,
            14,
            InputIntegrityReport {
                samples_received: 2,
                sequence_gaps: 0,
                duplicate_sequences: 0,
                out_of_order_samples: 0,
                timestamp_regressions: 0,
            },
        );

        assert_eq!(result.expected_counts, 360.0 / 0.035);
        assert_eq!(result.observed_net_counts, -10.0);
        assert_eq!(result.observed_abs_path_counts, 14.0);
        assert!((result.observed_degrees - -0.35).abs() < f64::EPSILON);
        assert_eq!(result.count_difference, -10.0 - result.expected_counts);
        assert_eq!(
            result.error_percent,
            (result.count_difference / result.expected_counts) * 100.0
        );
    }

    #[test]
    fn utc_date_is_derived_without_locale() {
        assert_eq!(utc_date_from_unix_ms(0), "19700101");
        assert_eq!(utc_date_from_unix_ms(1_788_998_400_000), "20260910");
    }
}

use std::path::Path;

use rusqlite::{params, Connection};
use sense_types::{ConfigurationRecord, FovAxis, SessionRecord, ValidationResult};

use crate::SessionBuffers;

pub struct TelemetryDb {
    pub connection: Connection,
}

impl TelemetryDb {
    pub fn open(path: &Path) -> Result<Self, String> {
        Connection::open(path)
            .map(|connection| Self { connection })
            .map_err(|error| error.to_string())
    }

    pub fn migrate(&self) -> Result<(), String> {
        let has_m1_schema = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'configurations')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|error| error.to_string())?;

        if !has_m1_schema {
            self.connection
                .execute_batch(
                    r#"
CREATE TABLE configurations (
  id TEXT PRIMARY KEY,
  dpi REAL NOT NULL,
  sensitivity REAL NOT NULL,
  edpi REAL NOT NULL,
  yaw_deg_per_count_at_sens_1 REAL NOT NULL,
  fov_axis TEXT NOT NULL,
  fov_degrees REAL NOT NULL,
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  refresh_hz REAL NOT NULL,
  acceleration_enabled INTEGER NOT NULL,
  acceleration_model TEXT NOT NULL,
  polling_rate_hz REAL,
  snapshot_json TEXT NOT NULL
);

CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  configuration_id TEXT NOT NULL,
  app_version TEXT NOT NULL,
  experiment_id TEXT NOT NULL,
  experiment_version TEXT NOT NULL,
  random_seed INTEGER NOT NULL,
  start_unix_ms INTEGER NOT NULL,
  end_unix_ms INTEGER,
  FOREIGN KEY(configuration_id) REFERENCES configurations(id)
);

CREATE TABLE raw_mouse_events (
  session_id TEXT NOT NULL,
  sequence_number INTEGER NOT NULL,
  timestamp_ns INTEGER NOT NULL,
  dx INTEGER NOT NULL,
  dy INTEGER NOT NULL,
  buttons INTEGER NOT NULL,
  PRIMARY KEY(session_id, sequence_number)
);

CREATE TABLE input_camera_samples (
  session_id TEXT NOT NULL,
  sequence_number INTEGER NOT NULL,
  timestamp_ns INTEGER NOT NULL,
  yaw_deg REAL NOT NULL,
  pitch_deg REAL NOT NULL,
  PRIMARY KEY(session_id, sequence_number)
);

CREATE TABLE frame_samples (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  timestamp_ns INTEGER NOT NULL,
  frame_time_s REAL NOT NULL,
  fps REAL NOT NULL
);

CREATE TABLE validation_results (
  session_id TEXT PRIMARY KEY,
  expected_counts REAL NOT NULL,
  observed_net_counts REAL NOT NULL,
  observed_abs_path_counts REAL NOT NULL,
  expected_degrees REAL NOT NULL,
  observed_degrees REAL NOT NULL,
  count_difference REAL NOT NULL,
  error_percent REAL NOT NULL,
  samples_received INTEGER NOT NULL,
  sequence_gaps INTEGER NOT NULL,
  duplicate_sequences INTEGER NOT NULL,
  out_of_order_samples INTEGER NOT NULL,
  timestamp_regressions INTEGER NOT NULL,
  pipeline_suspect INTEGER NOT NULL
);

CREATE INDEX idx_raw_mouse_session ON raw_mouse_events(session_id);
CREATE INDEX idx_input_cam_session ON input_camera_samples(session_id);
CREATE INDEX idx_frame_session ON frame_samples(session_id);
"#,
                )
                .map_err(|error| error.to_string())?;
        }

        self.connection
            .execute_batch(
                r#"
CREATE TABLE IF NOT EXISTS processed_mouse_events (
  session_id TEXT NOT NULL,
  sequence_number INTEGER NOT NULL,
  timestamp_ns INTEGER NOT NULL,
  processed_dx REAL NOT NULL,
  processed_dy REAL NOT NULL,
  processor_id TEXT NOT NULL,
  processor_version TEXT NOT NULL,
  processor_config_json TEXT NOT NULL,
  PRIMARY KEY(session_id, sequence_number)
);
CREATE INDEX IF NOT EXISTS idx_processed_mouse_session
  ON processed_mouse_events(session_id);
"#,
            )
            .map_err(|error| error.to_string())
    }

    pub fn upsert_configuration(&self, cfg: &ConfigurationRecord) -> Result<(), String> {
        let fov_axis = match cfg.sensitivity.fov_axis {
            FovAxis::Horizontal => "Horizontal",
        };
        let snapshot_json = serde_json::to_string(cfg).map_err(|error| error.to_string())?;

        self.connection
            .execute(
                r#"
INSERT INTO configurations (
  id, dpi, sensitivity, edpi, yaw_deg_per_count_at_sens_1, fov_axis,
  fov_degrees, width, height, refresh_hz, acceleration_enabled,
  acceleration_model, polling_rate_hz, snapshot_json
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(id) DO UPDATE SET
  dpi = excluded.dpi,
  sensitivity = excluded.sensitivity,
  edpi = excluded.edpi,
  yaw_deg_per_count_at_sens_1 = excluded.yaw_deg_per_count_at_sens_1,
  fov_axis = excluded.fov_axis,
  fov_degrees = excluded.fov_degrees,
  width = excluded.width,
  height = excluded.height,
  refresh_hz = excluded.refresh_hz,
  acceleration_enabled = excluded.acceleration_enabled,
  acceleration_model = excluded.acceleration_model,
  polling_rate_hz = excluded.polling_rate_hz,
  snapshot_json = excluded.snapshot_json
"#,
                params![
                    cfg.id,
                    cfg.sensitivity.dpi,
                    cfg.sensitivity.sensitivity,
                    cfg.edpi,
                    cfg.sensitivity.yaw_deg_per_count_at_sens_1,
                    fov_axis,
                    cfg.sensitivity.fov_degrees,
                    cfg.display.width,
                    cfg.display.height,
                    cfg.display.refresh_hz,
                    cfg.accel.enabled,
                    cfg.accel.model,
                    cfg.polling_rate_hz,
                    snapshot_json,
                ],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn insert_session_start(&self, session: &SessionRecord) -> Result<(), String> {
        self.connection
            .execute(
                r#"
INSERT INTO sessions (
  id, configuration_id, app_version, experiment_id, experiment_version,
  random_seed, start_unix_ms, end_unix_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
"#,
                params![
                    session.id,
                    session.config_id,
                    session.app_version,
                    session.experiment_id,
                    session.experiment_version,
                    as_i64(session.seed, "session seed")?,
                    session.start_unix_ms,
                    session.end_unix_ms,
                ],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn end_session(&self, session_id: &str, end_unix_ms: i64) -> Result<(), String> {
        self.connection
            .execute(
                "UPDATE sessions SET end_unix_ms = ? WHERE id = ?",
                params![end_unix_ms, session_id],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn insert_validation_result(
        &self,
        session_id: &str,
        result: &ValidationResult,
    ) -> Result<(), String> {
        write_validation_result(&self.connection, session_id, result)
    }

    pub fn flush_buffers(&self, session_id: &str, buffers: &SessionBuffers) -> Result<(), String> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| error.to_string())?;
        write_buffers(&transaction, session_id, buffers)?;
        transaction.commit().map_err(|error| error.to_string())
    }

    pub fn complete_validation(
        &self,
        session_id: &str,
        buffers: &SessionBuffers,
        result: &ValidationResult,
        end_unix_ms: i64,
    ) -> Result<(), String> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| error.to_string())?;
        write_buffers(&transaction, session_id, buffers)?;
        write_validation_result(&transaction, session_id, result)?;
        transaction
            .execute(
                "UPDATE sessions SET end_unix_ms = ? WHERE id = ?",
                params![end_unix_ms, session_id],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())
    }
}

fn write_validation_result(
    connection: &Connection,
    session_id: &str,
    result: &ValidationResult,
) -> Result<(), String> {
    connection
        .execute(
            r#"
INSERT INTO validation_results (
  session_id, expected_counts, observed_net_counts, observed_abs_path_counts,
  expected_degrees, observed_degrees, count_difference, error_percent,
  samples_received, sequence_gaps, duplicate_sequences, out_of_order_samples,
  timestamp_regressions, pipeline_suspect
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
"#,
            params![
                session_id,
                result.expected_counts,
                result.observed_net_counts,
                result.observed_abs_path_counts,
                result.expected_degrees,
                result.observed_degrees,
                result.count_difference,
                result.error_percent,
                as_i64(result.integrity.samples_received, "samples received")?,
                as_i64(result.integrity.sequence_gaps, "sequence gaps")?,
                as_i64(result.integrity.duplicate_sequences, "duplicate sequences")?,
                as_i64(
                    result.integrity.out_of_order_samples,
                    "out-of-order samples"
                )?,
                as_i64(
                    result.integrity.timestamp_regressions,
                    "timestamp regressions"
                )?,
                result.integrity.is_pipeline_suspect(),
            ],
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn write_buffers(
    connection: &Connection,
    session_id: &str,
    buffers: &SessionBuffers,
) -> Result<(), String> {
    {
        let mut statement = connection
            .prepare(
                r#"
INSERT INTO raw_mouse_events (
  session_id, sequence_number, timestamp_ns, dx, dy, buttons
) VALUES (?, ?, ?, ?, ?, ?)
"#,
            )
            .map_err(|error| error.to_string())?;
        for sample in &buffers.mouse {
            statement
                .execute(params![
                    session_id,
                    as_i64(sample.sequence_number, "mouse sequence number")?,
                    as_i64(sample.timestamp_ns, "mouse timestamp")?,
                    sample.dx,
                    sample.dy,
                    sample.buttons,
                ])
                .map_err(|error| error.to_string())?;
        }
    }

    {
        let mut statement = connection
            .prepare(
                r#"
INSERT INTO processed_mouse_events (
  session_id, sequence_number, timestamp_ns, processed_dx, processed_dy,
  processor_id, processor_version, processor_config_json
) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
"#,
            )
            .map_err(|error| error.to_string())?;
        for sample in &buffers.processed {
            statement
                .execute(params![
                    session_id,
                    as_i64(sample.sequence_number, "processed mouse sequence number")?,
                    as_i64(sample.timestamp_ns, "processed mouse timestamp")?,
                    sample.processed_dx,
                    sample.processed_dy,
                    sample.processor_id,
                    sample.processor_version,
                    sample.processor_config_json,
                ])
                .map_err(|error| error.to_string())?;
        }
    }

    {
        let mut statement = connection
            .prepare(
                r#"
INSERT INTO input_camera_samples (
  session_id, sequence_number, timestamp_ns, yaw_deg, pitch_deg
) VALUES (?, ?, ?, ?, ?)
"#,
            )
            .map_err(|error| error.to_string())?;
        for sample in &buffers.input_camera {
            statement
                .execute(params![
                    session_id,
                    as_i64(sample.sequence_number, "input camera sequence number")?,
                    as_i64(sample.timestamp_ns, "input camera timestamp")?,
                    sample.yaw_deg,
                    sample.pitch_deg,
                ])
                .map_err(|error| error.to_string())?;
        }
    }

    {
        let mut statement = connection
            .prepare(
                r#"
INSERT INTO frame_samples (
  session_id, timestamp_ns, frame_time_s, fps
) VALUES (?, ?, ?, ?)
"#,
            )
            .map_err(|error| error.to_string())?;
        for sample in &buffers.frames {
            statement
                .execute(params![
                    session_id,
                    as_i64(sample.timestamp_ns, "frame timestamp")?,
                    sample.frame_time_s,
                    sample.fps,
                ])
                .map_err(|error| error.to_string())?;
        }
    }

    Ok(())
}

fn as_i64(value: u64, field: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{field} exceeds SQLite INTEGER range"))
}

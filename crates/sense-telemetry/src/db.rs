use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Row};
use sense_types::{
    AimCameraSampleRecord, AimInputSampleRecord, AimShotRecord, AimTargetEventRecord,
    AimTrialRecord, ConfigurationRecord, FovAxis, SessionRecord, ValidationResult,
};

use crate::SessionBuffers;

pub struct TelemetryDb {
    pub connection: Connection,
}

#[derive(Debug, Clone)]
pub struct AimTrialSummary {
    pub id: String,
    pub trial_type: String,
    pub hits: u32,
    pub shots: u32,
    pub accuracy: f64,
    pub score_secs: f64,
    pub experiment_version: String,
    pub end_unix_ms: i64,
}

#[derive(Debug, Clone)]
pub struct AimTrialReplayBundle {
    pub trial: AimTrialRecord,
    pub target_events: Vec<AimTargetEventRecord>,
    pub shots: Vec<AimShotRecord>,
    pub camera_samples: Vec<AimCameraSampleRecord>,
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

CREATE TABLE IF NOT EXISTS aim_trials (
  id TEXT PRIMARY KEY,
  app_version TEXT NOT NULL,
  experiment_id TEXT NOT NULL,
  experiment_version TEXT NOT NULL,
  trial_type TEXT NOT NULL,
  status TEXT NOT NULL,
  processor_id TEXT NOT NULL,
  processor_version TEXT NOT NULL,
  processor_config_json TEXT NOT NULL,
  dpi REAL NOT NULL,
  sensitivity REAL NOT NULL,
  polling_rate_hz REAL NOT NULL,
  fov_degrees_h REAL NOT NULL,
  pitch_model_id TEXT NOT NULL DEFAULT '',
  pitch_model_version TEXT NOT NULL DEFAULT '',
  pitch_config_json TEXT NOT NULL DEFAULT '{}',
  resolution_width INTEGER NOT NULL DEFAULT 0,
  resolution_height INTEGER NOT NULL DEFAULT 0,
  aspect_ratio REAL NOT NULL DEFAULT 0,
  random_seed INTEGER NOT NULL DEFAULT 0,
  task_version TEXT NOT NULL DEFAULT '',
  hardware_config_json TEXT NOT NULL DEFAULT '{}',
  view_config_json TEXT NOT NULL DEFAULT '{}',
  task_config_json TEXT NOT NULL,
  metrics_json TEXT NOT NULL,
  start_unix_ms INTEGER NOT NULL,
  end_unix_ms INTEGER NOT NULL,
  start_timestamp_ns INTEGER NOT NULL,
  end_timestamp_ns INTEGER NOT NULL,
  duration_secs REAL NOT NULL,
  hits INTEGER NOT NULL,
  shots INTEGER NOT NULL,
  misses INTEGER NOT NULL,
  score_secs REAL NOT NULL,
  accuracy REAL NOT NULL
);

CREATE TABLE IF NOT EXISTS aim_shots (
  trial_id TEXT NOT NULL,
  shot_index INTEGER NOT NULL,
  timestamp_ns INTEGER NOT NULL,
  hit INTEGER NOT NULL,
  yaw_deg REAL NOT NULL,
  pitch_deg REAL NOT NULL,
  target_x REAL NOT NULL,
  target_y REAL NOT NULL,
  target_z REAL NOT NULL,
  target_radius REAL NOT NULL,
  target_id TEXT NOT NULL DEFAULT '',
  PRIMARY KEY(trial_id, shot_index),
  FOREIGN KEY(trial_id) REFERENCES aim_trials(id)
);

CREATE INDEX IF NOT EXISTS idx_aim_shots_trial ON aim_shots(trial_id);

CREATE TABLE IF NOT EXISTS aim_target_events (
  trial_id TEXT NOT NULL,
  target_id TEXT NOT NULL,
  event_index INTEGER NOT NULL,
  timestamp_ns INTEGER NOT NULL,
  event_type TEXT NOT NULL,
  position_x REAL NOT NULL,
  position_y REAL NOT NULL,
  position_z REAL NOT NULL,
  yaw_deg REAL,
  pitch_deg REAL,
  velocity_x REAL NOT NULL,
  velocity_y REAL NOT NULL,
  velocity_z REAL NOT NULL,
  event_data_json TEXT NOT NULL,
  PRIMARY KEY(trial_id, event_index),
  FOREIGN KEY(trial_id) REFERENCES aim_trials(id)
);
CREATE INDEX IF NOT EXISTS idx_aim_target_events_trial ON aim_target_events(trial_id);

CREATE TABLE IF NOT EXISTS aim_input_samples (
  trial_id TEXT NOT NULL,
  timestamp_ns INTEGER NOT NULL,
  sequence_number INTEGER NOT NULL,
  raw_dx INTEGER NOT NULL,
  raw_dy INTEGER NOT NULL,
  processed_dx REAL NOT NULL,
  processed_dy REAL NOT NULL,
  dt_ns INTEGER NOT NULL,
  dt_used_ns INTEGER NOT NULL,
  input_speed REAL,
  acceleration_scale REAL,
  PRIMARY KEY(trial_id, sequence_number),
  FOREIGN KEY(trial_id) REFERENCES aim_trials(id)
);
CREATE INDEX IF NOT EXISTS idx_aim_input_samples_trial ON aim_input_samples(trial_id);

CREATE TABLE IF NOT EXISTS aim_camera_samples (
  trial_id TEXT NOT NULL,
  timestamp_ns INTEGER NOT NULL,
  yaw_deg REAL NOT NULL,
  pitch_deg REAL NOT NULL,
  yaw_delta_deg REAL NOT NULL,
  pitch_delta_deg REAL NOT NULL,
  PRIMARY KEY(trial_id, timestamp_ns),
  FOREIGN KEY(trial_id) REFERENCES aim_trials(id)
);
CREATE INDEX IF NOT EXISTS idx_aim_camera_samples_trial ON aim_camera_samples(trial_id);
"#,
            )
            .map_err(|error| error.to_string())?;

        self.migrate_aim_trial_columns()?;
        self.migrate_aim_shot_columns()?;
        Ok(())
    }

    fn migrate_aim_trial_columns(&self) -> Result<(), String> {
        let additions = [
            ("pitch_model_id", "TEXT NOT NULL DEFAULT ''"),
            ("pitch_model_version", "TEXT NOT NULL DEFAULT ''"),
            ("pitch_config_json", "TEXT NOT NULL DEFAULT '{}'"),
            ("resolution_width", "INTEGER NOT NULL DEFAULT 0"),
            ("resolution_height", "INTEGER NOT NULL DEFAULT 0"),
            ("aspect_ratio", "REAL NOT NULL DEFAULT 0"),
            ("random_seed", "INTEGER NOT NULL DEFAULT 0"),
            ("task_version", "TEXT NOT NULL DEFAULT ''"),
            ("hardware_config_json", "TEXT NOT NULL DEFAULT '{}'"),
            ("view_config_json", "TEXT NOT NULL DEFAULT '{}'"),
        ];
        for (name, decl) in additions {
            add_column_if_missing(&self.connection, "aim_trials", name, decl)?;
        }
        Ok(())
    }

    fn migrate_aim_shot_columns(&self) -> Result<(), String> {
        add_column_if_missing(
            &self.connection,
            "aim_shots",
            "target_id",
            "TEXT NOT NULL DEFAULT ''",
        )
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

    /// Allocates `aim_{date}_{seq:06}` inside the txn (max suffix + 1).
    /// Returns allocated trial id. Rolls back entirely on any error / PK conflict.
    pub fn insert_completed_aim_trial(
        &self,
        utc_date: &str,
        trial: &AimTrialRecord,
        shots: &[AimShotRecord],
        target_events: &[AimTargetEventRecord],
        input_samples: &[AimInputSampleRecord],
        camera_samples: &[AimCameraSampleRecord],
    ) -> Result<String, String> {
        if trial.status != "completed" {
            return Err(format!(
                "aim trial status must be \"completed\", got \"{}\"",
                trial.status
            ));
        }

        let prefix = format!("aim_{utc_date}_");
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| error.to_string())?;
        let sequence = next_aim_sequence(&transaction, &prefix)?;
        let trial_id = format!("{prefix}{sequence:06}");

        transaction
            .execute(
                r#"
INSERT INTO aim_trials (
  id, app_version, experiment_id, experiment_version, trial_type, status,
  processor_id, processor_version, processor_config_json, dpi, sensitivity,
  polling_rate_hz, fov_degrees_h,
  pitch_model_id, pitch_model_version, pitch_config_json,
  resolution_width, resolution_height, aspect_ratio, random_seed, task_version,
  hardware_config_json, view_config_json,
  task_config_json, metrics_json,
  start_unix_ms, end_unix_ms, start_timestamp_ns, end_timestamp_ns,
  duration_secs, hits, shots, misses, score_secs, accuracy
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
"#,
                params![
                    trial_id,
                    trial.app_version,
                    trial.experiment_id,
                    trial.experiment_version,
                    trial.trial_type,
                    trial.status,
                    trial.processor_id,
                    trial.processor_version,
                    trial.processor_config_json,
                    trial.dpi,
                    trial.sensitivity,
                    trial.polling_rate_hz,
                    trial.fov_degrees_h,
                    trial.pitch_model_id,
                    trial.pitch_model_version,
                    trial.pitch_config_json,
                    as_i64(u64::from(trial.resolution_width), "resolution width")?,
                    as_i64(u64::from(trial.resolution_height), "resolution height")?,
                    trial.aspect_ratio,
                    as_i64(trial.random_seed, "random seed")?,
                    trial.task_version,
                    trial.hardware_config_json,
                    trial.view_config_json,
                    trial.task_config_json,
                    trial.metrics_json,
                    trial.start_unix_ms,
                    trial.end_unix_ms,
                    as_i64(trial.start_timestamp_ns, "start timestamp ns")?,
                    as_i64(trial.end_timestamp_ns, "end timestamp ns")?,
                    trial.duration_secs,
                    as_i64(u64::from(trial.hits), "hits")?,
                    as_i64(u64::from(trial.shots), "shots")?,
                    as_i64(u64::from(trial.misses), "misses")?,
                    trial.score_secs,
                    trial.accuracy,
                ],
            )
            .map_err(|error| error.to_string())?;

        {
            let mut statement = transaction
                .prepare(
                    r#"
INSERT INTO aim_target_events (
  trial_id, target_id, event_index, timestamp_ns, event_type,
  position_x, position_y, position_z, yaw_deg, pitch_deg,
  velocity_x, velocity_y, velocity_z, event_data_json
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
"#,
                )
                .map_err(|error| error.to_string())?;
            for event in target_events {
                statement
                    .execute(params![
                        trial_id,
                        event.target_id,
                        as_i64(u64::from(event.event_index), "event index")?,
                        as_i64(event.timestamp_ns, "target event timestamp ns")?,
                        event.event_type,
                        event.position_x,
                        event.position_y,
                        event.position_z,
                        event.yaw_deg,
                        event.pitch_deg,
                        event.velocity_x,
                        event.velocity_y,
                        event.velocity_z,
                        event.event_data_json,
                    ])
                    .map_err(|error| error.to_string())?;
            }
        }

        {
            let mut statement = transaction
                .prepare(
                    r#"
INSERT INTO aim_shots (
  trial_id, shot_index, timestamp_ns, hit, yaw_deg, pitch_deg,
  target_x, target_y, target_z, target_radius, target_id
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
"#,
                )
                .map_err(|error| error.to_string())?;
            for shot in shots {
                statement
                    .execute(params![
                        trial_id,
                        as_i64(u64::from(shot.shot_index), "shot index")?,
                        as_i64(shot.timestamp_ns, "shot timestamp ns")?,
                        shot.hit as i64,
                        shot.yaw_deg,
                        shot.pitch_deg,
                        shot.target_x,
                        shot.target_y,
                        shot.target_z,
                        shot.target_radius,
                        shot.target_id,
                    ])
                    .map_err(|error| error.to_string())?;
            }
        }

        {
            let mut statement = transaction
                .prepare(
                    r#"
INSERT INTO aim_input_samples (
  trial_id, timestamp_ns, sequence_number, raw_dx, raw_dy,
  processed_dx, processed_dy, dt_ns, dt_used_ns, input_speed, acceleration_scale
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
"#,
                )
                .map_err(|error| error.to_string())?;
            for sample in input_samples {
                statement
                    .execute(params![
                        trial_id,
                        as_i64(sample.timestamp_ns, "input sample timestamp ns")?,
                        as_i64(sample.sequence_number, "input sample sequence number")?,
                        sample.raw_dx,
                        sample.raw_dy,
                        sample.processed_dx,
                        sample.processed_dy,
                        as_i64(sample.dt_ns, "input sample dt_ns")?,
                        as_i64(sample.dt_used_ns, "input sample dt_used_ns")?,
                        sample.input_speed,
                        sample.acceleration_scale,
                    ])
                    .map_err(|error| error.to_string())?;
            }
        }

        {
            let mut statement = transaction
                .prepare(
                    r#"
INSERT INTO aim_camera_samples (
  trial_id, timestamp_ns, yaw_deg, pitch_deg, yaw_delta_deg, pitch_delta_deg
) VALUES (?, ?, ?, ?, ?, ?)
"#,
                )
                .map_err(|error| error.to_string())?;
            for sample in camera_samples {
                statement
                    .execute(params![
                        trial_id,
                        as_i64(sample.timestamp_ns, "camera sample timestamp ns")?,
                        sample.yaw_deg,
                        sample.pitch_deg,
                        sample.yaw_delta_deg,
                        sample.pitch_delta_deg,
                    ])
                    .map_err(|error| error.to_string())?;
            }
        }

        transaction.commit().map_err(|error| error.to_string())?;
        Ok(trial_id)
    }

    pub fn list_aim_trials_summary(&self, limit: usize) -> Result<Vec<AimTrialSummary>, String> {
        let limit =
            i64::try_from(limit).map_err(|_| "aim trial summary limit is too large".to_owned())?;
        let mut statement = self
            .connection
            .prepare(
                r#"
SELECT id, trial_type, hits, shots, accuracy, score_secs, experiment_version, end_unix_ms
FROM aim_trials
ORDER BY end_unix_ms DESC
LIMIT ?1
"#,
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([limit], |row| {
                Ok(AimTrialSummary {
                    id: row.get(0)?,
                    trial_type: row.get(1)?,
                    hits: row.get(2)?,
                    shots: row.get(3)?,
                    accuracy: row.get(4)?,
                    score_secs: row.get(5)?,
                    experiment_version: row.get(6)?,
                    end_unix_ms: row.get(7)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| error.to_string())
    }

    pub fn load_aim_trial_bundle(&self, id: &str) -> Result<AimTrialReplayBundle, String> {
        let trial = self
            .connection
            .query_row(
                r#"
SELECT id, app_version, experiment_id, experiment_version, trial_type, status,
       processor_id, processor_version, processor_config_json, dpi, sensitivity,
       polling_rate_hz, fov_degrees_h, pitch_model_id, pitch_model_version,
       pitch_config_json, resolution_width, resolution_height, aspect_ratio,
       random_seed, task_version, hardware_config_json, view_config_json,
       task_config_json, metrics_json, start_unix_ms, end_unix_ms,
       start_timestamp_ns, end_timestamp_ns, duration_secs, hits, shots, misses,
       score_secs, accuracy
FROM aim_trials
WHERE id = ?1
"#,
                [id],
                aim_trial_from_row,
            )
            .optional()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("aim trial not found: {id}"))?;

        let target_events = query_target_events(&self.connection, id)?;
        let shots = query_shots(&self.connection, id)?;
        let camera_samples = query_camera_samples(&self.connection, id)?;

        Ok(AimTrialReplayBundle {
            trial,
            target_events,
            shots,
            camera_samples,
        })
    }
}

fn aim_trial_from_row(row: &Row<'_>) -> rusqlite::Result<AimTrialRecord> {
    Ok(AimTrialRecord {
        id: row.get(0)?,
        app_version: row.get(1)?,
        experiment_id: row.get(2)?,
        experiment_version: row.get(3)?,
        trial_type: row.get(4)?,
        status: row.get(5)?,
        processor_id: row.get(6)?,
        processor_version: row.get(7)?,
        processor_config_json: row.get(8)?,
        dpi: row.get(9)?,
        sensitivity: row.get(10)?,
        polling_rate_hz: row.get(11)?,
        fov_degrees_h: row.get(12)?,
        pitch_model_id: row.get(13)?,
        pitch_model_version: row.get(14)?,
        pitch_config_json: row.get(15)?,
        resolution_width: row.get(16)?,
        resolution_height: row.get(17)?,
        aspect_ratio: row.get(18)?,
        random_seed: row.get(19)?,
        task_version: row.get(20)?,
        hardware_config_json: row.get(21)?,
        view_config_json: row.get(22)?,
        task_config_json: row.get(23)?,
        metrics_json: row.get(24)?,
        start_unix_ms: row.get(25)?,
        end_unix_ms: row.get(26)?,
        start_timestamp_ns: row.get(27)?,
        end_timestamp_ns: row.get(28)?,
        duration_secs: row.get(29)?,
        hits: row.get(30)?,
        shots: row.get(31)?,
        misses: row.get(32)?,
        score_secs: row.get(33)?,
        accuracy: row.get(34)?,
    })
}

fn query_target_events(
    connection: &Connection,
    trial_id: &str,
) -> Result<Vec<AimTargetEventRecord>, String> {
    let mut statement = connection
        .prepare(
            r#"
SELECT target_id, event_index, timestamp_ns, event_type, position_x, position_y,
       position_z, yaw_deg, pitch_deg, velocity_x, velocity_y, velocity_z,
       event_data_json
FROM aim_target_events
WHERE trial_id = ?1
ORDER BY timestamp_ns, event_index
"#,
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([trial_id], |row| {
            Ok(AimTargetEventRecord {
                target_id: row.get(0)?,
                event_index: row.get(1)?,
                timestamp_ns: row.get(2)?,
                event_type: row.get(3)?,
                position_x: row.get(4)?,
                position_y: row.get(5)?,
                position_z: row.get(6)?,
                yaw_deg: row.get(7)?,
                pitch_deg: row.get(8)?,
                velocity_x: row.get(9)?,
                velocity_y: row.get(10)?,
                velocity_z: row.get(11)?,
                event_data_json: row.get(12)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| error.to_string())
}

fn query_shots(connection: &Connection, trial_id: &str) -> Result<Vec<AimShotRecord>, String> {
    let mut statement = connection
        .prepare(
            r#"
SELECT shot_index, timestamp_ns, hit, yaw_deg, pitch_deg, target_x, target_y,
       target_z, target_radius, target_id
FROM aim_shots
WHERE trial_id = ?1
ORDER BY timestamp_ns, shot_index
"#,
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([trial_id], |row| {
            Ok(AimShotRecord {
                shot_index: row.get(0)?,
                timestamp_ns: row.get(1)?,
                hit: row.get(2)?,
                yaw_deg: row.get(3)?,
                pitch_deg: row.get(4)?,
                target_x: row.get(5)?,
                target_y: row.get(6)?,
                target_z: row.get(7)?,
                target_radius: row.get(8)?,
                target_id: row.get(9)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| error.to_string())
}

fn query_camera_samples(
    connection: &Connection,
    trial_id: &str,
) -> Result<Vec<AimCameraSampleRecord>, String> {
    let mut statement = connection
        .prepare(
            r#"
SELECT timestamp_ns, yaw_deg, pitch_deg, yaw_delta_deg, pitch_delta_deg
FROM aim_camera_samples
WHERE trial_id = ?1
ORDER BY timestamp_ns
"#,
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([trial_id], |row| {
            Ok(AimCameraSampleRecord {
                timestamp_ns: row.get(0)?,
                yaw_deg: row.get(1)?,
                pitch_deg: row.get(2)?,
                yaw_delta_deg: row.get(3)?,
                pitch_delta_deg: row.get(4)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| error.to_string())
}

fn add_column_if_missing(
    connection: &Connection,
    table: &str,
    column: &str,
    decl: &str,
) -> Result<(), String> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| error.to_string())?;
    let exists = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .any(|name| name == column);
    if exists {
        return Ok(());
    }
    connection
        .execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"),
            [],
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn next_aim_sequence(connection: &Connection, prefix: &str) -> Result<u32, String> {
    let like_pattern = format!("{prefix}%");
    let mut statement = connection
        .prepare("SELECT id FROM aim_trials WHERE id LIKE ?1")
        .map_err(|error| error.to_string())?;
    let mut rows = statement
        .query([like_pattern])
        .map_err(|error| error.to_string())?;
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
        .ok_or_else(|| format!("aim trial ID sequence exhausted for {prefix}"))
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

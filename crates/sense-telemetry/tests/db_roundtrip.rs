use std::path::Path;

use sense_telemetry::{SessionBuffers, TelemetryDb};
use sense_types::{
    AccelerationConfig, ConfigurationRecord, DisplayConfig, FovAxis, FrameSample,
    InputCameraSample, InputIntegrityReport, MouseSample, ProcessedMouseSample, SensitivityConfig,
    SessionRecord, ValidationResult,
};

#[test]
fn sqlite_roundtrip_flushes_all_sample_types() {
    let db = TelemetryDb::open(Path::new(":memory:")).unwrap();
    db.migrate().unwrap();

    let configuration = ConfigurationRecord {
        id: "config-1".into(),
        sensitivity: SensitivityConfig {
            dpi: 800.0,
            sensitivity: 0.35,
            yaw_deg_per_count_at_sens_1: 0.07,
            fov_axis: FovAxis::Horizontal,
            fov_degrees: 103.0,
        },
        edpi: 280.0,
        display: DisplayConfig {
            width: 1920,
            height: 1080,
            refresh_hz: 240.0,
        },
        accel: AccelerationConfig {
            enabled: false,
            model: "none".into(),
            processor_id: "none".into(),
            processor_version: "1.0.0".into(),
            processor_config_json: "{}".into(),
        },
        polling_rate_hz: Some(1_000.0),
    };
    db.upsert_configuration(&configuration).unwrap();

    let session = SessionRecord {
        id: "session-1".into(),
        app_version: "0.1.0".into(),
        experiment_id: "validation".into(),
        experiment_version: "1".into(),
        config_id: configuration.id.clone(),
        seed: 42,
        start_unix_ms: 1_700_000_000_000,
        end_unix_ms: None,
    };
    db.insert_session_start(&session).unwrap();

    let buffers = SessionBuffers {
        mouse: vec![
            MouseSample {
                timestamp_ns: 10,
                dx: 4,
                dy: -2,
                buttons: 0,
                sequence_number: 1,
            },
            MouseSample {
                timestamp_ns: 20,
                dx: 5,
                dy: -3,
                buttons: 1,
                sequence_number: 2,
            },
        ],
        processed: vec![
            ProcessedMouseSample {
                timestamp_ns: 10,
                sequence_number: 1,
                processed_dx: 4.0,
                processed_dy: -2.0,
                processor_id: "none".into(),
                processor_version: "1.0.0".into(),
                processor_config_json: "{}".into(),
            },
            ProcessedMouseSample {
                timestamp_ns: 20,
                sequence_number: 2,
                processed_dx: 5.0,
                processed_dy: -3.0,
                processor_id: "none".into(),
                processor_version: "1.0.0".into(),
                processor_config_json: "{}".into(),
            },
        ],
        input_camera: vec![
            InputCameraSample {
                timestamp_ns: 10,
                yaw_deg: 0.28,
                pitch_deg: 0.14,
                sequence_number: 1,
            },
            InputCameraSample {
                timestamp_ns: 20,
                yaw_deg: 0.63,
                pitch_deg: 0.35,
                sequence_number: 2,
            },
        ],
        frames: vec![FrameSample {
            timestamp_ns: 20,
            frame_time_s: 1.0 / 240.0,
            fps: 240.0,
        }],
    };
    let result = ValidationResult {
        expected_counts: 9.0,
        observed_net_counts: 9.0,
        observed_abs_path_counts: 9.0,
        expected_degrees: 0.63,
        observed_degrees: 0.63,
        count_difference: 0.0,
        error_percent: 0.0,
        integrity: InputIntegrityReport {
            samples_received: 2,
            sequence_gaps: 0,
            duplicate_sequences: 0,
            out_of_order_samples: 0,
            timestamp_regressions: 0,
        },
    };
    let end_unix_ms = 1_700_000_001_234;
    db.complete_validation(&session.id, &buffers, &result, end_unix_ms)
        .unwrap();

    for (table, expected) in [
        ("raw_mouse_events", 2),
        ("processed_mouse_events", 2),
        ("input_camera_samples", 2),
        ("frame_samples", 1),
        ("validation_results", 1),
    ] {
        let count: i64 = db
            .connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, expected, "unexpected row count in {table}");
    }

    let processor_id: String = db
        .connection
        .query_row(
            "SELECT processor_id FROM processed_mouse_events WHERE sequence_number = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(processor_id, "none");

    let stored_end_unix_ms: i64 = db
        .connection
        .query_row(
            "SELECT end_unix_ms FROM sessions WHERE id = ?1",
            [&session.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored_end_unix_ms, end_unix_ms);
}

#[test]
fn migrate_upgrades_existing_m1_database_with_processed_table() {
    let db = TelemetryDb::open(Path::new(":memory:")).unwrap();
    // Simulate a pre-M2 database: M1 tables only, no processed_mouse_events.
    db.connection
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
"#,
        )
        .unwrap();

    let before: bool = db
        .connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='processed_mouse_events')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!before, "fixture must start without processed_mouse_events");

    db.migrate().unwrap();

    let after: bool = db
        .connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='processed_mouse_events')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(after, "migrate must add processed_mouse_events on existing M1 DBs");

    // Second migrate is idempotent.
    db.migrate().unwrap();
}

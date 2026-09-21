use std::path::Path;

use sense_telemetry::{SessionBuffers, TelemetryDb};
use sense_types::{
    AccelerationConfig, AimCameraSampleRecord, AimInputSampleRecord, AimShotRecord,
    AimTargetEventRecord, AimTrialRecord, ConfigurationRecord, DisplayConfig, FovAxis,
    FrameSample, InputCameraSample, InputIntegrityReport, MouseSample, ProcessedMouseSample,
    SensitivityConfig, SessionRecord, ValidationResult,
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

fn sample_aim_trial(status: &str) -> AimTrialRecord {
    AimTrialRecord {
        id: String::new(),
        app_version: "0.7.0".into(),
        experiment_id: "aim_lab".into(),
        experiment_version: "0.7.0".into(),
        trial_type: "STATIC_CLICK".into(),
        status: status.into(),
        processor_id: "none".into(),
        processor_version: "1.0.0".into(),
        processor_config_json: "{}".into(),
        dpi: 800.0,
        sensitivity: 0.35,
        polling_rate_hz: 1_000.0,
        fov_degrees_h: 103.0,
        pitch_model_id: "unverified_0.1".into(),
        pitch_model_version: "1".into(),
        pitch_config_json: r#"{"certainty":"UNCERTAIN"}"#.into(),
        resolution_width: 1920,
        resolution_height: 1080,
        aspect_ratio: 1920.0 / 1080.0,
        random_seed: 42,
        task_version: "1".into(),
        hardware_config_json: "{}".into(),
        view_config_json: r#"{"projection":"perspective"}"#.into(),
        task_config_json: r#"{"hits_required":5,"target_radius":0.25}"#.into(),
        metrics_json: "{}".into(),
        start_unix_ms: 1_700_000_000_000,
        end_unix_ms: 1_700_000_005_000,
        start_timestamp_ns: 1_000_000_000,
        end_timestamp_ns: 6_000_000_000,
        duration_secs: 5.0,
        hits: 5,
        shots: 7,
        misses: 2,
        score_secs: 4.2,
        accuracy: 5.0 / 7.0,
    }
}

fn sample_aim_shots() -> Vec<AimShotRecord> {
    vec![
        AimShotRecord {
            shot_index: 0,
            timestamp_ns: 1_100_000_000,
            hit: true,
            yaw_deg: 0.1,
            pitch_deg: -0.2,
            target_x: 1.0,
            target_y: 2.0,
            target_z: 10.0,
            target_radius: 0.25,
            target_id: "target_001".into(),
        },
        AimShotRecord {
            shot_index: 1,
            timestamp_ns: 1_200_000_000,
            hit: false,
            yaw_deg: 0.3,
            pitch_deg: -0.1,
            target_x: -1.0,
            target_y: 1.5,
            target_z: 10.0,
            target_radius: 0.25,
            target_id: "target_001".into(),
        },
    ]
}

#[test]
fn insert_completed_aim_trial_roundtrip_and_sequence() {
    let db = TelemetryDb::open(Path::new(":memory:")).unwrap();
    db.migrate().unwrap();

    let trial = sample_aim_trial("completed");
    let shots = sample_aim_shots();
    let trial_id = db
        .insert_completed_aim_trial("20260918", &trial, &shots, &[], &[], &[])
        .unwrap();
    assert_eq!(trial_id, "aim_20260918_000001");

    let (hits, shots_count, misses, score_secs, accuracy, trial_type, status, experiment_version): (
        u32,
        u32,
        u32,
        f64,
        f64,
        String,
        String,
        String,
    ) = db
        .connection
        .query_row(
            "SELECT hits, shots, misses, score_secs, accuracy, trial_type, status, experiment_version
             FROM aim_trials WHERE id = ?1",
            [&trial_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)? as u32,
                    row.get::<_, i64>(1)? as u32,
                    row.get::<_, i64>(2)? as u32,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(hits, trial.hits);
    assert_eq!(shots_count, trial.shots);
    assert_eq!(misses, trial.misses);
    assert!((score_secs - trial.score_secs).abs() < f64::EPSILON);
    assert!((accuracy - trial.accuracy).abs() < f64::EPSILON);
    assert_eq!(trial_type, trial.trial_type);
    assert_eq!(status, "completed");
    assert_eq!(experiment_version, trial.experiment_version);

    let mut statement = db
        .connection
        .prepare(
            "SELECT shot_index, hit FROM aim_shots WHERE trial_id = ?1 ORDER BY shot_index",
        )
        .unwrap();
    let rows: Vec<(u32, bool)> = statement
        .query_map([&trial_id], |row| {
            Ok((row.get::<_, i64>(0)? as u32, row.get::<_, i64>(1)? != 0))
        })
        .unwrap()
        .map(|row| row.unwrap())
        .collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], (0, true));
    assert_eq!(rows[1], (1, false));

    let second_id = db
        .insert_completed_aim_trial("20260918", &trial, &shots, &[], &[], &[])
        .unwrap();
    assert_eq!(second_id, "aim_20260918_000002");
}

#[test]
fn insert_completed_aim_trial_rejects_non_completed_status() {
    let db = TelemetryDb::open(Path::new(":memory:")).unwrap();
    db.migrate().unwrap();

    let trial = sample_aim_trial("armed");
    let shots = sample_aim_shots();
    let error = db
        .insert_completed_aim_trial("20260918", &trial, &shots, &[], &[], &[])
        .unwrap_err();
    assert!(
        error.contains("completed"),
        "expected status guard error, got: {error}"
    );

    let count: i64 = db
        .connection
        .query_row("SELECT COUNT(*) FROM aim_trials", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn insert_completed_aim_trial_rolls_back_on_duplicate_shot_index() {
    let db = TelemetryDb::open(Path::new(":memory:")).unwrap();
    db.migrate().unwrap();

    let trial = sample_aim_trial("completed");
    let shot = AimShotRecord {
        shot_index: 0,
        timestamp_ns: 1_100_000_000,
        hit: true,
        yaw_deg: 0.1,
        pitch_deg: -0.2,
        target_x: 1.0,
        target_y: 2.0,
        target_z: 10.0,
        target_radius: 0.25,
        target_id: "target_001".into(),
    };
    // Second row reuses shot_index 0 so the shot INSERT violates
    // PRIMARY KEY(trial_id, shot_index) after the parent aim_trials row is written.
    let duplicate_shots = vec![shot.clone(), shot];

    let error = db
        .insert_completed_aim_trial("20260918", &trial, &duplicate_shots, &[], &[], &[])
        .unwrap_err();
    assert!(
        !error.is_empty(),
        "duplicate shot_index must fail the insert"
    );

    let trial_count: i64 = db
        .connection
        .query_row("SELECT COUNT(*) FROM aim_trials", [], |row| row.get(0))
        .unwrap();
    let shot_count: i64 = db
        .connection
        .query_row("SELECT COUNT(*) FROM aim_shots", [], |row| row.get(0))
        .unwrap();
    assert_eq!(trial_count, 0, "aim_trials must fully roll back");
    assert_eq!(shot_count, 0, "aim_shots must fully roll back");
}

#[test]
fn insert_completed_aim_trial_roundtrips_five_layers() {
    let db = TelemetryDb::open(Path::new(":memory:")).unwrap();
    db.migrate().unwrap();

    let trial = sample_aim_trial("completed");
    let target_events = vec![
        AimTargetEventRecord {
            target_id: "target_001".into(),
            event_index: 0,
            timestamp_ns: 1_050_000_000,
            event_type: "spawn".into(),
            position_x: 1.0,
            position_y: 2.0,
            position_z: 10.0,
            yaw_deg: Some(5.0),
            pitch_deg: Some(-1.0),
            velocity_x: 0.0,
            velocity_y: 0.0,
            velocity_z: 0.0,
            event_data_json: r#"{"radius":0.25}"#.into(),
        },
        AimTargetEventRecord {
            target_id: "target_001".into(),
            event_index: 1,
            timestamp_ns: 2_000_000_000,
            event_type: "despawn".into(),
            position_x: 1.0,
            position_y: 2.0,
            position_z: 10.0,
            yaw_deg: None,
            pitch_deg: None,
            velocity_x: 0.0,
            velocity_y: 0.0,
            velocity_z: 0.0,
            event_data_json: "{}".into(),
        },
    ];
    let shots = vec![AimShotRecord {
        shot_index: 0,
        timestamp_ns: 1_900_000_000,
        hit: true,
        yaw_deg: 0.1,
        pitch_deg: -0.2,
        target_x: 1.0,
        target_y: 2.0,
        target_z: 10.0,
        target_radius: 0.25,
        target_id: "target_001".into(),
    }];
    // First sample: dt_ns=0; second: raw dt_ns differs from clamped dt_used_ns.
    let input_samples = vec![
        AimInputSampleRecord {
            timestamp_ns: 1_100_000_000,
            sequence_number: 0,
            raw_dx: 1,
            raw_dy: 0,
            processed_dx: 1.0,
            processed_dy: 0.0,
            dt_ns: 0,
            dt_used_ns: 0,
            input_speed: None,
            acceleration_scale: None,
        },
        AimInputSampleRecord {
            timestamp_ns: 1_100_500_000,
            sequence_number: 1,
            raw_dx: 2,
            raw_dy: -1,
            processed_dx: 2.0,
            processed_dy: -1.0,
            dt_ns: 500_000,
            dt_used_ns: 1_000_000,
            input_speed: Some(2.236),
            acceleration_scale: Some(1.0),
        },
    ];
    let camera_samples = vec![
        AimCameraSampleRecord {
            timestamp_ns: 1_100_000_000,
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            yaw_delta_deg: 0.0,
            pitch_delta_deg: 0.0,
        },
        AimCameraSampleRecord {
            timestamp_ns: 1_100_500_000,
            yaw_deg: 0.14,
            pitch_deg: -0.07,
            yaw_delta_deg: 0.14,
            pitch_delta_deg: -0.07,
        },
    ];

    let trial_id = db
        .insert_completed_aim_trial(
            "20260919",
            &trial,
            &shots,
            &target_events,
            &input_samples,
            &camera_samples,
        )
        .unwrap();
    assert_eq!(trial_id, "aim_20260919_000001");

    let (
        pitch_model_id,
        resolution_width,
        aspect_ratio,
        random_seed,
        task_version,
        hardware_config_json,
        view_config_json,
    ): (String, u32, f64, u64, String, String, String) = db
        .connection
        .query_row(
            "SELECT pitch_model_id, resolution_width, aspect_ratio, random_seed,
                    task_version, hardware_config_json, view_config_json
             FROM aim_trials WHERE id = ?1",
            [&trial_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get::<_, i64>(1)? as u32,
                    row.get(2)?,
                    row.get::<_, i64>(3)? as u64,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(pitch_model_id, trial.pitch_model_id);
    assert_eq!(resolution_width, trial.resolution_width);
    assert!((aspect_ratio - trial.aspect_ratio).abs() < f64::EPSILON);
    assert_eq!(random_seed, trial.random_seed);
    assert_eq!(task_version, trial.task_version);
    assert_eq!(hardware_config_json, trial.hardware_config_json);
    assert_eq!(view_config_json, trial.view_config_json);

    let event_count: i64 = db
        .connection
        .query_row(
            "SELECT COUNT(*) FROM aim_target_events WHERE trial_id = ?1",
            [&trial_id],
            |row| row.get(0),
        )
        .unwrap();
    let shot_count: i64 = db
        .connection
        .query_row(
            "SELECT COUNT(*) FROM aim_shots WHERE trial_id = ?1",
            [&trial_id],
            |row| row.get(0),
        )
        .unwrap();
    let input_count: i64 = db
        .connection
        .query_row(
            "SELECT COUNT(*) FROM aim_input_samples WHERE trial_id = ?1",
            [&trial_id],
            |row| row.get(0),
        )
        .unwrap();
    let camera_count: i64 = db
        .connection
        .query_row(
            "SELECT COUNT(*) FROM aim_camera_samples WHERE trial_id = ?1",
            [&trial_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(event_count, 2);
    assert_eq!(shot_count, 1);
    assert_eq!(input_count, 2);
    assert_eq!(camera_count, 2);

    let shot_target_id: String = db
        .connection
        .query_row(
            "SELECT target_id FROM aim_shots WHERE trial_id = ?1 AND shot_index = 0",
            [&trial_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(shot_target_id, "target_001");

    let despawn_yaw: Option<f64> = db
        .connection
        .query_row(
            "SELECT yaw_deg FROM aim_target_events
             WHERE trial_id = ?1 AND event_type = 'despawn'",
            [&trial_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(despawn_yaw, None);

    let (dt_ns, dt_used_ns, input_speed, acceleration_scale): (
        u64,
        u64,
        Option<f64>,
        Option<f64>,
    ) = db
        .connection
        .query_row(
            "SELECT dt_ns, dt_used_ns, input_speed, acceleration_scale
             FROM aim_input_samples
             WHERE trial_id = ?1 AND sequence_number = 1",
            [&trial_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)? as u64,
                    row.get::<_, i64>(1)? as u64,
                    row.get(2)?,
                    row.get(3)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(dt_ns, 500_000, "dt_ns must store raw QPC interval");
    assert_eq!(dt_used_ns, 1_000_000, "dt_used_ns must store clamped interval");
    assert_ne!(dt_ns, dt_used_ns);
    assert!((input_speed.unwrap() - 2.236).abs() < 1e-9);
    assert!((acceleration_scale.unwrap() - 1.0).abs() < f64::EPSILON);

    let (yaw_deg, yaw_delta_deg): (f64, f64) = db
        .connection
        .query_row(
            "SELECT yaw_deg, yaw_delta_deg FROM aim_camera_samples
             WHERE trial_id = ?1 AND timestamp_ns = 1100500000",
            [&trial_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!((yaw_deg - 0.14).abs() < f64::EPSILON);
    assert!((yaw_delta_deg - 0.14).abs() < f64::EPSILON);
}

#[test]
fn insert_completed_aim_trial_rolls_back_all_layers_on_child_failure() {
    let db = TelemetryDb::open(Path::new(":memory:")).unwrap();
    db.migrate().unwrap();

    let trial = sample_aim_trial("completed");
    let target_events = vec![AimTargetEventRecord {
        target_id: "target_001".into(),
        event_index: 0,
        timestamp_ns: 1_050_000_000,
        event_type: "spawn".into(),
        position_x: 0.0,
        position_y: 0.0,
        position_z: 10.0,
        yaw_deg: None,
        pitch_deg: None,
        velocity_x: 0.0,
        velocity_y: 0.0,
        velocity_z: 0.0,
        event_data_json: "{}".into(),
    }];
    let shot = AimShotRecord {
        shot_index: 0,
        timestamp_ns: 1_100_000_000,
        hit: true,
        yaw_deg: 0.0,
        pitch_deg: 0.0,
        target_x: 0.0,
        target_y: 0.0,
        target_z: 10.0,
        target_radius: 0.25,
        target_id: "target_001".into(),
    };
    let duplicate_shots = vec![shot.clone(), shot];
    let input_samples = vec![AimInputSampleRecord {
        timestamp_ns: 1_100_000_000,
        sequence_number: 0,
        raw_dx: 0,
        raw_dy: 0,
        processed_dx: 0.0,
        processed_dy: 0.0,
        dt_ns: 0,
        dt_used_ns: 0,
        input_speed: None,
        acceleration_scale: None,
    }];
    let camera_samples = vec![AimCameraSampleRecord {
        timestamp_ns: 1_100_000_000,
        yaw_deg: 0.0,
        pitch_deg: 0.0,
        yaw_delta_deg: 0.0,
        pitch_delta_deg: 0.0,
    }];

    let error = db
        .insert_completed_aim_trial(
            "20260919",
            &trial,
            &duplicate_shots,
            &target_events,
            &input_samples,
            &camera_samples,
        )
        .unwrap_err();
    assert!(!error.is_empty());

    let trial_count: i64 = db
        .connection
        .query_row("SELECT COUNT(*) FROM aim_trials", [], |row| row.get(0))
        .unwrap();
    let event_count: i64 = db
        .connection
        .query_row("SELECT COUNT(*) FROM aim_target_events", [], |row| row.get(0))
        .unwrap();
    let shot_count: i64 = db
        .connection
        .query_row("SELECT COUNT(*) FROM aim_shots", [], |row| row.get(0))
        .unwrap();
    let input_count: i64 = db
        .connection
        .query_row("SELECT COUNT(*) FROM aim_input_samples", [], |row| row.get(0))
        .unwrap();
    let camera_count: i64 = db
        .connection
        .query_row("SELECT COUNT(*) FROM aim_camera_samples", [], |row| row.get(0))
        .unwrap();
    assert_eq!(trial_count, 0, "aim_trials must fully roll back");
    assert_eq!(event_count, 0, "aim_target_events must fully roll back");
    assert_eq!(shot_count, 0, "aim_shots must fully roll back");
    assert_eq!(input_count, 0, "aim_input_samples must fully roll back");
    assert_eq!(camera_count, 0, "aim_camera_samples must fully roll back");
}

#[test]
fn list_aim_trials_summary_orders_newest_first_and_honors_limit() {
    let db = TelemetryDb::open(Path::new(":memory:")).unwrap();
    db.migrate().unwrap();

    let mut older = sample_aim_trial("completed");
    older.end_unix_ms = 1_700_000_005_000;
    older.trial_type = "STATIC_CLICK".into();
    let older_id = db
        .insert_completed_aim_trial("20260918", &older, &[], &[], &[], &[])
        .unwrap();

    let mut newer = sample_aim_trial("completed");
    newer.end_unix_ms = 1_700_000_010_000;
    newer.trial_type = "GRIDSHOT".into();
    newer.hits = 12;
    newer.shots = 15;
    newer.accuracy = 0.8;
    newer.score_secs = 3.5;
    newer.experiment_version = "0.12.1".into();
    let newer_id = db
        .insert_completed_aim_trial("20260919", &newer, &[], &[], &[], &[])
        .unwrap();

    let summaries = db.list_aim_trials_summary(1).unwrap();

    assert_eq!(summaries.len(), 1);
    let summary = &summaries[0];
    assert_eq!(summary.id, newer_id);
    assert_eq!(summary.trial_type, "GRIDSHOT");
    assert_eq!(summary.hits, 12);
    assert_eq!(summary.shots, 15);
    assert!((summary.accuracy - 0.8).abs() < f64::EPSILON);
    assert!((summary.score_secs - 3.5).abs() < f64::EPSILON);
    assert_eq!(summary.experiment_version, "0.12.1");
    assert_eq!(summary.end_unix_ms, newer.end_unix_ms);
    assert_ne!(summary.id, older_id);
}

#[test]
fn load_aim_trial_bundle_roundtrips_and_orders_children() {
    let db = TelemetryDb::open(Path::new(":memory:")).unwrap();
    db.migrate().unwrap();

    let trial = sample_aim_trial("completed");
    let target_events = vec![
        AimTargetEventRecord {
            target_id: "target_001".into(),
            event_index: 1,
            timestamp_ns: 2_000_000_000,
            event_type: "despawn".into(),
            position_x: 1.0,
            position_y: 2.0,
            position_z: 10.0,
            yaw_deg: None,
            pitch_deg: None,
            velocity_x: 0.0,
            velocity_y: 0.0,
            velocity_z: 0.0,
            event_data_json: "{}".into(),
        },
        AimTargetEventRecord {
            target_id: "target_001".into(),
            event_index: 0,
            timestamp_ns: 1_050_000_000,
            event_type: "spawn".into(),
            position_x: 1.0,
            position_y: 2.0,
            position_z: 10.0,
            yaw_deg: Some(5.0),
            pitch_deg: Some(-1.0),
            velocity_x: 0.0,
            velocity_y: 0.0,
            velocity_z: 0.0,
            event_data_json: r#"{"radius":0.25}"#.into(),
        },
    ];
    let shots = vec![
        AimShotRecord {
            shot_index: 1,
            timestamp_ns: 1_900_000_000,
            hit: false,
            yaw_deg: 0.2,
            pitch_deg: -0.1,
            target_x: 1.0,
            target_y: 2.0,
            target_z: 10.0,
            target_radius: 0.25,
            target_id: "target_001".into(),
        },
        AimShotRecord {
            shot_index: 0,
            timestamp_ns: 1_100_000_000,
            hit: true,
            yaw_deg: 0.1,
            pitch_deg: -0.2,
            target_x: 1.0,
            target_y: 2.0,
            target_z: 10.0,
            target_radius: 0.25,
            target_id: "target_001".into(),
        },
    ];
    let camera_samples = vec![
        AimCameraSampleRecord {
            timestamp_ns: 1_200_000_000,
            yaw_deg: 0.2,
            pitch_deg: -0.1,
            yaw_delta_deg: 0.1,
            pitch_delta_deg: 0.1,
        },
        AimCameraSampleRecord {
            timestamp_ns: 1_100_000_000,
            yaw_deg: 0.1,
            pitch_deg: -0.2,
            yaw_delta_deg: 0.1,
            pitch_delta_deg: -0.2,
        },
    ];
    let trial_id = db
        .insert_completed_aim_trial(
            "20260919",
            &trial,
            &shots,
            &target_events,
            &[],
            &camera_samples,
        )
        .unwrap();
    let mut expected_trial = trial.clone();
    expected_trial.id = trial_id.clone();

    let bundle = db.load_aim_trial_bundle(&trial_id).unwrap();

    assert_eq!(bundle.trial, expected_trial);
    assert_eq!(
        bundle.target_events,
        vec![target_events[1].clone(), target_events[0].clone()]
    );
    assert_eq!(bundle.shots, vec![shots[1].clone(), shots[0].clone()]);
    assert_eq!(
        bundle.camera_samples,
        vec![camera_samples[1].clone(), camera_samples[0].clone()]
    );
    assert!(db.load_aim_trial_bundle("missing").is_err());
}

#[test]
fn load_aim_trial_analysis_bundle_includes_ordered_inputs() {
    let db = TelemetryDb::open(Path::new(":memory:")).unwrap();
    db.migrate().unwrap();

    let trial = sample_aim_trial("completed");
    let input_samples = vec![
        AimInputSampleRecord {
            timestamp_ns: 1_200_000_000,
            sequence_number: 1,
            raw_dx: 2,
            raw_dy: 0,
            processed_dx: 2.0,
            processed_dy: 0.0,
            dt_ns: 100_000_000,
            dt_used_ns: 100_000_000,
            input_speed: Some(2.0),
            acceleration_scale: Some(1.1),
        },
        AimInputSampleRecord {
            timestamp_ns: 1_100_000_000,
            sequence_number: 0,
            raw_dx: 1,
            raw_dy: 0,
            processed_dx: 1.0,
            processed_dy: 0.0,
            dt_ns: 0,
            dt_used_ns: 0,
            input_speed: None,
            acceleration_scale: None,
        },
    ];
    let trial_id = db
        .insert_completed_aim_trial("20260920", &trial, &[], &[], &input_samples, &[])
        .unwrap();

    let bundle = db.load_aim_trial_analysis_bundle(&trial_id).unwrap();
    assert_eq!(bundle.input_samples.len(), 2);
    assert_eq!(bundle.input_samples[0].sequence_number, 0);
    assert_eq!(bundle.input_samples[0].timestamp_ns, 1_100_000_000);
    assert_eq!(bundle.input_samples[1].sequence_number, 1);
    assert_eq!(
        bundle.input_samples[1].acceleration_scale,
        Some(1.1)
    );
    assert!(db.load_aim_trial_analysis_bundle("missing").is_err());
}

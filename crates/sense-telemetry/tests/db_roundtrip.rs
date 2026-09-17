use std::path::Path;

use sense_telemetry::{SessionBuffers, TelemetryDb};
use sense_types::{
    AccelerationConfig, ConfigurationRecord, DisplayConfig, FovAxis, FrameSample,
    InputCameraSample, InputIntegrityReport, MouseSample, SensitivityConfig, SessionRecord,
    ValidationResult,
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

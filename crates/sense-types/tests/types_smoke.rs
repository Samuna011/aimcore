use sense_math::{edpi, VALORANT_YAW_DEG_PER_COUNT_AT_SENS_1};
use sense_types::{
    AccelerationConfig, ConfigurationRecord, DisplayConfig, FovAxis, InputCameraSample,
    InputIntegrityReport, InputProcessor, MouseSample, NoAcceleration, SensitivityConfig,
    SessionRecord, ValidationResult,
};

#[test]
fn mouse_sample_roundtrip() {
    let sample = MouseSample {
        timestamp_ns: 1_000_000,
        dx: 42,
        dy: -7,
        buttons: 0b001,
        sequence_number: 1,
    };
    let json = serde_json::to_string(&sample).expect("serialize");
    let back: MouseSample = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.dx, 42);
    assert_eq!(back.sequence_number, 1);
}

#[test]
fn input_camera_sample_roundtrip() {
    let sample = InputCameraSample {
        timestamp_ns: 2_000_000,
        yaw_deg: 12.25,
        pitch_deg: 0.0,
        sequence_number: 2,
    };
    let json = serde_json::to_string(&sample).expect("serialize");
    let back: InputCameraSample = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.yaw_deg, 12.25);
    assert_eq!(back.sequence_number, 2);
}

#[test]
fn validation_result_constructs() {
    let integrity = InputIntegrityReport {
        samples_received: 100,
        sequence_gaps: 0,
        duplicate_sequences: 0,
        out_of_order_samples: 0,
        timestamp_regressions: 0,
    };
    assert!(!integrity.is_pipeline_suspect());

    let result = ValidationResult {
        expected_counts: 29_387.755,
        observed_net_counts: 29_400.0,
        observed_abs_path_counts: 29_500.0,
        expected_degrees: 360.0,
        observed_degrees: 360.15,
        count_difference: 12.245,
        error_percent: 0.0418,
        integrity,
    };
    assert_eq!(result.expected_degrees, 360.0);
}

#[test]
fn no_acceleration_is_identity() {
    let mut proc = NoAcceleration;
    assert_eq!(proc.process(10.0, -3.0, 0.016), (10.0, -3.0));
}

#[test]
fn configuration_and_session_records_roundtrip() {
    let cfg = ConfigurationRecord {
        id: "config_000001".into(),
        sensitivity: SensitivityConfig {
            dpi: 1600.0,
            sensitivity: 0.175,
            yaw_deg_per_count_at_sens_1: VALORANT_YAW_DEG_PER_COUNT_AT_SENS_1,
            fov_axis: FovAxis::Horizontal,
            fov_degrees: 103.0,
        },
        edpi: edpi(1600.0, 0.175),
        display: DisplayConfig {
            width: 1920,
            height: 1080,
            refresh_hz: 144.0,
        },
        accel: AccelerationConfig {
            enabled: false,
            model: "none".into(),
        },
        polling_rate_hz: Some(1000.0),
    };
    let json = serde_json::to_string(&cfg).expect("serialize config");
    let _: ConfigurationRecord = serde_json::from_str(&json).expect("deserialize config");

    let session = SessionRecord {
        id: "session_20260917_000001".into(),
        app_version: "0.1.0".into(),
        experiment_id: "validation_lab".into(),
        experiment_version: "0.1.0".into(),
        config_id: cfg.id.clone(),
        seed: 0,
        start_unix_ms: 1_757_961_600_000,
        end_unix_ms: None,
    };
    let json = serde_json::to_string(&session).expect("serialize session");
    let _: SessionRecord = serde_json::from_str(&json).expect("deserialize session");
}

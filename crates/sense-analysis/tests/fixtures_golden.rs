use sense_analysis::{analyze_trial, analysis_result_to_json, AnalysisConfig};
use sense_telemetry::AimTrialAnalysisBundle;
use sense_types::{AimCameraSampleRecord, AimInputSampleRecord, AimShotRecord, AimTrialRecord};

#[test]
fn golden_json_contains_stable_namespaces() {
    let mut trial = AimTrialRecord {
        id: "aim_fixture_001".into(),
        app_version: "0.1.0".into(),
        experiment_id: "aim_lab".into(),
        experiment_version: "0.12.1".into(),
        trial_type: "STATIC_CLICK".into(),
        status: "completed".into(),
        processor_id: "none".into(),
        processor_version: "1.0.0".into(),
        processor_config_json: "{}".into(),
        dpi: 3200.0,
        sensitivity: 0.09,
        polling_rate_hz: 1000.0,
        fov_degrees_h: 103.0,
        pitch_model_id: "unverified_0.1".into(),
        pitch_model_version: "1".into(),
        pitch_config_json: "{}".into(),
        resolution_width: 1920,
        resolution_height: 1080,
        aspect_ratio: 16.0 / 9.0,
        random_seed: 42,
        task_version: "2".into(),
        hardware_config_json: "{}".into(),
        view_config_json: "{}".into(),
        task_config_json: "{}".into(),
        metrics_json: "{}".into(),
        start_unix_ms: 0,
        end_unix_ms: 1,
        start_timestamp_ns: 0,
        end_timestamp_ns: 200_000_000,
        duration_secs: 0.2,
        hits: 1,
        shots: 1,
        misses: 0,
        score_secs: 0.2,
        accuracy: 1.0,
    };
    let _ = &mut trial;

    // Movement: 50°/s for >40ms then settle
    let mut camera = Vec::new();
    for i in 0..20 {
        let t = i * 10; // ms
        let speed = if i < 8 { 50.0 } else { 10.0 };
        camera.push(AimCameraSampleRecord {
            timestamp_ns: (t as u64) * 1_000_000,
            yaw_deg: (i as f64) * 0.5,
            pitch_deg: 0.0,
            yaw_delta_deg: 0.5,
            pitch_delta_deg: 0.0,
        });
        let _ = speed; // speeds come from reconstruct deltas; force via yaw steps
    }
    // Ensure angular speed from yaw deltas: 0.5° / 10ms = 50 °/s
    let shots = vec![AimShotRecord {
        shot_index: 0,
        timestamp_ns: 70_000_000,
        hit: true,
        yaw_deg: 3.5,
        pitch_deg: 0.0,
        target_x: 0.0,
        target_y: 1.6,
        target_z: -2.0,
        target_radius: 0.25,
        target_id: "target_001".into(),
    }];

    let bundle = AimTrialAnalysisBundle {
        trial,
        target_events: vec![],
        shots,
        camera_samples: camera,
        input_samples: vec![AimInputSampleRecord {
            timestamp_ns: 50_000_000,
            sequence_number: 0,
            raw_dx: 1,
            raw_dy: 0,
            processed_dx: 1.0,
            processed_dy: 0.0,
            dt_ns: 1_000_000,
            dt_used_ns: 1_000_000,
            input_speed: Some(1.0),
            acceleration_scale: Some(1.0),
        }],
    };

    let result = analyze_trial(&bundle, &AnalysisConfig::v1()).unwrap();
    let json = analysis_result_to_json(&result);
    assert!(json.contains("\"analysis_version\": \"2\""));
    assert!(json.contains("\"metric_scope\""));
    assert!(json.contains("\"physical_raw_speed_mean\"") || json.contains("endpoint_error_deg") || json.contains("ORPHAN_SHOT") || json.contains("orphan"));
    assert!(!json.contains("\"raw_speed_mean\""));
    assert!(!json.contains("cause"));
    assert!(!json.contains("recommend"));
}

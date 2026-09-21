use sense_analysis::geometry::{endpoint_error_yaw_pitch, wrap_to_180};
use sense_analysis::reconstruct::reconstruct;
use sense_analysis::AnalysisConfig;
use sense_telemetry::AimTrialAnalysisBundle;
use sense_types::{AimCameraSampleRecord, AimTrialRecord};

fn bare_trial() -> AimTrialRecord {
    AimTrialRecord {
        id: "t".into(),
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
        pitch_model_id: "x".into(),
        pitch_model_version: "1".into(),
        pitch_config_json: "{}".into(),
        resolution_width: 1920,
        resolution_height: 1080,
        aspect_ratio: 16.0 / 9.0,
        random_seed: 1,
        task_version: "2".into(),
        hardware_config_json: "{}".into(),
        view_config_json: "{}".into(),
        task_config_json: "{}".into(),
        metrics_json: "{}".into(),
        start_unix_ms: 0,
        end_unix_ms: 1,
        start_timestamp_ns: 0,
        end_timestamp_ns: 1,
        duration_secs: 1.0,
        hits: 0,
        shots: 0,
        misses: 0,
        score_secs: 0.0,
        accuracy: 0.0,
    }
}

#[test]
fn unwrap_179_to_neg179_is_two_degree_step_not_358() {
    let camera = vec![
        AimCameraSampleRecord {
            timestamp_ns: 0,
            yaw_deg: 179.0,
            pitch_deg: 0.0,
            yaw_delta_deg: 0.0,
            pitch_delta_deg: 0.0,
        },
        AimCameraSampleRecord {
            timestamp_ns: 10_000_000, // 10 ms
            yaw_deg: -179.0,
            pitch_deg: 0.0,
            yaw_delta_deg: 0.0,
            pitch_delta_deg: 0.0,
        },
    ];
    let bundle = AimTrialAnalysisBundle {
        trial: bare_trial(),
        target_events: vec![],
        shots: vec![],
        camera_samples: camera,
        input_samples: vec![],
    };
    let recon = reconstruct(&bundle, &AnalysisConfig::v1()).unwrap();
    assert!(
        (recon.camera[1].delta_yaw_deg - 2.0).abs() < 1e-6,
        "got {}",
        recon.camera[1].delta_yaw_deg
    );
    assert!(
        (recon.camera[1].angular_speed_deg_s - 200.0).abs() < 1.0,
        "got {}",
        recon.camera[1].angular_speed_deg_s
    );
    assert!((wrap_to_180(-179.0 - 179.0) - 2.0).abs() < 1e-9);
}

#[test]
fn constant_yaw_rate_reconstructs_expected_speed() {
    let camera = vec![
        AimCameraSampleRecord {
            timestamp_ns: 0,
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            yaw_delta_deg: 0.0,
            pitch_delta_deg: 0.0,
        },
        AimCameraSampleRecord {
            timestamp_ns: 100_000_000,
            yaw_deg: 10.0,
            pitch_deg: 0.0,
            yaw_delta_deg: 10.0,
            pitch_delta_deg: 0.0,
        },
    ];
    let bundle = AimTrialAnalysisBundle {
        trial: bare_trial(),
        target_events: vec![],
        shots: vec![],
        camera_samples: camera,
        input_samples: vec![],
    };
    let recon = reconstruct(&bundle, &AnalysisConfig::v1()).unwrap();
    assert!((recon.camera[1].angular_speed_deg_s - 100.0).abs() < 1.0);
}

#[test]
fn endpoint_past_target_is_positive_in_yaw_pitch() {
    let err_short = endpoint_error_yaw_pitch(0.0, 0.0, 10.0, 0.0, 5.0, 0.0);
    let err_past = endpoint_error_yaw_pitch(0.0, 0.0, 10.0, 0.0, 15.0, 0.0);
    assert!(err_short < 0.0, "{err_short}");
    assert!(err_past > 0.0, "{err_past}");
}

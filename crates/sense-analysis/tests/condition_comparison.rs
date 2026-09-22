use sense_analysis::{
    compare_trials, metric_scope_for_trial_type, AnalysisResult, BehaviorMetrics, ComparisonValidity,
    ExposureMetrics, MatchStatus, MovementCandidate, ShotAnalysis,
};
use sense_types::AimTrialRecord;

fn base_trial() -> AimTrialRecord {
    AimTrialRecord {
        id: "base".into(),
        app_version: "0.1.0".into(),
        experiment_id: "aim_lab".into(),
        experiment_version: "0.12.2".into(),
        trial_type: "GRIDSHOT".into(),
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
        random_seed: 1,
        task_version: "2".into(),
        hardware_config_json: "{}".into(),
        view_config_json: "{}".into(),
        task_config_json: r#"{"duration_secs":60}"#.into(),
        metrics_json: "{}".into(),
        start_unix_ms: 0,
        end_unix_ms: 1,
        start_timestamp_ns: 0,
        end_timestamp_ns: 60_000_000_000,
        duration_secs: 60.0,
        hits: 100,
        shots: 200,
        misses: 100,
        score_secs: 60.0,
        accuracy: 0.5,
    }
}

fn empty_analysis(trial: &AimTrialRecord) -> AnalysisResult {
    AnalysisResult {
        analysis_version: "2".into(),
        trial_id: trial.id.clone(),
        trial_type: trial.trial_type.clone(),
        experiment_version: trial.experiment_version.clone(),
        task_version: trial.task_version.clone(),
        processor_id: trial.processor_id.clone(),
        processor_version: trial.processor_version.clone(),
        metric_scope: metric_scope_for_trial_type(&trial.trial_type),
        candidates: Vec::<MovementCandidate>::new(),
        shots: Vec::<ShotAnalysis>::new(),
        trial_quality_flags: vec![],
        reconstruction_max_abs_step_deg: 0.0,
        reconstruction_max_angular_speed_deg_s: 0.0,
        reconstruction_discontinuity_count: 0,
        reconstruction_yaw_wrap_crossings: 0,
    }
}

#[test]
fn processor_only_diff_is_fair_match() {
    let mut a = base_trial();
    a.id = "a".into();
    a.processor_id = "none".into();
    let mut b = a.clone();
    b.id = "b".into();
    b.processor_id = "rawaccel_linear".into();
    b.processor_config_json = r#"{"gain":1.2}"#.into();
    // Realized duration may differ without breaking fairness.
    b.duration_secs = 59.9;
    b.score_secs = 59.9;
    let cmp = compare_trials(&a, &empty_analysis(&a), &b, &empty_analysis(&b));
    assert_eq!(cmp.match_status, MatchStatus::Matched);
    assert_eq!(cmp.comparison_validity, ComparisonValidity::FairMatch);
    assert!(cmp.condition_mismatches.is_empty());
}

#[test]
fn sensitivity_diff_is_intentional() {
    let a = base_trial();
    let mut b = a.clone();
    b.sensitivity = 0.10;
    let cmp = compare_trials(&a, &empty_analysis(&a), &b, &empty_analysis(&b));
    assert_eq!(cmp.match_status, MatchStatus::Mismatched);
    assert_eq!(
        cmp.comparison_validity,
        ComparisonValidity::IntentionalParameterDiff
    );
    assert!(cmp
        .condition_mismatches
        .iter()
        .any(|m| m.key == "sensitivity"));
}

#[test]
fn gridshot_vs_tracking_is_invalid_cross_task() {
    let mut a = base_trial();
    a.trial_type = "GRIDSHOT".into();
    let mut b = a.clone();
    b.trial_type = "TRACKING".into();
    let cmp = compare_trials(&a, &empty_analysis(&a), &b, &empty_analysis(&b));
    assert_eq!(
        cmp.comparison_validity,
        ComparisonValidity::InvalidCrossTask
    );
}

fn behavior_stub(endpoint_error_deg: f64) -> BehaviorMetrics {
    BehaviorMetrics {
        endpoint_error_deg,
        overshoot_deg: 0.0,
        movement_duration_ns: 100_000_000,
        path_length_deg: 1.0,
        path_efficiency: Some(1.0),
        angular_velocity_mean_deg_s: 10.0,
        angular_velocity_peak_deg_s: 20.0,
        angular_acceleration_peak_deg_s2: 1.0,
        jitter_rms_deg_s: 0.1,
        correction_magnitude_deg: Some(0.5),
        correction_duration_ns: Some(10_000_000),
    }
}

fn shot_with_behavior(index: u32, endpoint: f64) -> ShotAnalysis {
    ShotAnalysis {
        shot_index: index,
        hit: true,
        primary_movement_id: Some(0),
        correction_start_ns: None,
        correction_end_ns: None,
        behavior: Some(behavior_stub(endpoint)),
        exposure: Some(ExposureMetrics {
            physical_raw_speed_mean: 1.0,
            physical_raw_speed_peak: 2.0,
            physical_processed_speed_mean: 1.0,
            physical_processed_speed_peak: 2.0,
            processor_input_speed_mean: None,
            processor_input_speed_peak: None,
            acceleration_scale_mean: None,
            acceleration_scale_peak: None,
            gain_ratio_mean: 1.0,
            gain_ratio_peak: 1.0,
            cap_exposure: 0.0,
            cap_applicable: false,
            processor_time_ns: 1,
        }),
        movement_demand: None,
        processor_exposure: None,
        quality_flags: vec![],
    }
}

#[test]
fn behavior_mean_and_median_and_delta() {
    let a = base_trial();
    let b = base_trial();
    let mut aa = empty_analysis(&a);
    aa.shots = vec![
        shot_with_behavior(0, 1.0),
        shot_with_behavior(1, 3.0),
    ];
    let mut ab = empty_analysis(&b);
    ab.shots = vec![shot_with_behavior(0, 2.0)];
    let cmp = compare_trials(&a, &aa, &b, &ab);
    let ee = cmp
        .metric_deltas
        .iter()
        .find(|m| m.name == "endpoint_error_deg")
        .expect("endpoint_error_deg");
    assert_eq!(ee.n_a, 2);
    assert_eq!(ee.n_b, 1);
    assert!((ee.a_mean.unwrap() - 2.0).abs() < 1e-12);
    assert!((ee.a_median.unwrap() - 2.0).abs() < 1e-12);
    assert!((ee.b_mean.unwrap() - 2.0).abs() < 1e-12);
    assert!((ee.delta_mean.unwrap()).abs() < 1e-12);
}

#[test]
fn no_behavior_yields_null_metric_not_zero() {
    let a = base_trial();
    let b = base_trial();
    let cmp = compare_trials(&a, &empty_analysis(&a), &b, &empty_analysis(&b));
    let ee = cmp
        .metric_deltas
        .iter()
        .find(|m| m.name == "endpoint_error_deg")
        .expect("endpoint_error_deg");
    assert_eq!(ee.n_a, 0);
    assert_eq!(ee.n_b, 0);
    assert!(ee.a_mean.is_none());
    assert!(ee.b_mean.is_none());
    assert!(ee.delta_mean.is_none());
}

#[test]
fn trial_level_shot_accuracy_delta() {
    let mut a = base_trial();
    a.accuracy = 0.5;
    let mut b = base_trial();
    b.accuracy = 0.8;
    let cmp = compare_trials(&a, &empty_analysis(&a), &b, &empty_analysis(&b));
    let m = cmp
        .metric_deltas
        .iter()
        .find(|m| m.name == "shot_accuracy")
        .expect("shot_accuracy");
    assert_eq!(m.n_a, 1);
    assert!((m.a_mean.unwrap() - 0.5).abs() < 1e-12);
    assert!((m.b_mean.unwrap() - 0.8).abs() < 1e-12);
    assert!((m.delta_mean.unwrap() - 0.3).abs() < 1e-12);
}

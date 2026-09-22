use sense_analysis::{
    compare_trials, compare_trials_with_demand, metric_scope_for_trial_type, BehaviorMetrics,
    DemandCompareConfig, DemandStratum, ExposureMetrics, MovementCandidate, MovementDemand,
    ProcessorExposure, ShotAnalysis, AnalysisResult,
};
use sense_types::AimTrialRecord;

fn base_trial() -> AimTrialRecord {
    AimTrialRecord {
        id: "t".into(),
        app_version: "0.1.0".into(),
        experiment_id: "aim_lab".into(),
        experiment_version: "0.14.0".into(),
        trial_type: "FLICK_DEMAND".into(),
        status: "completed".into(),
        processor_id: "none".into(),
        processor_version: "1".into(),
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
        task_version: "1".into(),
        hardware_config_json: "{}".into(),
        view_config_json: "{}".into(),
        task_config_json: "{}".into(),
        metrics_json: "{}".into(),
        start_unix_ms: 0,
        end_unix_ms: 1,
        start_timestamp_ns: 0,
        end_timestamp_ns: 1,
        duration_secs: 60.0,
        hits: 10,
        shots: 20,
        misses: 10,
        score_secs: 60.0,
        accuracy: 0.5,
    }
}

fn behavior(endpoint: f64) -> BehaviorMetrics {
    BehaviorMetrics {
        endpoint_error_deg: endpoint,
        overshoot_deg: 0.0,
        movement_duration_ns: 100_000_000,
        path_length_deg: 30.0,
        path_efficiency: Some(1.0),
        angular_velocity_mean_deg_s: 100.0,
        angular_velocity_peak_deg_s: 200.0,
        angular_acceleration_peak_deg_s2: 1000.0,
        jitter_rms_deg_s: 0.1,
        correction_magnitude_deg: None,
        correction_duration_ns: None,
    }
}

fn shot(index: u32, commanded: f64, endpoint: f64) -> ShotAnalysis {
    ShotAnalysis {
        shot_index: index,
        hit: true,
        primary_movement_id: Some(0),
        correction_start_ns: None,
        correction_end_ns: None,
        behavior: Some(behavior(endpoint)),
        exposure: None,
        movement_demand: Some(MovementDemand {
            angular_displacement_deg: commanded.abs(),
            target_distance_deg: Some(commanded.abs()),
            commanded_yaw_deg: Some(commanded),
            peak_physical_input_speed: Some(1.0),
            movement_duration_ms: 100.0,
        }),
        processor_exposure: Some(ProcessorExposure {
            peak_processed_speed: Some(1.0),
            peak_accel_scale: None,
        }),
        quality_flags: vec![],
    }
}

fn analysis(trial: &AimTrialRecord, shots: Vec<ShotAnalysis>) -> AnalysisResult {
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
        shots,
        trial_quality_flags: vec![],
        reconstruction_max_abs_step_deg: 0.0,
        reconstruction_max_angular_speed_deg_s: 0.0,
        reconstruction_discontinuity_count: 0,
        reconstruction_yaw_wrap_crossings: 0,
    }
}

#[test]
fn comparison_version_is_2() {
    let a = base_trial();
    let b = a.clone();
    let cmp = compare_trials(&a, &analysis(&a, vec![]), &b, &analysis(&b, vec![]));
    assert_eq!(cmp.comparison_version, "2");
}

#[test]
fn default_compare_has_no_strata_but_notes_empty_commanded() {
    let a = base_trial();
    let b = a.clone();
    let cmp = compare_trials(&a, &analysis(&a, vec![]), &b, &analysis(&b, vec![]));
    assert!(cmp.demand_strata.is_empty());
    assert!(cmp
        .demand_distribution_notes
        .iter()
        .any(|n| n.contains("no commanded_yaw_deg")));
}

#[test]
fn commanded_abs_yaw_strata_separates_30_and_60() {
    let mut a = base_trial();
    a.id = "a".into();
    a.processor_id = "none".into();
    let mut b = a.clone();
    b.id = "b".into();
    b.processor_id = "rawaccel_linear".into();

    let aa = analysis(
        &a,
        vec![
            shot(0, 30.0, 1.0),
            shot(1, -30.0, 3.0),
            shot(2, 60.0, 5.0),
        ],
    );
    let ab = analysis(
        &b,
        vec![
            shot(0, 30.0, 2.0),
            shot(1, 30.0, 4.0),
            shot(2, -60.0, 8.0),
        ],
    );

    let cmp = compare_trials_with_demand(
        &a,
        &aa,
        &b,
        &ab,
        &DemandCompareConfig {
            stratum: DemandStratum::CommandedAbsYaw,
        },
    );

    assert_eq!(cmp.demand_strata.len(), 2);
    let s30 = cmp
        .demand_strata
        .iter()
        .find(|s| s.key == "commanded_abs_yaw:30")
        .expect("30");
    let s60 = cmp
        .demand_strata
        .iter()
        .find(|s| s.key == "commanded_abs_yaw:60")
        .expect("60");
    assert_eq!(s30.n_a, 2);
    assert_eq!(s30.n_b, 2);
    assert_eq!(s60.n_a, 1);
    assert_eq!(s60.n_b, 1);
    let m30 = &s30.metric_deltas[0];
    assert!((m30.a_mean.unwrap() - 2.0).abs() < 1e-9); // (1+3)/2
    assert!((m30.b_mean.unwrap() - 3.0).abs() < 1e-9); // (2+4)/2
}

#[test]
fn histogram_mismatch_note_when_demand_mix_differs() {
    let a = base_trial();
    let b = a.clone();
    let aa = analysis(&a, vec![shot(0, 10.0, 1.0)]);
    let ab = analysis(&b, vec![shot(0, 90.0, 1.0)]);
    let cmp = compare_trials(&a, &aa, &b, &ab);
    assert!(cmp
        .demand_distribution_notes
        .iter()
        .any(|n| n.contains("histogram mismatch")));
}

#[test]
fn processor_exposure_not_required_for_stratum_key() {
    // Same commanded yaw, different peak_accel_scale — still one stratum.
    let a = base_trial();
    let b = a.clone();
    let mut sa = shot(0, 60.0, 1.0);
    sa.processor_exposure = Some(ProcessorExposure {
        peak_processed_speed: Some(1.0),
        peak_accel_scale: None,
    });
    let mut sb = shot(0, 60.0, 2.0);
    sb.processor_exposure = Some(ProcessorExposure {
        peak_processed_speed: Some(5.0),
        peak_accel_scale: Some(2.0),
    });
    let cmp = compare_trials_with_demand(
        &a,
        &analysis(&a, vec![sa]),
        &b,
        &analysis(&b, vec![sb]),
        &DemandCompareConfig {
            stratum: DemandStratum::CommandedAbsYaw,
        },
    );
    assert_eq!(cmp.demand_strata.len(), 1);
    assert_eq!(cmp.demand_strata[0].key, "commanded_abs_yaw:60");
    let _ = ExposureMetrics {
        physical_raw_speed_mean: 0.0,
        physical_raw_speed_peak: 0.0,
        physical_processed_speed_mean: 0.0,
        physical_processed_speed_peak: 0.0,
        processor_input_speed_mean: None,
        processor_input_speed_peak: None,
        acceleration_scale_mean: None,
        acceleration_scale_peak: None,
        gain_ratio_mean: 1.0,
        gain_ratio_peak: 1.0,
        cap_exposure: 0.0,
        cap_applicable: false,
        processor_time_ns: 0,
    };
}

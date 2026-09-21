//! M4.1 freeze validation: unwrap, duration, multi-shot, kinematics.

use sense_analysis::geometry::{unwrap_yaw_series, wrap_to_180};
use sense_analysis::quality::QualityFlag;
use sense_analysis::reconstruct::reconstruct;
use sense_analysis::segment::segment;
use sense_analysis::{analyze_trial, AnalysisConfig};
use sense_telemetry::AimTrialAnalysisBundle;
use sense_types::{AimCameraSampleRecord, AimShotRecord, AimTrialRecord};

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

fn cam_sample(t_ms: u64, yaw: f64, pitch: f64) -> AimCameraSampleRecord {
    AimCameraSampleRecord {
        timestamp_ns: t_ms * 1_000_000,
        yaw_deg: yaw,
        pitch_deg: pitch,
        yaw_delta_deg: 0.0,
        pitch_delta_deg: 0.0,
    }
}

fn shot_at(idx: u32, t_ms: u64) -> AimShotRecord {
    AimShotRecord {
        shot_index: idx,
        timestamp_ns: t_ms * 1_000_000,
        hit: true,
        yaw_deg: 0.0,
        pitch_deg: 0.0,
        target_x: 0.0,
        target_y: 1.6,
        target_z: -2.0,
        target_radius: 0.25,
        target_id: format!("t{idx}"),
    }
}

// --- 1. Yaw unwrapping -------------------------------------------------------

#[test]
fn yaw_unwrap_dedicated_seam_sequence() {
    // Physical +0.5, +0.7, +0.6 across the ±180 seam — not −359.
    let wrapped = [179.0, 179.5, -179.8, -179.2];
    let u = unwrap_yaw_series(&wrapped);
    assert!((u[0] - 179.0).abs() < 1e-9);
    assert!((u[1] - 179.5).abs() < 1e-9);
    assert!((u[2] - 180.2).abs() < 1e-6, "got {}", u[2]);
    assert!((u[3] - 180.8).abs() < 1e-6, "got {}", u[3]);

    let naive_mid = wrapped[2] - wrapped[1];
    assert!(naive_mid < -300.0, "naïve delta should look like wrap artifact");
    assert!((wrap_to_180(naive_mid) - 0.7).abs() < 1e-6);
}

#[test]
fn yaw_unwrap_via_reconstruct_reports_wrap_crossing_and_sane_speed() {
    let camera = vec![
        cam_sample(0, 179.0, 0.0),
        cam_sample(10, 179.5, 0.0),  // +0.5° / 10ms = 50 °/s
        cam_sample(20, -179.8, 0.0), // +0.7° / 10ms = 70 °/s
        cam_sample(30, -179.2, 0.0), // +0.6° / 10ms = 60 °/s
    ];
    let bundle = AimTrialAnalysisBundle {
        trial: bare_trial(),
        target_events: vec![],
        shots: vec![],
        camera_samples: camera,
        input_samples: vec![],
    };
    let recon = reconstruct(&bundle, &AnalysisConfig::v1()).unwrap();
    assert_eq!(recon.stats.yaw_wrap_crossings, 1);
    assert_eq!(recon.stats.discontinuity_count, 0);
    assert!((recon.camera[2].delta_yaw_deg - 0.7).abs() < 1e-6);
    assert!((recon.camera[2].angular_speed_deg_s - 70.0).abs() < 1.0);
    // Naïve would be ~35930 °/s — must not appear.
    assert!(recon.stats.max_angular_speed_deg_s < 200.0);
}

// --- 2. movement_duration_ns = 0 / first movement -----------------------------

#[test]
fn first_flick_into_click_has_nonzero_duration() {
    // Quiet, then a real flick ending at the click sample (speed stamped on click).
    // Acquisition must start at the *previous* sample (segment start), not the click.
    let mut camera = vec![cam_sample(0, 0.0, 0.0)];
    // settle for >80ms
    for t in 1..100 {
        camera.push(cam_sample(t, 0.0, 0.0));
    }
    // flick: 5° over 50ms → 100 °/s, last sample at click
    camera.push(cam_sample(110, 1.0, 0.0));
    camera.push(cam_sample(120, 2.0, 0.0));
    camera.push(cam_sample(130, 3.0, 0.0));
    camera.push(cam_sample(140, 4.0, 0.0));
    camera.push(cam_sample(150, 5.0, 0.0)); // click

    let bundle = AimTrialAnalysisBundle {
        trial: bare_trial(),
        target_events: vec![],
        shots: vec![AimShotRecord {
            shot_index: 0,
            timestamp_ns: 150_000_000,
            hit: true,
            yaw_deg: 5.0,
            pitch_deg: 0.0,
            target_x: 0.0,
            target_y: 1.6,
            target_z: -2.0,
            target_radius: 0.25,
            target_id: "t0".into(),
        }],
        camera_samples: camera,
        input_samples: vec![],
    };
    let result = analyze_trial(&bundle, &AnalysisConfig::v1()).unwrap();
    let b = result.shots[0].behavior.as_ref().expect("behavior");
    assert!(
        b.movement_duration_ns > 0,
        "first-movement duration must be > 0, got {}",
        b.movement_duration_ns
    );
    assert!(
        b.path_length_deg > 0.0,
        "expected nonzero path, got {}",
        b.path_length_deg
    );
}

// --- 3. MULTI_SHOT_MOVEMENT intentional --------------------------------------

#[test]
fn continuous_high_speed_with_two_clicks_is_one_multi_shot_candidate() {
    // One long bout above onset with two clicks inside — must not auto-split.
    let mut camera = Vec::new();
    for t in 0..300 {
        let speed_yaw = if (20..250).contains(&t) {
            // ~0.5° per ms → 500 °/s
            t as f64 * 0.5
        } else {
            camera.last().map(|c: &AimCameraSampleRecord| c.yaw_deg).unwrap_or(0.0)
        };
        camera.push(cam_sample(t, speed_yaw, 0.0));
    }
    // Force settle at end for clean candidate end
    for t in 250..300 {
        let yaw = camera[249].yaw_deg;
        camera[t as usize] = cam_sample(t, yaw, 0.0);
    }

    let recon = {
        let bundle = AimTrialAnalysisBundle {
            trial: bare_trial(),
            target_events: vec![],
            shots: vec![],
            camera_samples: camera.clone(),
            input_samples: vec![],
        };
        reconstruct(&bundle, &AnalysisConfig::v1()).unwrap()
    };

    let shots = vec![shot_at(0, 80), shot_at(1, 160)];
    let (cands, links) = segment(&recon, &shots, &AnalysisConfig::v1());
    assert!(
        cands.iter().any(|c| c.flags.contains(&QualityFlag::MultiShotMovement)),
        "expected MULTI_SHOT_MOVEMENT, cands={cands:?}"
    );
    let multi = cands
        .iter()
        .find(|c| c.flags.contains(&QualityFlag::MultiShotMovement))
        .unwrap();
    assert_eq!(links[0].primary_movement_id, Some(multi.id));
    assert_eq!(links[1].primary_movement_id, Some(multi.id));
    // Still one kinematic interval (not split into two candidates for the two shots).
    let containing = cands
        .iter()
        .filter(|c| {
            shots.iter().all(|s| s.timestamp_ns >= c.start_ns && s.timestamp_ns <= c.end_ns)
        })
        .count();
    assert_eq!(containing, 1);
}

// --- 4. Velocity / acceleration math -----------------------------------------

#[test]
fn angular_speed_and_acceleration_match_closed_form() {
    // Constant 2° yaw every 10ms → 200 °/s; then stop → accel magnitude 200/0.01 = 20000 °/s²
    let camera = vec![
        cam_sample(0, 0.0, 0.0),
        cam_sample(10, 2.0, 0.0),
        cam_sample(20, 4.0, 0.0),
        cam_sample(30, 6.0, 0.0),
        cam_sample(40, 6.0, 0.0), // Δyaw=0 → speed 0
    ];
    let bundle = AimTrialAnalysisBundle {
        trial: bare_trial(),
        target_events: vec![],
        shots: vec![],
        camera_samples: camera,
        input_samples: vec![],
    };
    let recon = reconstruct(&bundle, &AnalysisConfig::v1()).unwrap();
    assert!((recon.camera[1].angular_speed_deg_s - 200.0).abs() < 1e-6);
    assert!((recon.camera[2].angular_speed_deg_s - 200.0).abs() < 1e-6);
    assert!((recon.camera[3].angular_speed_deg_s - 200.0).abs() < 1e-6);
    assert!(recon.camera[4].angular_speed_deg_s.abs() < 1e-6);

    // Peak |d(speed)/dt| between samples 3→4: |0-200|/0.01s = 20000
    let dt_s = 0.01;
    let peak_acc = ((recon.camera[4].angular_speed_deg_s - recon.camera[3].angular_speed_deg_s)
        / dt_s)
        .abs();
    assert!((peak_acc - 20_000.0).abs() < 1.0, "got {peak_acc}");
}

#[test]
fn hypot_yaw_pitch_step_for_speed() {
    // 3° yaw and 4° pitch in 10ms → step 5°, speed 500 °/s
    let camera = vec![cam_sample(0, 0.0, 0.0), cam_sample(10, 3.0, 4.0)];
    let bundle = AimTrialAnalysisBundle {
        trial: bare_trial(),
        target_events: vec![],
        shots: vec![],
        camera_samples: camera,
        input_samples: vec![],
    };
    let recon = reconstruct(&bundle, &AnalysisConfig::v1()).unwrap();
    assert!((recon.camera[1].angular_speed_deg_s - 500.0).abs() < 1e-6);
}

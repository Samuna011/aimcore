use sense_analysis::quality::QualityFlag;
use sense_analysis::reconstruct::{CameraPoint, InputPoint, ReconstructedTrial};
use sense_analysis::segment::segment;
use sense_analysis::AnalysisConfig;
use sense_types::AimShotRecord;

fn cam(t_ms: u64, speed: f64) -> CameraPoint {
    CameraPoint {
        timestamp_ns: t_ms * 1_000_000,
        wrapped_yaw_deg: 0.0,
        pitch_deg: 0.0,
        unwrapped_yaw_deg: 0.0,
        delta_yaw_deg: 0.0,
        delta_pitch_deg: 0.0,
        angular_speed_deg_s: speed,
    }
}

fn recon_from_speeds(speeds: &[(u64, f64)]) -> ReconstructedTrial {
    ReconstructedTrial {
        camera: speeds.iter().map(|(t, s)| cam(*t, *s)).collect(),
        inputs: vec![InputPoint {
            timestamp_ns: 0,
            sequence_number: 0,
            raw_dx: 0,
            raw_dy: 0,
            processed_dx: 0.0,
            processed_dy: 0.0,
            dt_ns: 0,
            dt_used_ns: 0,
            physical_raw_speed: 0.0,
            physical_processed_speed: 0.0,
            acceleration_scale: None,
            processor_input_speed: None,
        }],
        stats: sense_analysis::reconstruct::ReconstructionStats {
            max_abs_step_deg: 0.0,
            max_angular_speed_deg_s: 0.0,
            timestamp_monotonic: true,
            discontinuity_count: 0,
            yaw_wrap_crossings: 0,
        },
    }
}

#[test]
fn onset_and_settle_hysteresis() {
    // Quiet, burst above onset, then settle below settle for 80ms+
    let recon = recon_from_speeds(&[
        (0, 0.0),
        (10, 50.0),  // onset
        (20, 60.0),
        (30, 10.0),  // below settle — start clock
        (50, 10.0),
        (110, 10.0), // 80ms from 30 → settle
        (120, 0.0),
    ]);
    let (cands, _) = segment(&recon, &[], &AnalysisConfig::v1());
    assert_eq!(cands.len(), 1);
    assert_eq!(cands[0].start_ns, 10_000_000);
    assert!(cands[0].end_ns >= 110_000_000);
}

#[test]
fn settle_clock_resets_when_speed_rises() {
    let recon = recon_from_speeds(&[
        (0, 50.0),
        (10, 10.0), // settle start
        (40, 20.0), // reset (>= settle 15)
        (50, 10.0),
        (130, 10.0), // need 80ms from 50
    ]);
    let (cands, _) = segment(&recon, &[], &AnalysisConfig::v1());
    assert_eq!(cands.len(), 1);
    assert!(cands[0].end_ns >= 130_000_000);
}

#[test]
fn min_duration_rejects_short_candidates() {
    let recon = recon_from_speeds(&[
        (0, 50.0),
        (10, 10.0),
        (30, 10.0), // only 30ms total if settles early — may be short
    ]);
    let (cands, _) = segment(&recon, &[], &AnalysisConfig::v1());
    // 30ms < 40ms min → rejected
    assert!(cands.is_empty());
}

#[test]
fn orphan_shot_when_no_candidate() {
    let recon = recon_from_speeds(&[(0, 0.0), (100, 0.0)]);
    let shots = vec![AimShotRecord {
        shot_index: 0,
        timestamp_ns: 50_000_000,
        hit: false,
        yaw_deg: 0.0,
        pitch_deg: 0.0,
        target_x: 0.0,
        target_y: 1.6,
        target_z: -2.0,
        target_radius: 0.25,
        target_id: "t".into(),
    }];
    let (_, links) = segment(&recon, &shots, &AnalysisConfig::v1());
    assert!(links[0].flags.contains(&QualityFlag::OrphanShot));
    assert!(links[0].primary_movement_id.is_none());
}

#[test]
fn multi_shot_flag_on_shared_candidate() {
    let recon = recon_from_speeds(&[
        (0, 50.0),
        (20, 50.0),
        (40, 50.0),
        (60, 10.0),
        (150, 10.0),
    ]);
    let shots = vec![
        AimShotRecord {
            shot_index: 0,
            timestamp_ns: 30_000_000,
            hit: true,
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            target_x: 0.0,
            target_y: 1.6,
            target_z: -2.0,
            target_radius: 0.25,
            target_id: "a".into(),
        },
        AimShotRecord {
            shot_index: 1,
            timestamp_ns: 50_000_000,
            hit: false,
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            target_x: 0.0,
            target_y: 1.6,
            target_z: -2.0,
            target_radius: 0.25,
            target_id: "b".into(),
        },
    ];
    let (cands, links) = segment(&recon, &shots, &AnalysisConfig::v1());
    assert_eq!(cands.len(), 1);
    assert!(cands[0].flags.contains(&QualityFlag::MultiShotMovement));
    assert_eq!(links[0].primary_movement_id, Some(0));
    assert_eq!(links[1].primary_movement_id, Some(0));
}

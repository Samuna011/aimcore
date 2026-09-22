//! ONE_WALL_SIX: six small targets on a wide wall (1w6t), 60s click scoring.

use bevy::prelude::Vec3;

use crate::aim_gridshot::nearest_live_target_for_miss;
use crate::aim_trial::{
    active_elapsed_ns, format_target_id, lcg_next_unit, look_direction_neg_z, AimPhase,
    AimRunConfigSnapshot, AimTaskKind, AimTrial, LiveAimTarget, AIM_CAMERA_ORIGIN,
};
use crate::camera_ctrl::YawPitch;
use crate::config::ValidationState;
use sense_types::{AimShotRecord, AimTargetEventRecord};

pub const ONE_WALL_SIX_TRIAL_TYPE: &str = "ONE_WALL_SIX";
pub const ONE_WALL_SIX_TASK_VERSION: &str = "1";
pub const ONE_WALL_SIX_DURATION_SECS: f64 = 60.0;
pub const ONE_WALL_SIX_CONCURRENT: usize = 6;
pub const ONE_WALL_SIX_RADIUS: f32 = 0.12;
pub const ONE_WALL_SIX_Z: f32 = -8.0;
/// World AABB chosen for ~±40° yaw / ±22° pitch from camera at z=4 looking to z=-8.
pub const ONE_WALL_SIX_X_MIN: f32 = -10.0;
pub const ONE_WALL_SIX_X_MAX: f32 = 10.0;
pub const ONE_WALL_SIX_Y_MIN: f32 = 0.5;
pub const ONE_WALL_SIX_Y_MAX: f32 = 5.5;
pub const ONE_WALL_SIX_MIN_SEPARATION: f32 = ONE_WALL_SIX_RADIUS * 2.5;

pub fn one_wall_six_task_config_json() -> String {
    format!(
        r#"{{"duration_secs":{},"concurrent_targets":{},"radius":{},"wall_z":{},"x_min":{},"x_max":{},"y_min":{},"y_max":{},"min_separation":{},"scoring_rule":"click","rng":"lcg","rng_version":"1"}}"#,
        ONE_WALL_SIX_DURATION_SECS,
        ONE_WALL_SIX_CONCURRENT,
        ONE_WALL_SIX_RADIUS,
        ONE_WALL_SIX_Z,
        ONE_WALL_SIX_X_MIN,
        ONE_WALL_SIX_X_MAX,
        ONE_WALL_SIX_Y_MIN,
        ONE_WALL_SIX_Y_MAX,
        ONE_WALL_SIX_MIN_SEPARATION,
    )
}

fn next_unit(rng: &mut u64) -> f64 {
    lcg_next_unit(rng)
}

pub fn sample_wall_point(rng: &mut u64) -> Vec3 {
    let x = ONE_WALL_SIX_X_MIN
        + (ONE_WALL_SIX_X_MAX - ONE_WALL_SIX_X_MIN) * next_unit(rng) as f32;
    let y = ONE_WALL_SIX_Y_MIN
        + (ONE_WALL_SIX_Y_MAX - ONE_WALL_SIX_Y_MIN) * next_unit(rng) as f32;
    Vec3::new(x, y, ONE_WALL_SIX_Z)
}

pub fn point_is_separated(candidate: Vec3, occupied: &[Vec3], min_sep: f32) -> bool {
    occupied
        .iter()
        .all(|c| candidate.distance(*c) >= min_sep)
}

/// Resample until separated or attempts exhausted (then accept last).
pub fn pick_wall_point(rng: &mut u64, occupied: &[Vec3]) -> Vec3 {
    let mut last = sample_wall_point(rng);
    for _ in 0..64 {
        last = sample_wall_point(rng);
        if point_is_separated(last, occupied, ONE_WALL_SIX_MIN_SEPARATION) {
            return last;
        }
    }
    last
}

fn push_spawn(trial: &mut AimTrial, live: &LiveAimTarget, timestamp_ns: u64) {
    trial.target_events.push(AimTargetEventRecord {
        target_id: live.target_id.clone(),
        event_index: trial.target_events.len() as u32,
        timestamp_ns,
        event_type: "spawn".into(),
        position_x: live.center.x as f64,
        position_y: live.center.y as f64,
        position_z: live.center.z as f64,
        yaw_deg: None,
        pitch_deg: None,
        velocity_x: 0.0,
        velocity_y: 0.0,
        velocity_z: 0.0,
        event_data_json: format!(r#"{{"radius":{}}}"#, ONE_WALL_SIX_RADIUS),
    });
}

fn push_despawn(trial: &mut AimTrial, live: &LiveAimTarget, timestamp_ns: u64) {
    trial.target_events.push(AimTargetEventRecord {
        target_id: live.target_id.clone(),
        event_index: trial.target_events.len() as u32,
        timestamp_ns,
        event_type: "despawn".into(),
        position_x: live.center.x as f64,
        position_y: live.center.y as f64,
        position_z: live.center.z as f64,
        yaw_deg: None,
        pitch_deg: None,
        velocity_x: 0.0,
        velocity_y: 0.0,
        velocity_z: 0.0,
        event_data_json: "{}".into(),
    });
}

fn alloc_live(trial: &mut AimTrial, center: Vec3) -> LiveAimTarget {
    let id = format_target_id(trial.next_target_ordinal);
    trial.next_target_ordinal = trial.next_target_ordinal.saturating_add(1);
    LiveAimTarget {
        target_id: id,
        row: 0,
        col: 0,
        center,
    }
}

pub fn start_one_wall_six_trial(
    pose: &mut YawPitch,
    trial: &mut AimTrial,
    validation: ValidationState,
    now_ns: u64,
    start_unix_ms: i64,
    random_seed: u64,
    config: AimRunConfigSnapshot,
) -> bool {
    if validation.is_running() || trial.phase == AimPhase::Armed {
        return false;
    }
    pose.yaw_deg = 0.0;
    pose.pitch_deg = 0.0;
    trial.random_seed = random_seed;
    trial.rng_state = random_seed;
    trial.task_kind = AimTaskKind::OneWallSix;
    trial.live_targets.clear();
    trial.phase = AimPhase::Armed;
    trial.hits = 0;
    trial.last_hit = None;
    trial.score_secs = None;
    trial.shot_log.clear();
    trial.target_events.clear();
    trial.clear_sample_logs();
    trial.next_target_ordinal = 1;
    trial.start_timestamp_ns = now_ns;
    trial.accumulated_pause_ns = 0;
    trial.paused_at_qpc = None;
    trial.start_unix_ms = start_unix_ms;
    trial.config_snapshot = Some(config);

    let mut occupied = Vec::new();
    for _ in 0..ONE_WALL_SIX_CONCURRENT {
        let center = pick_wall_point(&mut trial.rng_state, &occupied);
        occupied.push(center);
        let live = alloc_live(trial, center);
        push_spawn(trial, &live, now_ns);
        trial.live_targets.push(live);
    }
    if let Some(first) = trial.live_targets.first() {
        trial.current_center = first.center;
        trial.current_target_id = first.target_id.clone();
    }
    true
}

pub fn one_wall_six_should_end(trial: &AimTrial, now_ns: u64) -> bool {
    active_elapsed_ns(trial, now_ns) as f64 / 1e9 >= ONE_WALL_SIX_DURATION_SECS
}

pub fn finish_one_wall_six_trial(trial: &mut AimTrial, end_ns: u64) -> bool {
    if trial.phase != AimPhase::Armed || trial.task_kind != AimTaskKind::OneWallSix {
        return false;
    }
    let elapsed = active_elapsed_ns(trial, end_ns) as f64 / 1e9;
    trial.score_secs = Some(elapsed.min(ONE_WALL_SIX_DURATION_SECS));
    trial.last_timestamp_ns = end_ns;
    for live in trial.live_targets.clone() {
        push_despawn(trial, &live, end_ns);
    }
    trial.live_targets.clear();
    trial.phase = AimPhase::Idle;
    true
}

pub fn apply_one_wall_six_shot(trial: &mut AimTrial, pose: &YawPitch, timestamp_ns: u64) -> bool {
    if trial.phase != AimPhase::Armed || trial.task_kind != AimTaskKind::OneWallSix {
        return false;
    }
    if trial.paused_at_qpc.is_some() {
        return false;
    }
    let origin = [
        AIM_CAMERA_ORIGIN.x as f64,
        AIM_CAMERA_ORIGIN.y as f64,
        AIM_CAMERA_ORIGIN.z as f64,
    ];
    let dir = look_direction_neg_z(pose.yaw_deg, pose.pitch_deg);

    let mut hit_idx: Option<usize> = None;
    let mut best_t = f64::INFINITY;
    for (i, live) in trial.live_targets.iter().enumerate() {
        let center = [
            live.center.x as f64,
            live.center.y as f64,
            live.center.z as f64,
        ];
        if let Some(t) =
            sense_math::ray_sphere_hit_t(origin, dir, center, ONE_WALL_SIX_RADIUS as f64)
        {
            if t >= 0.0 && t < best_t {
                best_t = t;
                hit_idx = Some(i);
            }
        }
    }

    if hit_idx.is_none() {
        let nearest = nearest_live_target_for_miss(&trial.live_targets, origin, dir);
        let (tid, cx, cy, cz) = nearest
            .map(|l| {
                (
                    l.target_id.clone(),
                    l.center.x as f64,
                    l.center.y as f64,
                    l.center.z as f64,
                )
            })
            .unwrap_or_else(|| (String::new(), 0.0, 0.0, 0.0));
        trial.shot_log.push(AimShotRecord {
            shot_index: trial.shot_log.len() as u32,
            timestamp_ns,
            hit: false,
            yaw_deg: pose.yaw_deg,
            pitch_deg: pose.pitch_deg,
            target_x: cx,
            target_y: cy,
            target_z: cz,
            target_radius: ONE_WALL_SIX_RADIUS as f64,
            target_id: tid,
        });
        trial.last_hit = Some(false);
        trial.last_timestamp_ns = timestamp_ns;
        return false;
    }

    let idx = hit_idx.unwrap();
    let victim = trial.live_targets[idx].clone();
    trial.shot_log.push(AimShotRecord {
        shot_index: trial.shot_log.len() as u32,
        timestamp_ns,
        hit: true,
        yaw_deg: pose.yaw_deg,
        pitch_deg: pose.pitch_deg,
        target_x: victim.center.x as f64,
        target_y: victim.center.y as f64,
        target_z: victim.center.z as f64,
        target_radius: ONE_WALL_SIX_RADIUS as f64,
        target_id: victim.target_id.clone(),
    });
    trial.last_hit = Some(true);
    trial.last_timestamp_ns = timestamp_ns;
    trial.hits = trial.hits.saturating_add(1);

    push_despawn(trial, &victim, timestamp_ns);
    trial.live_targets.remove(idx);

    let occupied: Vec<Vec3> = trial.live_targets.iter().map(|t| t.center).collect();
    let center = pick_wall_point(&mut trial.rng_state, &occupied);
    let replacement = alloc_live(trial, center);
    push_spawn(trial, &replacement, timestamp_ns);
    trial.live_targets.push(replacement);
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aim_trial::{build_completed_aim_trial_record, AimRunConfigSnapshot};
    use crate::config::ExperimentSettings;

    fn test_config() -> AimRunConfigSnapshot {
        AimRunConfigSnapshot::from_live(
            &ExperimentSettings::default(),
            "none",
            "0.1.0",
            "{}",
            1920,
            1080,
        )
    }

    #[test]
    fn start_spawns_six_separated() {
        let mut pose = YawPitch::default();
        let mut trial = AimTrial::default();
        assert!(start_one_wall_six_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            1_000,
            1,
            42,
            test_config(),
        ));
        assert_eq!(trial.live_targets.len(), ONE_WALL_SIX_CONCURRENT);
        for i in 0..trial.live_targets.len() {
            for j in (i + 1)..trial.live_targets.len() {
                assert!(
                    trial.live_targets[i]
                        .center
                        .distance(trial.live_targets[j].center)
                        >= ONE_WALL_SIX_MIN_SEPARATION * 0.99
                );
            }
        }
    }

    #[test]
    fn same_seed_same_initial_centers() {
        let mut pa = YawPitch::default();
        let mut pb = YawPitch::default();
        let mut ta = AimTrial::default();
        let mut tb = AimTrial::default();
        assert!(start_one_wall_six_trial(
            &mut pa,
            &mut ta,
            ValidationState::Idle,
            1,
            1,
            99,
            test_config(),
        ));
        assert!(start_one_wall_six_trial(
            &mut pb,
            &mut tb,
            ValidationState::Idle,
            1,
            1,
            99,
            test_config(),
        ));
        for (a, b) in ta.live_targets.iter().zip(tb.live_targets.iter()) {
            assert!((a.center - b.center).length() < 1e-5);
        }
    }

    #[test]
    fn finishes_at_sixty_seconds() {
        let mut pose = YawPitch::default();
        let mut trial = AimTrial::default();
        let start = 5_000_000_000u64;
        assert!(start_one_wall_six_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            start,
            1,
            3,
            test_config(),
        ));
        assert!(one_wall_six_should_end(&trial, start + 60_000_000_000));
        assert!(finish_one_wall_six_trial(&mut trial, start + 60_000_000_000));
        let record = build_completed_aim_trial_record(&trial, 2, start + 60_000_000_000);
        assert_eq!(record.trial_type, ONE_WALL_SIX_TRIAL_TYPE);
        assert_eq!(record.task_version, ONE_WALL_SIX_TASK_VERSION);
    }
}

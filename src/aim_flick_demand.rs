//! FLICK_DEMAND: controlled commanded-yaw ladder for demand × config matrix.

use bevy::prelude::Vec3;

use crate::aim_trial::{
    active_elapsed_ns, format_target_id, front_cone_center, lcg_next_unit, look_direction_neg_z,
    AimPhase, AimRunConfigSnapshot, AimTaskKind, AimTrial, AIM_CAMERA_ORIGIN, AIM_DISTANCE,
    AIM_FLOOR_CLEARANCE, AIM_TARGET_RADIUS,
};
use crate::camera_ctrl::YawPitch;
use crate::config::ValidationState;
use sense_types::{AimShotRecord, AimTargetEventRecord};

pub const FLICK_DEMAND_TRIAL_TYPE: &str = "FLICK_DEMAND";
pub const FLICK_DEMAND_TASK_VERSION: &str = "1";
pub const FLICK_DEMAND_DURATION_SECS: f64 = 60.0;

/// Absolute yaw offsets (degrees) from world forward (−Z).
pub const FLICK_DEMAND_YAW_SET: [f64; 8] = [-90.0, -60.0, -30.0, -10.0, 10.0, 30.0, 60.0, 90.0];

pub fn flick_demand_task_config_json() -> String {
    format!(
        r#"{{"duration_secs":{},"radius":{},"aim_distance":{},"yaw_set":[-90,-60,-30,-10,10,30,60,90],"pitch_deg":0,"scoring_rule":"click","rng":"lcg","rng_version":"1"}}"#,
        FLICK_DEMAND_DURATION_SECS, AIM_TARGET_RADIUS, AIM_DISTANCE,
    )
}

pub fn flick_demand_center(yaw_deg: f64) -> Vec3 {
    let mut c = front_cone_center(yaw_deg, 0.0);
    c.y = c.y.max(AIM_FLOOR_CLEARANCE);
    c
}

/// Pick next yaw from the set; avoid immediate repeat of `prev` when possible.
pub fn next_commanded_yaw(rng: &mut u64, prev: Option<f64>) -> f64 {
    let mut choices: Vec<f64> = FLICK_DEMAND_YAW_SET.to_vec();
    if let Some(p) = prev {
        choices.retain(|y| (*y - p).abs() > 1e-9);
    }
    if choices.is_empty() {
        choices = FLICK_DEMAND_YAW_SET.to_vec();
    }
    let i = (lcg_next_unit(rng) * choices.len() as f64) as usize % choices.len();
    choices[i]
}

fn push_spawn(trial: &mut AimTrial, yaw_deg: f64, timestamp_ns: u64) {
    let event_index = trial.target_events.len() as u32;
    trial.target_events.push(AimTargetEventRecord {
        target_id: trial.current_target_id.clone(),
        event_index,
        timestamp_ns,
        event_type: "spawn".into(),
        position_x: trial.current_center.x as f64,
        position_y: trial.current_center.y as f64,
        position_z: trial.current_center.z as f64,
        yaw_deg: Some(yaw_deg),
        pitch_deg: Some(0.0),
        velocity_x: 0.0,
        velocity_y: 0.0,
        velocity_z: 0.0,
        event_data_json: format!(r#"{{"radius":{},"commanded_yaw_deg":{yaw_deg}}}"#, AIM_TARGET_RADIUS),
    });
}

fn push_despawn(trial: &mut AimTrial, timestamp_ns: u64) {
    let event_index = trial.target_events.len() as u32;
    trial.target_events.push(AimTargetEventRecord {
        target_id: trial.current_target_id.clone(),
        event_index,
        timestamp_ns,
        event_type: "despawn".into(),
        position_x: trial.current_center.x as f64,
        position_y: trial.current_center.y as f64,
        position_z: trial.current_center.z as f64,
        yaw_deg: None,
        pitch_deg: None,
        velocity_x: 0.0,
        velocity_y: 0.0,
        velocity_z: 0.0,
        event_data_json: "{}".into(),
    });
}

fn spawn_commanded(trial: &mut AimTrial, yaw_deg: f64, timestamp_ns: u64) {
    trial.current_target_id = format_target_id(trial.next_target_ordinal);
    trial.next_target_ordinal = trial.next_target_ordinal.saturating_add(1);
    trial.current_center = flick_demand_center(yaw_deg);
    trial.last_yaw_deg = yaw_deg;
    push_spawn(trial, yaw_deg, timestamp_ns);
}

pub fn start_flick_demand_trial(
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
    trial.task_kind = AimTaskKind::FlickDemand;
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
    let yaw = next_commanded_yaw(&mut trial.rng_state, None);
    spawn_commanded(trial, yaw, now_ns);
    true
}

pub fn flick_demand_should_end(trial: &AimTrial, now_ns: u64) -> bool {
    active_elapsed_ns(trial, now_ns) as f64 / 1e9 >= FLICK_DEMAND_DURATION_SECS
}

pub fn finish_flick_demand_trial(trial: &mut AimTrial, end_ns: u64) -> bool {
    if trial.phase != AimPhase::Armed || trial.task_kind != AimTaskKind::FlickDemand {
        return false;
    }
    let elapsed = active_elapsed_ns(trial, end_ns) as f64 / 1e9;
    trial.score_secs = Some(elapsed.min(FLICK_DEMAND_DURATION_SECS));
    trial.last_timestamp_ns = end_ns;
    push_despawn(trial, end_ns);
    trial.phase = AimPhase::Idle;
    true
}

pub fn apply_flick_demand_shot(trial: &mut AimTrial, pose: &YawPitch, timestamp_ns: u64) -> bool {
    if trial.phase != AimPhase::Armed || trial.task_kind != AimTaskKind::FlickDemand {
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
    let center = [
        trial.current_center.x as f64,
        trial.current_center.y as f64,
        trial.current_center.z as f64,
    ];
    let hit = sense_math::ray_sphere_hit(origin, dir, center, AIM_TARGET_RADIUS as f64);
    trial.shot_log.push(AimShotRecord {
        shot_index: trial.shot_log.len() as u32,
        timestamp_ns,
        hit,
        yaw_deg: pose.yaw_deg,
        pitch_deg: pose.pitch_deg,
        target_x: center[0],
        target_y: center[1],
        target_z: center[2],
        target_radius: AIM_TARGET_RADIUS as f64,
        target_id: trial.current_target_id.clone(),
    });
    trial.last_hit = Some(hit);
    trial.last_yaw_deg = pose.yaw_deg;
    trial.last_pitch_deg = pose.pitch_deg;
    trial.last_timestamp_ns = timestamp_ns;
    if !hit {
        return false;
    }
    let prev_yaw = trial
        .target_events
        .iter()
        .rev()
        .find(|e| e.event_type == "spawn")
        .and_then(|e| e.yaw_deg);
    push_despawn(trial, timestamp_ns);
    trial.hits = trial.hits.saturating_add(1);
    let yaw = next_commanded_yaw(&mut trial.rng_state, prev_yaw);
    spawn_commanded(trial, yaw, timestamp_ns);
    false // timer owns end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aim_trial::{
        aim_persist_status_from_insert, build_completed_aim_trial_record, AimRunConfigSnapshot,
    };
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
    fn yaw_set_has_eight_absolute_offsets() {
        assert_eq!(FLICK_DEMAND_YAW_SET.len(), 8);
        assert!(FLICK_DEMAND_YAW_SET.contains(&10.0));
        assert!(FLICK_DEMAND_YAW_SET.contains(&30.0));
        assert!(FLICK_DEMAND_YAW_SET.contains(&90.0));
        assert!(FLICK_DEMAND_YAW_SET.contains(&-10.0));
    }

    #[test]
    fn next_yaw_avoids_immediate_repeat() {
        let mut rng = 42u64;
        let mut prev = None;
        for _ in 0..40 {
            let y = next_commanded_yaw(&mut rng, prev);
            assert!(FLICK_DEMAND_YAW_SET.iter().any(|v| (*v - y).abs() < 1e-9));
            if let Some(p) = prev {
                assert!((y - p).abs() > 1e-9);
            }
            prev = Some(y);
        }
    }

    #[test]
    fn same_seed_same_yaw_sequence() {
        let mut a = 7u64;
        let mut b = 7u64;
        for _ in 0..20 {
            assert_eq!(
                next_commanded_yaw(&mut a, None),
                next_commanded_yaw(&mut b, None)
            );
        }
    }

    #[test]
    fn finishes_at_sixty_active_seconds() {
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        let start = 1_000_000_000u64;
        assert!(start_flick_demand_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            start,
            1,
            9,
            test_config(),
        ));
        assert!(!flick_demand_should_end(&trial, start + 59_999_999_999));
        assert!(flick_demand_should_end(&trial, start + 60_000_000_000));
        assert!(finish_flick_demand_trial(&mut trial, start + 60_000_000_000));
        let record = build_completed_aim_trial_record(&trial, 2, start + 60_000_000_000);
        assert_eq!(record.trial_type, FLICK_DEMAND_TRIAL_TYPE);
        assert_eq!(record.task_version, FLICK_DEMAND_TASK_VERSION);
        let _ = aim_persist_status_from_insert(&trial, Ok("x".into()));
    }
}

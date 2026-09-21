//! TRACKING: horizontal strafe motion and hold rapid-fire scoring helpers (pure logic).

use bevy::prelude::Vec3;
use sense_types::{AimShotRecord, AimTargetEventRecord};

use crate::{
    aim_trial::{
        active_elapsed_ns, format_target_id, lcg_next_unit, look_direction_neg_z, update_lmb_held,
        AimPhase, AimRunConfigSnapshot, AimTaskKind, AimTrial, AIM_CAMERA_ORIGIN,
    },
    camera_ctrl::YawPitch,
    config::ValidationState,
};

pub const TRACKING_DURATION_SECS: f64 = 30.0;
pub const TRACKING_RADIUS: f32 = 0.25;
pub const TRACKING_Y: f32 = 1.6;
pub const TRACKING_Z: f32 = -6.0;
pub const TRACKING_X_MIN: f32 = -1.5;
pub const TRACKING_X_MAX: f32 = 1.5;
pub const TRACKING_SPEED: f32 = 1.2;
pub const TRACKING_REVERSE_DELAY_MIN_S: f64 = 1.5;
pub const TRACKING_REVERSE_DELAY_MAX_S: f64 = 3.5;
pub const TRACKING_TASK_VERSION: &str = "2";
pub const TRACKING_FIRE_RATE_HZ: f64 = 20.0;
pub const TRACKING_FIRE_INTERVAL_NS: u64 = 50_000_000; // 1/20 s

pub fn tracking_task_config_json() -> String {
    format!(
        r#"{{"duration_secs":{},"radius":{},"y":{},"z":{},"x_min":{},"x_max":{},"speed":{},"reverse_delay_min_secs":{},"reverse_delay_max_secs":{},"scoring_rule":"hold_rapid_fire","fire_rate_hz":{},"rng":"lcg","rng_version":"1"}}"#,
        TRACKING_DURATION_SECS,
        TRACKING_RADIUS,
        TRACKING_Y,
        TRACKING_Z,
        TRACKING_X_MIN,
        TRACKING_X_MAX,
        TRACKING_SPEED,
        TRACKING_REVERSE_DELAY_MIN_S,
        TRACKING_REVERSE_DELAY_MAX_S,
        TRACKING_FIRE_RATE_HZ,
    )
}

/// Step horizontal motion by `dt_s`. Returns new x, vx, and whether a wall bounce occurred.
pub fn step_strafe(x: f32, vx: f32, dt_s: f64, x_min: f32, x_max: f32) -> (f32, f32, bool) {
    if dt_s <= 0.0 {
        return (x, vx, false);
    }
    let mut new_x = x + vx * dt_s as f32;
    let mut new_vx = vx;
    let mut bounced = false;
    if new_x > x_max {
        new_x = x_max;
        new_vx = reverse_vx(vx);
        bounced = true;
    } else if new_x < x_min {
        new_x = x_min;
        new_vx = reverse_vx(vx);
        bounced = true;
    }
    (new_x, new_vx, bounced)
}

/// Sample next reverse delay seconds from LCG rng in `[min_s, max_s)`.
pub fn next_reverse_delay_s(rng: &mut u64, min_s: f64, max_s: f64) -> f64 {
    debug_assert!(min_s <= max_s);
    min_s + lcg_next_unit(rng) * (max_s - min_s)
}

/// Flip vx (preserve speed magnitude).
pub fn reverse_vx(vx: f32) -> f32 {
    -vx
}

/// If held && ray hits sphere at center, return `dt_ns` to add; else 0.
pub fn on_target_dt_ns(
    lmb_held: bool,
    origin: [f64; 3],
    dir: [f64; 3],
    center: [f64; 3],
    radius: f64,
    dt_ns: u64,
) -> u64 {
    if !lmb_held {
        return 0;
    }
    if sense_math::ray_sphere_hit(origin, dir, center, radius) {
        dt_ns
    } else {
        0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackingTickResult {
    Continue,
    Finished,
}

fn delay_ns(delay_s: f64) -> u64 {
    (delay_s * 1e9).round() as u64
}

fn schedule_next_reverse(trial: &mut AimTrial, now_ns: u64) {
    let delay = next_reverse_delay_s(
        &mut trial.rng_state,
        TRACKING_REVERSE_DELAY_MIN_S,
        TRACKING_REVERSE_DELAY_MAX_S,
    );
    trial.next_reverse_at_ns = Some(now_ns.saturating_add(delay_ns(delay)));
}

fn push_tracking_event(trial: &mut AimTrial, timestamp_ns: u64, event_type: &str, velocity_x: f32) {
    trial.target_events.push(AimTargetEventRecord {
        target_id: trial.current_target_id.clone(),
        event_index: trial.target_events.len() as u32,
        timestamp_ns,
        event_type: event_type.into(),
        position_x: trial.current_center.x as f64,
        position_y: trial.current_center.y as f64,
        position_z: trial.current_center.z as f64,
        yaw_deg: None,
        pitch_deg: None,
        velocity_x: velocity_x as f64,
        velocity_y: 0.0,
        velocity_z: 0.0,
        event_data_json: if event_type == "spawn" {
            format!(r#"{{"radius":{TRACKING_RADIUS}}}"#)
        } else {
            "{}".into()
        },
    });
}

pub(crate) fn push_tracking_direction_event(
    trial: &mut AimTrial,
    timestamp_ns: u64,
    velocity_x: f32,
) {
    push_tracking_event(trial, timestamp_ns, "direction_change", velocity_x);
}

pub fn start_tracking_trial(
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
    trial.task_kind = AimTaskKind::Tracking;
    trial.live_targets.clear();
    trial.phase = AimPhase::Armed;
    trial.hits = 0;
    trial.last_hit = None;
    trial.score_secs = None;
    trial.time_on_target_ns = 0;
    trial.lmb_held = false;
    trial.tracking_vx = if random_seed & 1 == 0 {
        TRACKING_SPEED
    } else {
        -TRACKING_SPEED
    };
    trial.next_reverse_at_ns = None;
    trial.last_tracking_tick_ns = Some(now_ns);
    trial.next_tracking_shot_ns = None;
    trial.shot_log.clear();
    trial.target_events.clear();
    trial.clear_sample_logs();
    trial.next_target_ordinal = 2;
    trial.current_target_id = format_target_id(1);
    trial.current_center = Vec3::new(0.0, TRACKING_Y, TRACKING_Z);
    trial.start_timestamp_ns = now_ns;
    trial.accumulated_pause_ns = 0;
    trial.paused_at_qpc = None;
    trial.start_unix_ms = start_unix_ms;
    trial.config_snapshot = Some(config);
    schedule_next_reverse(trial, now_ns);
    push_tracking_event(trial, now_ns, "spawn", trial.tracking_vx);
    true
}

pub fn tracking_should_end(trial: &AimTrial, now_ns: u64) -> bool {
    active_elapsed_ns(trial, now_ns) as f64 / 1e9 >= TRACKING_DURATION_SECS
}

/// While Armed+Tracking is paused, input drain still syncs LMB held state without
/// motion, scoring, or camera apply (see `drain_mouse_to_camera`).
pub fn sync_tracking_lmb_during_pause(trial: &mut AimTrial, buttons: u32) -> bool {
    if trial.phase == AimPhase::Armed
        && trial.task_kind == AimTaskKind::Tracking
        && trial.paused_at_qpc.is_some()
    {
        update_lmb_held(&mut trial.lmb_held, buttons);
        true
    } else {
        false
    }
}

pub fn sync_tracking_lmb_from_sample(trial: &mut AimTrial, buttons: u32) {
    if trial.phase != AimPhase::Armed || trial.task_kind != AimTaskKind::Tracking {
        return;
    }
    update_lmb_held(&mut trial.lmb_held, buttons);
}

pub fn tick_tracking_frame_logic(
    trial: &mut AimTrial,
    pose: &YawPitch,
    now_ns: u64,
) -> TrackingTickResult {
    if trial.phase != AimPhase::Armed || trial.task_kind != AimTaskKind::Tracking {
        return TrackingTickResult::Finished;
    }
    if trial.paused_at_qpc.is_some() {
        return TrackingTickResult::Continue;
    }

    let previous_tick_ns = trial
        .last_tracking_tick_ns
        .replace(now_ns)
        .unwrap_or(now_ns);
    let raw_dt_ns = now_ns.saturating_sub(previous_tick_ns);
    let duration_ns = delay_ns(TRACKING_DURATION_SECS);
    let previous_active_ns = active_elapsed_ns(trial, previous_tick_ns);
    let dt_ns = raw_dt_ns.min(duration_ns.saturating_sub(previous_active_ns));

    let (x, vx, wall_bounced) = step_strafe(
        trial.current_center.x,
        trial.tracking_vx,
        dt_ns as f64 / 1e9,
        TRACKING_X_MIN,
        TRACKING_X_MAX,
    );
    trial.current_center.x = x;
    trial.tracking_vx = vx;

    let scheduled_reverse = trial.next_reverse_at_ns.is_some_and(|at| now_ns >= at);
    if scheduled_reverse && !wall_bounced {
        trial.tracking_vx = reverse_vx(trial.tracking_vx);
    }
    if wall_bounced || scheduled_reverse {
        push_tracking_event(trial, now_ns, "direction_change", trial.tracking_vx);
        schedule_next_reverse(trial, now_ns);
    }

    let origin = [
        AIM_CAMERA_ORIGIN.x as f64,
        AIM_CAMERA_ORIGIN.y as f64,
        AIM_CAMERA_ORIGIN.z as f64,
    ];
    let center = [
        trial.current_center.x as f64,
        trial.current_center.y as f64,
        trial.current_center.z as f64,
    ];
    trial.time_on_target_ns = trial.time_on_target_ns.saturating_add(on_target_dt_ns(
        trial.lmb_held,
        origin,
        look_direction_neg_z(pose.yaw_deg, pose.pitch_deg),
        center,
        TRACKING_RADIUS as f64,
        dt_ns,
    ));

    emit_tracking_rapid_fire(trial, pose, now_ns);

    if tracking_should_end(trial, now_ns) {
        finish_tracking_trial(trial, now_ns);
        return TrackingTickResult::Finished;
    }
    TrackingTickResult::Continue
}

/// While LMB held: fire at 20 Hz on the QPC grid (pause-safe — only called when not paused).
fn emit_tracking_rapid_fire(trial: &mut AimTrial, pose: &YawPitch, now_ns: u64) {
    if !trial.lmb_held {
        trial.next_tracking_shot_ns = None;
        return;
    }
    let mut next = trial.next_tracking_shot_ns.unwrap_or(now_ns);
    // Cap catch-up so a long hitch cannot enqueue thousands of identical shots.
    let mut fired = 0u32;
    const MAX_CATCH_UP: u32 = 40; // 2 s at 20 Hz
    while next <= now_ns && fired < MAX_CATCH_UP {
        push_tracking_virtual_shot(trial, pose, next);
        next = next.saturating_add(TRACKING_FIRE_INTERVAL_NS);
        fired += 1;
    }
    trial.next_tracking_shot_ns = Some(next);
}

fn push_tracking_virtual_shot(trial: &mut AimTrial, pose: &YawPitch, timestamp_ns: u64) {
    let origin = [
        AIM_CAMERA_ORIGIN.x as f64,
        AIM_CAMERA_ORIGIN.y as f64,
        AIM_CAMERA_ORIGIN.z as f64,
    ];
    let center = [
        trial.current_center.x as f64,
        trial.current_center.y as f64,
        trial.current_center.z as f64,
    ];
    let hit = sense_math::ray_sphere_hit(
        origin,
        look_direction_neg_z(pose.yaw_deg, pose.pitch_deg),
        center,
        TRACKING_RADIUS as f64,
    );
    trial.shot_log.push(AimShotRecord {
        shot_index: trial.shot_log.len() as u32,
        timestamp_ns,
        hit,
        yaw_deg: pose.yaw_deg,
        pitch_deg: pose.pitch_deg,
        target_x: center[0],
        target_y: center[1],
        target_z: center[2],
        target_radius: TRACKING_RADIUS as f64,
        target_id: trial.current_target_id.clone(),
    });
    if hit {
        trial.hits = trial.hits.saturating_add(1);
    }
    trial.last_hit = Some(hit);
    trial.last_yaw_deg = pose.yaw_deg;
    trial.last_pitch_deg = pose.pitch_deg;
    trial.last_timestamp_ns = timestamp_ns;
}

pub fn finish_tracking_trial(trial: &mut AimTrial, end_ns: u64) -> bool {
    if trial.phase != AimPhase::Armed || trial.task_kind != AimTaskKind::Tracking {
        return false;
    }
    // Secondary diagnostic: on-target hold time. Primary score is hits/shots/accuracy.
    trial.score_secs = Some(trial.time_on_target_ns as f64 / 1e9);
    trial.last_timestamp_ns = end_ns;
    push_tracking_event(trial, end_ns, "despawn", 0.0);
    trial.lmb_held = false;
    trial.next_reverse_at_ns = None;
    trial.last_tracking_tick_ns = None;
    trial.next_tracking_shot_ns = None;
    trial.phase = AimPhase::Idle;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aim_trial::{
            aim_persist_status_from_insert, begin_aim_pause, build_completed_aim_trial_record,
            end_aim_pause, AimRunConfigSnapshot, AimTaskKind, AimTrial, RI_MOUSE_LEFT_BUTTON_DOWN,
            RI_MOUSE_LEFT_BUTTON_UP,
        },
        camera_ctrl::YawPitch,
        config::{ExperimentSettings, ValidationState},
    };

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
    fn step_strafe_moves_without_bounce() {
        let (x, vx, bounced) = step_strafe(0.0, 1.2, 0.5, TRACKING_X_MIN, TRACKING_X_MAX);
        assert!((x - 0.6).abs() < 1e-5);
        assert!((vx - 1.2).abs() < 1e-5);
        assert!(!bounced);
    }

    #[test]
    fn step_strafe_bounces_at_x_max() {
        let (x, vx, bounced) = step_strafe(1.4, 1.2, 0.2, TRACKING_X_MIN, TRACKING_X_MAX);
        assert!((x - TRACKING_X_MAX).abs() < 1e-5);
        assert!((vx - (-1.2)).abs() < 1e-5);
        assert!(bounced);
    }

    #[test]
    fn step_strafe_bounces_at_x_min() {
        let (x, vx, bounced) = step_strafe(-1.4, -1.2, 0.2, TRACKING_X_MIN, TRACKING_X_MAX);
        assert!((x - TRACKING_X_MIN).abs() < 1e-5);
        assert!((vx - 1.2).abs() < 1e-5);
        assert!(bounced);
    }

    #[test]
    fn step_strafe_zero_dt_unchanged() {
        let (x, vx, bounced) = step_strafe(0.5, -1.2, 0.0, TRACKING_X_MIN, TRACKING_X_MAX);
        assert!((x - 0.5).abs() < 1e-5);
        assert!((vx - (-1.2)).abs() < 1e-5);
        assert!(!bounced);
    }

    #[test]
    fn reverse_vx_flips_sign() {
        assert!((reverse_vx(1.2) - (-1.2)).abs() < 1e-5);
        assert!((reverse_vx(-1.2) - 1.2).abs() < 1e-5);
    }

    #[test]
    fn next_reverse_delay_in_range() {
        let mut rng = 42u64;
        let mut saw_above_half_range = false;
        for _ in 0..100 {
            let d = next_reverse_delay_s(
                &mut rng,
                TRACKING_REVERSE_DELAY_MIN_S,
                TRACKING_REVERSE_DELAY_MAX_S,
            );
            assert!(d >= TRACKING_REVERSE_DELAY_MIN_S);
            assert!(d < TRACKING_REVERSE_DELAY_MAX_S);
            saw_above_half_range |= d > 2.5;
        }
        assert!(
            saw_above_half_range,
            "LCG must cover the upper half of the delay range"
        );
    }

    #[test]
    fn next_reverse_delay_same_seed_same_sequence() {
        let mut a = 99u64;
        let mut b = 99u64;
        let seq_a: Vec<f64> = (0..5)
            .map(|_| {
                next_reverse_delay_s(
                    &mut a,
                    TRACKING_REVERSE_DELAY_MIN_S,
                    TRACKING_REVERSE_DELAY_MAX_S,
                )
            })
            .collect();
        let seq_b: Vec<f64> = (0..5)
            .map(|_| {
                next_reverse_delay_s(
                    &mut b,
                    TRACKING_REVERSE_DELAY_MIN_S,
                    TRACKING_REVERSE_DELAY_MAX_S,
                )
            })
            .collect();
        assert_eq!(seq_a, seq_b);
    }

    #[test]
    fn on_target_dt_ns_zero_when_not_held() {
        let origin = [0.0, 1.6, 4.0];
        let dir = [0.0, 0.0, -1.0];
        let center = [0.0, 1.6, -6.0];
        assert_eq!(
            on_target_dt_ns(
                false,
                origin,
                dir,
                center,
                TRACKING_RADIUS as f64,
                16_666_666
            ),
            0
        );
    }

    #[test]
    fn on_target_dt_ns_zero_when_held_but_miss() {
        let origin = [0.0, 1.6, 4.0];
        let dir = [1.0, 0.0, 0.0];
        let center = [0.0, 1.6, -6.0];
        assert_eq!(
            on_target_dt_ns(
                true,
                origin,
                dir,
                center,
                TRACKING_RADIUS as f64,
                16_666_666
            ),
            0
        );
    }

    #[test]
    fn on_target_dt_ns_returns_dt_when_held_and_hit() {
        let origin = [0.0, 1.6, 4.0];
        let dir = [0.0, 0.0, -1.0];
        let center = [0.0, 1.6, -6.0];
        let dt = 16_666_666u64;
        assert_eq!(
            on_target_dt_ns(true, origin, dir, center, TRACKING_RADIUS as f64, dt),
            dt
        );
    }

    #[test]
    fn task_config_json_has_required_knobs() {
        assert_eq!(TRACKING_TASK_VERSION, "2");
        let j = tracking_task_config_json();
        assert!(j.contains("\"duration_secs\":30"));
        assert!(j.contains(&format!("\"radius\":{}", TRACKING_RADIUS)));
        assert!(j.contains(&format!("\"y\":{}", TRACKING_Y)));
        assert!(j.contains(&format!("\"z\":{}", TRACKING_Z)));
        assert!(j.contains(&format!("\"x_min\":{}", TRACKING_X_MIN)));
        assert!(j.contains(&format!("\"x_max\":{}", TRACKING_X_MAX)));
        assert!(j.contains(&format!("\"speed\":{}", TRACKING_SPEED)));
        assert!(j.contains("\"reverse_delay_min_secs\":1.5"));
        assert!(j.contains("\"reverse_delay_max_secs\":3.5"));
        assert!(j.contains("\"scoring_rule\":\"hold_rapid_fire\""));
        assert!(j.contains("\"fire_rate_hz\":20"));
        assert!(j.contains("\"rng\":\"lcg\""));
        assert!(j.contains("\"rng_version\":\"1\""));
    }

    #[test]
    fn lmb_level_tracks_down_hold_and_up() {
        let mut held = false;
        update_lmb_held(&mut held, RI_MOUSE_LEFT_BUTTON_DOWN);
        assert!(held);
        update_lmb_held(&mut held, 0);
        assert!(held);
        update_lmb_held(&mut held, RI_MOUSE_LEFT_BUTTON_UP);
        assert!(!held);
    }

    #[test]
    fn tracking_scores_only_while_held_and_aimed_at_target() {
        let start = 1_000_000_000;
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        assert!(start_tracking_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            start,
            1_700_000_000_000,
            42,
            test_config(),
        ));

        trial.tracking_vx = 0.0;
        sync_tracking_lmb_from_sample(&mut trial, 0);
        assert_eq!(
            tick_tracking_frame_logic(&mut trial, &pose, start + 10),
            TrackingTickResult::Continue
        );
        assert_eq!(trial.time_on_target_ns, 0);

        sync_tracking_lmb_from_sample(&mut trial, RI_MOUSE_LEFT_BUTTON_DOWN);
        tick_tracking_frame_logic(&mut trial, &pose, start + 20);
        assert_eq!(trial.time_on_target_ns, 10);

        pose.yaw_deg = 90.0;
        sync_tracking_lmb_from_sample(&mut trial, 0);
        tick_tracking_frame_logic(&mut trial, &pose, start + 30);
        assert_eq!(trial.time_on_target_ns, 10);

        pose.yaw_deg = 0.0;
        sync_tracking_lmb_from_sample(&mut trial, RI_MOUSE_LEFT_BUTTON_UP);
        tick_tracking_frame_logic(&mut trial, &pose, start + 40);
        assert_eq!(trial.time_on_target_ns, 10);
        // Rapid-fire only while held: one shot at press tick (start+20), none after release.
        assert_eq!(trial.shot_log.len(), 1);
        assert!(trial.shot_log[0].hit);
        assert_eq!(trial.hits, 1);
    }

    #[test]
    fn tracking_pause_syncs_lmb_release_without_scoring() {
        let start = 5_000_000_000;
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        assert!(start_tracking_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            start,
            1_700_000_000_000,
            13,
            test_config(),
        ));
        trial.lmb_held = true;
        trial.tracking_vx = 0.0;
        let score = trial.time_on_target_ns;

        begin_aim_pause(&mut trial, start + 100);
        assert!(sync_tracking_lmb_during_pause(
            &mut trial,
            RI_MOUSE_LEFT_BUTTON_UP
        ));
        assert!(!trial.lmb_held);

        tick_tracking_frame_logic(&mut trial, &pose, start + 5_000_000_100);
        assert_eq!(trial.time_on_target_ns, score);

        end_aim_pause(&mut trial, start + 5_000_000_100);
        assert!(!trial.lmb_held);
        tick_tracking_frame_logic(&mut trial, &pose, start + 5_000_000_200);
        assert_eq!(trial.time_on_target_ns, score);
    }

    #[test]
    fn tracking_pause_freezes_motion_score_and_reverse_schedule() {
        let start = 2_000_000_000;
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        assert!(start_tracking_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            start,
            1_700_000_000_000,
            7,
            test_config(),
        ));
        trial.lmb_held = true;
        let x = trial.current_center.x;
        let score = trial.time_on_target_ns;
        let reverse_at = trial.next_reverse_at_ns.expect("reverse schedule");

        begin_aim_pause(&mut trial, start + 100);
        let pause_event = trial.target_events.last().expect("pause direction change");
        assert_eq!(pause_event.event_type, "direction_change");
        assert_eq!(pause_event.timestamp_ns, start + 100);
        assert_eq!(pause_event.velocity_x, 0.0);
        tick_tracking_frame_logic(&mut trial, &pose, start + 5_000_000_100);
        assert_eq!(trial.current_center.x, x);
        assert_eq!(trial.time_on_target_ns, score);

        end_aim_pause(&mut trial, start + 5_000_000_100);
        let resume_event = trial.target_events.last().expect("resume direction change");
        assert_eq!(resume_event.event_type, "direction_change");
        assert_eq!(resume_event.timestamp_ns, start + 5_000_000_100);
        assert_eq!(resume_event.velocity_x, trial.tracking_vx as f64);
        assert_eq!(trial.next_reverse_at_ns, Some(reverse_at + 5_000_000_000));
    }

    #[test]
    fn frame_tick_finishes_tracking_without_mouse_samples() {
        let start = 8_000_000_000;
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        assert!(start_tracking_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            start,
            1_700_000_000_000,
            17,
            test_config(),
        ));

        assert_eq!(
            tick_tracking_frame_logic(&mut trial, &pose, start + 30_000_000_000),
            TrackingTickResult::Finished
        );
        assert_eq!(trial.phase, AimPhase::Idle);
        assert!(trial.input_log.is_empty());
    }

    #[test]
    fn wall_and_scheduled_reversals_emit_direction_changes() {
        let start = 3_000_000_000;
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        assert!(start_tracking_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            start,
            1_700_000_000_000,
            9,
            test_config(),
        ));
        assert_eq!(trial.target_events[0].event_type, "spawn");

        trial.current_center.x = TRACKING_X_MAX - 0.01;
        trial.tracking_vx = TRACKING_SPEED;
        trial.next_reverse_at_ns = Some(start + 10_000_000_000);
        tick_tracking_frame_logic(&mut trial, &pose, start + 20_000_000);
        assert_eq!(trial.target_events[1].event_type, "direction_change");
        assert!(trial.tracking_vx < 0.0);

        trial.next_reverse_at_ns = Some(start + 30_000_000);
        tick_tracking_frame_logic(&mut trial, &pose, start + 30_000_000);
        assert_eq!(trial.target_events[2].event_type, "direction_change");
        assert!(trial.tracking_vx > 0.0);
    }

    #[test]
    fn tracking_finishes_at_active_thirty_seconds_and_persists_score_accuracy() {
        let start = 4_000_000_000;
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        assert!(start_tracking_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            start,
            1_700_000_000_000,
            11,
            test_config(),
        ));
        trial.time_on_target_ns = 12_000_000_000;
        // Simulate 10 hits / 20 shots for primary accuracy.
        trial.hits = 10;
        for i in 0..20 {
            trial.shot_log.push(sense_types::AimShotRecord {
                shot_index: i,
                timestamp_ns: start + i as u64 * TRACKING_FIRE_INTERVAL_NS,
                hit: i < 10,
                yaw_deg: 0.0,
                pitch_deg: 0.0,
                target_x: 0.0,
                target_y: TRACKING_Y as f64,
                target_z: TRACKING_Z as f64,
                target_radius: TRACKING_RADIUS as f64,
                target_id: "target_001".into(),
            });
        }

        assert!(!tracking_should_end(&trial, start + 29_999_999_999));
        let end = start + 30_500_000_000;
        assert!(tracking_should_end(&trial, end));
        assert!(finish_tracking_trial(&mut trial, end));
        assert_eq!(trial.score_secs, Some(12.0));
        assert_eq!(trial.shot_log.len(), 20);
        assert_eq!(
            trial
                .target_events
                .last()
                .map(|event| event.event_type.as_str()),
            Some("despawn")
        );

        let record = build_completed_aim_trial_record(&trial, 1_700_000_030_000, end);
        assert_eq!(trial.task_kind, AimTaskKind::Tracking);
        assert_eq!(record.trial_type, "TRACKING");
        assert_eq!(record.task_version, TRACKING_TASK_VERSION);
        assert_eq!(record.hits, 10);
        assert_eq!(record.shots, 20);
        assert_eq!(record.misses, 10);
        assert_eq!(record.duration_secs, TRACKING_DURATION_SECS);
        assert_eq!(record.score_secs, 12.0);
        assert!((record.accuracy - 0.5).abs() < 1e-12);

        let status = aim_persist_status_from_insert(&trial, Ok("tracking_test".into()));
        assert!((status.accuracy - 0.5).abs() < 1e-12);
        assert_eq!(status.hits, 10);
        assert_eq!(status.shots, 20);
    }

    #[test]
    fn rapid_fire_emits_20hz_shots_only_while_held() {
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        let start = 1_000_000_000u64;
        assert!(start_tracking_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            start,
            1_700_000_000_000,
            1,
            test_config(),
        ));
        trial.tracking_vx = 0.0;

        sync_tracking_lmb_from_sample(&mut trial, RI_MOUSE_LEFT_BUTTON_DOWN);
        tick_tracking_frame_logic(&mut trial, &pose, start + 1);
        assert_eq!(trial.shot_log.len(), 1);
        assert!(trial.shot_log[0].hit);

        tick_tracking_frame_logic(&mut trial, &pose, start + 1 + TRACKING_FIRE_INTERVAL_NS);
        assert_eq!(trial.shot_log.len(), 2);
        tick_tracking_frame_logic(&mut trial, &pose, start + 1 + 2 * TRACKING_FIRE_INTERVAL_NS);
        assert_eq!(trial.shot_log.len(), 3);
        assert_eq!(trial.hits, 3);

        sync_tracking_lmb_from_sample(&mut trial, RI_MOUSE_LEFT_BUTTON_UP);
        tick_tracking_frame_logic(&mut trial, &pose, start + 200_000_000);
        assert_eq!(trial.shot_log.len(), 3);

        sync_tracking_lmb_from_sample(&mut trial, RI_MOUSE_LEFT_BUTTON_DOWN);
        tick_tracking_frame_logic(&mut trial, &pose, start + 200_000_001);
        assert_eq!(trial.shot_log.len(), 4);
    }

    #[test]
    fn pause_freezes_rapid_fire_schedule() {
        let mut trial = AimTrial::default();
        let mut pose = YawPitch::default();
        let start = 6_000_000_000u64;
        assert!(start_tracking_trial(
            &mut pose,
            &mut trial,
            ValidationState::Idle,
            start,
            1_700_000_000_000,
            3,
            test_config(),
        ));
        trial.tracking_vx = 0.0;
        sync_tracking_lmb_from_sample(&mut trial, RI_MOUSE_LEFT_BUTTON_DOWN);
        tick_tracking_frame_logic(&mut trial, &pose, start + 1);
        assert_eq!(trial.shot_log.len(), 1);
        let next = trial.next_tracking_shot_ns.expect("schedule after first shot");

        begin_aim_pause(&mut trial, start + 10_000_000);
        tick_tracking_frame_logic(&mut trial, &pose, start + 5_010_000_000);
        assert_eq!(trial.shot_log.len(), 1);

        end_aim_pause(&mut trial, start + 5_010_000_000);
        assert_eq!(
            trial.next_tracking_shot_ns,
            Some(next + 5_000_000_000),
            "fire schedule shifts by pause duration"
        );
        // Still before shifted next → no new shot
        tick_tracking_frame_logic(&mut trial, &pose, start + 5_010_000_001);
        assert_eq!(trial.shot_log.len(), 1);
        // At shifted next → fire
        tick_tracking_frame_logic(&mut trial, &pose, next + 5_000_000_000);
        assert_eq!(trial.shot_log.len(), 2);
    }
}

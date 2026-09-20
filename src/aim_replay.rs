use std::collections::BTreeMap;

use bevy::prelude::{Camera3d, Res, ResMut, Resource, Single, Time, With};
use sense_telemetry::{AimTrialReplayBundle, AimTrialSummary};
use sense_types::{AimCameraSampleRecord, AimShotRecord, AimTargetEventRecord};

use crate::{
    camera_ctrl::YawPitch,
    lab_ui::{LabScreen, LabUi},
};

const SHOT_FLASH_SECS: f32 = 0.15;

#[derive(Resource, Debug)]
pub struct AimReplay {
    pub bundle: Option<AimTrialReplayBundle>,
    pub summaries: Vec<AimTrialSummary>,
    pub playing: bool,
    pub speed: f32,
    pub t_ns: u64,
    pub last_shot: Option<bool>,
    pub flash_remaining_secs: f32,
    pub load_error: Option<String>,
}

impl Default for AimReplay {
    fn default() -> Self {
        Self {
            bundle: None,
            summaries: Vec::new(),
            playing: false,
            speed: 1.0,
            t_ns: 0,
            last_shot: None,
            flash_remaining_secs: 0.0,
            load_error: None,
        }
    }
}

pub fn advance_replay_t(
    playing: bool,
    speed: f32,
    dt_s: f64,
    t_ns: u64,
    end_ns: u64,
) -> (u64, bool) {
    if !playing {
        return (t_ns.min(end_ns), false);
    }
    let dt_ns = (dt_s.max(0.0) * f64::from(speed.max(0.0)) * 1e9) as u64;
    let next = t_ns.saturating_add(dt_ns).min(end_ns);
    (next, next < end_ns)
}

pub fn camera_pose_at(samples: &[AimCameraSampleRecord], t_ns: u64) -> (f64, f64) {
    samples
        .iter()
        .filter(|sample| sample.timestamp_ns <= t_ns)
        .max_by_key(|sample| sample.timestamp_ns)
        .map_or((0.0, 0.0), |sample| (sample.yaw_deg, sample.pitch_deg))
}

pub fn live_targets_at(events: &[AimTargetEventRecord], t_ns: u64) -> Vec<(String, f64, f64, f64)> {
    let mut live_targets: BTreeMap<String, (f64, f64, f64, f64, u64)> = BTreeMap::new();
    for event in events.iter().filter(|event| event.timestamp_ns <= t_ns) {
        match event.event_type.as_str() {
            "spawn" | "direction_change" => {
                live_targets.insert(
                    event.target_id.clone(),
                    (
                        event.position_x,
                        event.position_y,
                        event.position_z,
                        event.velocity_x,
                        event.timestamp_ns,
                    ),
                );
            }
            "despawn" => {
                live_targets.remove(&event.target_id);
            }
            _ => {}
        }
    }
    live_targets
        .into_iter()
        .map(|(target_id, (x, y, z, velocity_x, last_event_ns))| {
            let elapsed_s = t_ns.saturating_sub(last_event_ns) as f64 / 1e9;
            (target_id, x + velocity_x * elapsed_s, y, z)
        })
        .collect()
}

pub fn shots_crossed(shots: &[AimShotRecord], prev_t: u64, t_ns: u64) -> Vec<&AimShotRecord> {
    shots
        .iter()
        .filter(|shot| prev_t < shot.timestamp_ns && shot.timestamp_ns <= t_ns)
        .collect()
}

pub fn tick_aim_replay(
    time: Res<Time>,
    mut replay: ResMut<AimReplay>,
    ui: Res<LabUi>,
    mut yaw: Single<&mut YawPitch, With<Camera3d>>,
) {
    if ui.screen != LabScreen::HistoryReplay {
        return;
    }
    let Some(end_ns) = replay
        .bundle
        .as_ref()
        .map(|bundle| bundle.trial.end_timestamp_ns)
    else {
        return;
    };

    let prev_t = replay.t_ns;
    let (next_t, playing) = advance_replay_t(
        replay.playing,
        replay.speed,
        time.delta_secs_f64(),
        replay.t_ns,
        end_ns,
    );
    let (crossed_shot, (yaw_deg, pitch_deg)) = {
        let bundle = replay.bundle.as_ref().expect("bundle checked above");
        let crossed_shot = if next_t > prev_t {
            shots_crossed(&bundle.shots, prev_t, next_t)
                .last()
                .map(|shot| shot.hit)
        } else {
            None
        };
        (
            crossed_shot,
            camera_pose_at(&bundle.camera_samples, next_t),
        )
    };
    replay.t_ns = next_t;
    replay.playing = playing;
    replay.flash_remaining_secs =
        (replay.flash_remaining_secs - time.delta_secs()).max(0.0);

    if let Some(hit) = crossed_shot {
        replay.last_shot = Some(hit);
        replay.flash_remaining_secs = SHOT_FLASH_SECS;
    }

    yaw.yaw_deg = yaw_deg;
    yaw.pitch_deg = pitch_deg;
}

#[cfg(test)]
mod tests {
    use sense_types::{AimCameraSampleRecord, AimShotRecord, AimTargetEventRecord};

    use super::{
        advance_replay_t, camera_pose_at, live_targets_at, shots_crossed, AimReplay,
    };

    fn camera_sample(timestamp_ns: u64, yaw_deg: f64, pitch_deg: f64) -> AimCameraSampleRecord {
        AimCameraSampleRecord {
            timestamp_ns,
            yaw_deg,
            pitch_deg,
            yaw_delta_deg: 0.0,
            pitch_delta_deg: 0.0,
        }
    }

    fn target_event(
        target_id: &str,
        event_index: u32,
        timestamp_ns: u64,
        event_type: &str,
        position: (f64, f64, f64),
    ) -> AimTargetEventRecord {
        AimTargetEventRecord {
            target_id: target_id.into(),
            event_index,
            timestamp_ns,
            event_type: event_type.into(),
            position_x: position.0,
            position_y: position.1,
            position_z: position.2,
            yaw_deg: None,
            pitch_deg: None,
            velocity_x: 0.0,
            velocity_y: 0.0,
            velocity_z: 0.0,
            event_data_json: "{}".into(),
        }
    }

    fn shot(shot_index: u32, timestamp_ns: u64) -> AimShotRecord {
        AimShotRecord {
            shot_index,
            timestamp_ns,
            hit: false,
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            target_x: 0.0,
            target_y: 0.0,
            target_z: 10.0,
            target_radius: 0.25,
            target_id: "target_001".into(),
        }
    }

    #[test]
    fn replay_defaults_to_normal_speed_stopped_and_unloaded() {
        let replay = AimReplay::default();

        assert_eq!(replay.speed, 1.0);
        assert!(!replay.playing);
        assert_eq!(replay.t_ns, 0);
        assert!(replay.bundle.is_none());
        assert!(replay.load_error.is_none());
    }

    #[test]
    fn camera_pose_holds_latest_sample_at_or_before_time() {
        let samples = vec![camera_sample(100, 1.0, -1.0), camera_sample(200, 2.0, -2.0)];

        assert_eq!(camera_pose_at(&samples, 99), (0.0, 0.0));
        assert_eq!(camera_pose_at(&samples, 100), (1.0, -1.0));
        assert_eq!(camera_pose_at(&samples, 150), (1.0, -1.0));
        assert_eq!(camera_pose_at(&samples, 200), (2.0, -2.0));
        assert_eq!(camera_pose_at(&samples, 250), (2.0, -2.0));
    }

    #[test]
    fn live_targets_nets_spawn_and_despawn_events() {
        let events = vec![
            target_event("b", 0, 100, "spawn", (1.0, 2.0, 3.0)),
            target_event("a", 1, 110, "spawn", (4.0, 5.0, 6.0)),
            target_event("b", 2, 120, "direction_change", (9.0, 9.0, 9.0)),
            target_event("b", 3, 130, "spawn", (7.0, 8.0, 9.0)),
            target_event("a", 4, 140, "despawn", (4.0, 5.0, 6.0)),
        ];

        assert_eq!(
            live_targets_at(&events, 125),
            vec![("a".into(), 4.0, 5.0, 6.0), ("b".into(), 9.0, 9.0, 9.0),]
        );
        assert_eq!(
            live_targets_at(&events, 140),
            vec![("b".into(), 7.0, 8.0, 9.0)]
        );
    }

    #[test]
    fn live_targets_integrate_velocity_between_direction_changes() {
        let mut spawn = target_event("tracking", 0, 1_000_000_000, "spawn", (0.0, 1.6, -6.0));
        spawn.velocity_x = 1.2;
        let mut reverse = target_event(
            "tracking",
            1,
            3_000_000_000,
            "direction_change",
            (2.4, 1.6, -6.0),
        );
        reverse.velocity_x = -1.2;
        let events = vec![spawn, reverse];

        assert_eq!(
            live_targets_at(&events, 2_000_000_000),
            vec![("tracking".into(), 1.2, 1.6, -6.0)]
        );
        let after_reverse = live_targets_at(&events, 3_500_000_000);
        assert_eq!(after_reverse[0].0, "tracking");
        assert!((after_reverse[0].1 - 1.8).abs() < 1e-12);
        assert_eq!((after_reverse[0].2, after_reverse[0].3), (1.6, -6.0));
    }

    #[test]
    fn shots_crossed_uses_open_closed_time_window() {
        let shots = vec![shot(0, 100), shot(1, 150), shot(2, 200)];

        let crossed = shots_crossed(&shots, 100, 200);

        assert_eq!(
            crossed
                .iter()
                .map(|shot| shot.shot_index)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
    }

    #[test]
    fn advance_replay_t_applies_speed_and_pauses_at_end() {
        assert_eq!(
            advance_replay_t(true, 2.0, 0.25, 1_000_000_000, 2_000_000_000),
            (1_500_000_000, true)
        );
        assert_eq!(
            advance_replay_t(true, 4.0, 0.25, 1_500_000_000, 2_000_000_000),
            (2_000_000_000, false)
        );
    }

    #[test]
    fn advance_replay_t_does_not_move_while_paused() {
        assert_eq!(
            advance_replay_t(false, 4.0, 1.0, 1_500_000_000, 2_000_000_000),
            (1_500_000_000, false)
        );
    }
}

use std::collections::BTreeMap;

use bevy::prelude::Resource;
use sense_telemetry::{AimTrialReplayBundle, AimTrialSummary};
use sense_types::{AimCameraSampleRecord, AimShotRecord, AimTargetEventRecord};

#[derive(Resource, Debug)]
pub struct AimReplay {
    pub bundle: Option<AimTrialReplayBundle>,
    pub summaries: Vec<AimTrialSummary>,
    pub playing: bool,
    pub speed: f32,
    pub t_ns: u64,
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
            load_error: None,
        }
    }
}

pub fn camera_pose_at(samples: &[AimCameraSampleRecord], t_ns: u64) -> (f64, f64) {
    samples
        .iter()
        .filter(|sample| sample.timestamp_ns <= t_ns)
        .max_by_key(|sample| sample.timestamp_ns)
        .map_or((0.0, 0.0), |sample| (sample.yaw_deg, sample.pitch_deg))
}

pub fn live_targets_at(events: &[AimTargetEventRecord], t_ns: u64) -> Vec<(String, f64, f64, f64)> {
    let mut live_targets = BTreeMap::new();
    for event in events.iter().filter(|event| event.timestamp_ns <= t_ns) {
        match event.event_type.as_str() {
            "spawn" => {
                live_targets.insert(
                    event.target_id.clone(),
                    (event.position_x, event.position_y, event.position_z),
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
        .map(|(target_id, (x, y, z))| (target_id, x, y, z))
        .collect()
}

pub fn shots_crossed(shots: &[AimShotRecord], prev_t: u64, t_ns: u64) -> Vec<&AimShotRecord> {
    shots
        .iter()
        .filter(|shot| prev_t < shot.timestamp_ns && shot.timestamp_ns <= t_ns)
        .collect()
}

#[cfg(test)]
mod tests {
    use sense_types::{AimCameraSampleRecord, AimShotRecord, AimTargetEventRecord};

    use super::{camera_pose_at, live_targets_at, shots_crossed, AimReplay};

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
            vec![("a".into(), 4.0, 5.0, 6.0), ("b".into(), 1.0, 2.0, 3.0),]
        );
        assert_eq!(
            live_targets_at(&events, 140),
            vec![("b".into(), 7.0, 8.0, 9.0)]
        );
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
}

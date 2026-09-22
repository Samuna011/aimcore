//! M4.3 movement demand vs processor exposure (config-independent demand).

use crate::geometry::{
    angular_distance_deg, camera_origin, look_dir, path_length_yaw_pitch, target_dir,
};
use crate::reconstruct::ReconstructedTrial;
use crate::segment::MovementCandidate;
use sense_types::{AimShotRecord, AimTargetEventRecord};
use serde::Serialize;

/// Configuration-independent movement / task exposure.
///
/// Must not encode processor, sensitivity, or acceleration. Stratum keys for
/// factorial cells use [`MovementDemand::commanded_yaw_deg`] (absolute), not exposure.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MovementDemand {
    /// Realized camera path length (°) over the acquisition window.
    pub angular_displacement_deg: f64,
    /// Angular distance from aim at acquisition start to target (when computable).
    pub target_distance_deg: Option<f64>,
    /// Controlled task demand from spawn telemetry; `None` if not stamped.
    pub commanded_yaw_deg: Option<f64>,
    /// Peak physical raw input speed (counts/ms via dt_ns) on the window.
    pub peak_physical_input_speed: Option<f64>,
    pub movement_duration_ms: f64,
}

/// Processor response on the same window — **not** a demand dimension.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProcessorExposure {
    pub peak_processed_speed: Option<f64>,
    pub peak_accel_scale: Option<f64>,
}

/// Parse `commanded_yaw_deg` from spawn `event_data_json` or spawn `yaw_deg`.
pub fn commanded_yaw_for_target(
    events: &[AimTargetEventRecord],
    target_id: &str,
) -> Option<f64> {
    let spawn = events
        .iter()
        .rev()
        .find(|e| e.target_id == target_id && e.event_type == "spawn")?;
    if let Some(y) = parse_commanded_yaw_json(&spawn.event_data_json) {
        return Some(y);
    }
    spawn.yaw_deg
}

fn parse_commanded_yaw_json(json: &str) -> Option<f64> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v.get("commanded_yaw_deg")?.as_f64()
}

/// Attach demand + exposure for one shot's acquisition window `[acq_start_ns, shot]`.
pub fn demand_for_shot(
    recon: &ReconstructedTrial,
    events: &[AimTargetEventRecord],
    _cand: &MovementCandidate,
    shot: &AimShotRecord,
    acq_start_ns: u64,
) -> (MovementDemand, ProcessorExposure) {
    let end_ns = shot.timestamp_ns;
    let cam: Vec<_> = recon
        .camera
        .iter()
        .filter(|p| p.timestamp_ns >= acq_start_ns && p.timestamp_ns <= end_ns)
        .collect();

    let angular_displacement_deg = if cam.len() >= 2 {
        let yaw: Vec<f64> = cam.iter().map(|p| p.unwrapped_yaw_deg).collect();
        let pitch: Vec<f64> = cam.iter().map(|p| p.pitch_deg).collect();
        path_length_yaw_pitch(&yaw, &pitch)
    } else {
        0.0
    };

    let target_distance_deg = cam.first().map(|start| {
        let target = target_dir(
            camera_origin(),
            [shot.target_x, shot.target_y, shot.target_z],
        );
        let aim = look_dir(
            crate::geometry::wrap_to_180(start.wrapped_yaw_deg),
            start.pitch_deg,
        );
        angular_distance_deg(aim, target)
    });

    let duration_ns = end_ns.saturating_sub(acq_start_ns);
    let movement_duration_ms = duration_ns as f64 / 1e6;

    let inputs: Vec<_> = recon
        .inputs
        .iter()
        .filter(|s| s.timestamp_ns >= acq_start_ns && s.timestamp_ns <= end_ns)
        .collect();

    let peak_physical = if inputs.is_empty() {
        None
    } else {
        Some(
            inputs
                .iter()
                .map(|s| s.physical_raw_speed)
                .fold(0.0_f64, f64::max),
        )
    };

    let peak_processed = if inputs.is_empty() {
        None
    } else {
        Some(
            inputs
                .iter()
                .map(|s| s.physical_processed_speed)
                .fold(0.0_f64, f64::max),
        )
    };

    let mut scale_peak = None;
    for s in &inputs {
        if let Some(scale) = s.acceleration_scale {
            scale_peak = Some(scale_peak.map_or(scale, |p: f64| p.max(scale)));
        }
    }

    let demand = MovementDemand {
        angular_displacement_deg,
        target_distance_deg,
        commanded_yaw_deg: commanded_yaw_for_target(events, &shot.target_id),
        peak_physical_input_speed: peak_physical,
        movement_duration_ms,
    };

    let exposure = ProcessorExposure {
        peak_processed_speed: peak_processed,
        peak_accel_scale: scale_peak,
    };

    (demand, exposure)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconstruct::{CameraPoint, InputPoint, ReconstructionStats};

    fn cam(t: u64, yaw: f64, pitch: f64) -> CameraPoint {
        CameraPoint {
            timestamp_ns: t,
            wrapped_yaw_deg: yaw,
            pitch_deg: pitch,
            unwrapped_yaw_deg: yaw,
            delta_yaw_deg: 0.0,
            delta_pitch_deg: 0.0,
            angular_speed_deg_s: 0.0,
        }
    }

    fn inp(t: u64, raw_speed: f64, proc_speed: f64, scale: Option<f64>) -> InputPoint {
        InputPoint {
            timestamp_ns: t,
            sequence_number: 0,
            raw_dx: 1,
            raw_dy: 0,
            processed_dx: 1.0,
            processed_dy: 0.0,
            dt_ns: 1_000_000,
            dt_used_ns: 1_000_000,
            physical_raw_speed: raw_speed,
            physical_processed_speed: proc_speed,
            acceleration_scale: scale,
            processor_input_speed: scale.map(|_| raw_speed),
        }
    }

    #[test]
    fn commanded_yaw_from_event_data_json() {
        let events = vec![AimTargetEventRecord {
            target_id: "target_001".into(),
            event_index: 0,
            timestamp_ns: 0,
            event_type: "spawn".into(),
            position_x: 0.0,
            position_y: 1.6,
            position_z: -6.0,
            yaw_deg: Some(60.0),
            pitch_deg: Some(0.0),
            velocity_x: 0.0,
            velocity_y: 0.0,
            velocity_z: 0.0,
            event_data_json: r#"{"radius":0.25,"commanded_yaw_deg":60}"#.into(),
        }];
        assert_eq!(
            commanded_yaw_for_target(&events, "target_001"),
            Some(60.0)
        );
    }

    #[test]
    fn accel_scale_only_on_processor_exposure() {
        let recon = ReconstructedTrial {
            camera: vec![cam(0, 0.0, 0.0), cam(100_000_000, 60.0, 0.0)],
            inputs: vec![
                inp(10_000_000, 2.0, 3.0, Some(1.5)),
                inp(50_000_000, 4.0, 6.0, Some(2.0)),
            ],
            stats: ReconstructionStats {
                max_abs_step_deg: 60.0,
                max_angular_speed_deg_s: 600.0,
                timestamp_monotonic: true,
                discontinuity_count: 0,
                yaw_wrap_crossings: 0,
            },
        };
        let events = vec![AimTargetEventRecord {
            target_id: "t1".into(),
            event_index: 0,
            timestamp_ns: 0,
            event_type: "spawn".into(),
            position_x: 0.0,
            position_y: 1.6,
            position_z: -6.0,
            yaw_deg: Some(60.0),
            pitch_deg: Some(0.0),
            velocity_x: 0.0,
            velocity_y: 0.0,
            velocity_z: 0.0,
            event_data_json: r#"{"commanded_yaw_deg":60}"#.into(),
        }];
        let shot = AimShotRecord {
            shot_index: 0,
            timestamp_ns: 100_000_000,
            hit: true,
            yaw_deg: 60.0,
            pitch_deg: 0.0,
            target_x: 0.0,
            target_y: 1.6,
            target_z: -6.0,
            target_radius: 0.25,
            target_id: "t1".into(),
        };
        let cand = MovementCandidate {
            id: 0,
            start_ns: 0,
            end_ns: 100_000_000,
            flags: vec![],
        };
        let (demand, exp) = demand_for_shot(&recon, &events, &cand, &shot, 0);
        assert_eq!(demand.commanded_yaw_deg, Some(60.0));
        assert!((demand.angular_displacement_deg - 60.0).abs() < 1e-9);
        assert_eq!(demand.peak_physical_input_speed, Some(4.0));
        assert_eq!(exp.peak_accel_scale, Some(2.0));
        assert_eq!(exp.peak_processed_speed, Some(6.0));
        let json = serde_json::to_value(&demand).unwrap();
        assert!(json.get("peak_accel_scale").is_none());
        assert!(json.get("peak_processed_speed").is_none());
    }

    #[test]
    fn commanded_yaw_unchanged_when_only_input_speeds_differ() {
        let events = vec![AimTargetEventRecord {
            target_id: "t1".into(),
            event_index: 0,
            timestamp_ns: 0,
            event_type: "spawn".into(),
            position_x: 1.0,
            position_y: 1.6,
            position_z: -6.0,
            yaw_deg: Some(30.0),
            pitch_deg: Some(0.0),
            velocity_x: 0.0,
            velocity_y: 0.0,
            velocity_z: 0.0,
            event_data_json: r#"{"commanded_yaw_deg":30}"#.into(),
        }];
        let shot = AimShotRecord {
            shot_index: 0,
            timestamp_ns: 50_000_000,
            hit: true,
            yaw_deg: 30.0,
            pitch_deg: 0.0,
            target_x: 1.0,
            target_y: 1.6,
            target_z: -6.0,
            target_radius: 0.25,
            target_id: "t1".into(),
        };
        let cand = MovementCandidate {
            id: 0,
            start_ns: 0,
            end_ns: 50_000_000,
            flags: vec![],
        };
        let cam_pts = vec![cam(0, 0.0, 0.0), cam(50_000_000, 30.0, 0.0)];
        let stats = ReconstructionStats {
            max_abs_step_deg: 30.0,
            max_angular_speed_deg_s: 600.0,
            timestamp_monotonic: true,
            discontinuity_count: 0,
            yaw_wrap_crossings: 0,
        };
        let none = ReconstructedTrial {
            camera: cam_pts.clone(),
            inputs: vec![inp(10_000_000, 1.0, 1.0, None)],
            stats: stats.clone(),
        };
        let linear = ReconstructedTrial {
            camera: cam_pts,
            inputs: vec![inp(10_000_000, 1.0, 2.5, Some(2.5))],
            stats,
        };
        let (d0, e0) = demand_for_shot(&none, &events, &cand, &shot, 0);
        let (d1, e1) = demand_for_shot(&linear, &events, &cand, &shot, 0);
        assert_eq!(d0.commanded_yaw_deg, d1.commanded_yaw_deg);
        assert_eq!(d0.angular_displacement_deg, d1.angular_displacement_deg);
        assert_eq!(d0.peak_physical_input_speed, d1.peak_physical_input_speed);
        assert!(e0.peak_accel_scale.is_none());
        assert_eq!(e1.peak_accel_scale, Some(2.5));
    }
}

use bevy::prelude::*;
use sense_accel::{create_processor, InputProcessor, RawAccelLinearConfig};
use sense_types::{InputCameraSample, MouseSample, ProcessedMouseSample};

use crate::{
    aim_trial::{left_button_down, AimPhase, AimTrial},
    config::{ExperimentSettings, LookCapture, TelemetryBuffers, ValidationState},
    input_plugin::ArcMouseQueue,
};

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct YawPitch {
    pub yaw_deg: f64,
    pub pitch_deg: f64,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct LiveInputStats {
    pub last_dx: i32,
    pub last_dy: i32,
    pub net_dx: i64,
    pub net_dy: i64,
    pub abs_dx: u64,
    pub total_yaw_delta_deg: f64,
    pub samples_this_frame: usize,
    pub last_raw_dt_ms: f64,
    pub last_speed_dt_ms: f64,
    pub last_input_speed: f64,
    pub last_acceleration_scale: f64,
    pub last_time_clamped: bool,
    pub last_bypassed_dt: bool,
    pub has_accel_debug: bool,
}

pub struct ActiveInputProcessor {
    pub processor: Box<dyn InputProcessor>,
}

impl Default for ActiveInputProcessor {
    fn default() -> Self {
        Self {
            processor: create_processor("none", &RawAccelLinearConfig::trainer_default())
                .expect("built-in processor must exist"),
        }
    }
}

#[derive(Resource, Debug, Default)]
pub struct ProcessorTimingState {
    pub last_raw_timestamp_ns: Option<u64>,
}

impl ProcessorTimingState {
    fn dt_s_for(&mut self, timestamp_ns: u64) -> f64 {
        let dt_s = sense_accel::dt_s_from_timestamps(self.last_raw_timestamp_ns, timestamp_ns);
        self.last_raw_timestamp_ns = Some(timestamp_ns);
        dt_s
    }
}

pub fn drain_mouse_to_camera(
    queue: Res<ArcMouseQueue>,
    mut camera: Single<&mut YawPitch, With<Camera3d>>,
    settings: Res<ExperimentSettings>,
    mut active_processor: NonSendMut<ActiveInputProcessor>,
    mut timing: ResMut<ProcessorTimingState>,
    mut live: ResMut<LiveInputStats>,
    mut buffers: ResMut<TelemetryBuffers>,
    validation: Res<ValidationState>,
    look: Res<LookCapture>,
    mut aim: ResMut<AimTrial>,
) {
    let samples = queue.0.drain_all();
    live.samples_this_frame = 0;
    // While the cursor is free for egui (ESC UI mode), discard queued samples so
    // pointer movement toward buttons does not rotate the camera or pollute
    // validation counters / telemetry.
    if !look.enabled {
        return;
    }
    let accumulate_stats = validation.is_running();
    for sample in samples {
        let dt_s = timing.dt_s_for(sample.timestamp_ns);
        let processor_id = active_processor.processor.id().to_string();
        let processor_version = active_processor.processor.version().to_string();
        let processor_config_json = active_processor.processor.config_json();
        let (processed_dx, processed_dy) =
            active_processor
                .processor
                .process(sample.dx as f64, sample.dy as f64, dt_s);
        if let Some(e) = active_processor.processor.last_linear_eval() {
            live.last_raw_dt_ms = e.raw_dt_ms;
            live.last_speed_dt_ms = e.dt_ms;
            live.last_input_speed = e.input_speed;
            live.last_acceleration_scale = e.acceleration_scale;
            live.last_time_clamped = e.time_clamped;
            live.last_bypassed_dt = e.bypassed_nonpositive_dt;
            live.has_accel_debug = true;
        } else {
            live.has_accel_debug = false;
        }
        let camera_sample = apply_sample(
            &sample,
            processed_dx,
            processed_dy,
            settings.sensitivity,
            &mut camera,
            &mut live,
            accumulate_stats,
        );
        if aim.phase == AimPhase::Armed && left_button_down(sample.buttons) {
            let origin = [
                crate::aim_trial::AIM_CAMERA_ORIGIN.x as f64,
                crate::aim_trial::AIM_CAMERA_ORIGIN.y as f64,
                crate::aim_trial::AIM_CAMERA_ORIGIN.z as f64,
            ];
            let dir = crate::aim_trial::look_direction_neg_z(camera.yaw_deg, camera.pitch_deg);
            let center = [
                crate::aim_trial::AIM_TARGET_CENTER.x as f64,
                crate::aim_trial::AIM_TARGET_CENTER.y as f64,
                crate::aim_trial::AIM_TARGET_CENTER.z as f64,
            ];
            let hit = sense_math::ray_sphere_hit(
                origin,
                dir,
                center,
                crate::aim_trial::AIM_TARGET_RADIUS as f64,
            );
            aim.last_hit = Some(hit);
            aim.last_yaw_deg = camera.yaw_deg;
            aim.last_pitch_deg = camera.pitch_deg;
            aim.last_timestamp_ns = sample.timestamp_ns;
            aim.phase = AimPhase::Idle;
        }
        if accumulate_stats {
            buffers.0.processed.push(ProcessedMouseSample {
                timestamp_ns: sample.timestamp_ns,
                sequence_number: sample.sequence_number,
                processed_dx,
                processed_dy,
                processor_id,
                processor_version,
                processor_config_json,
            });
            buffers.0.mouse.push(sample);
            buffers.0.input_camera.push(camera_sample);
        }
    }
}

pub fn apply_yaw_transform(camera: Single<(&YawPitch, &mut Transform), With<Camera3d>>) {
    let (pose, mut transform) = camera.into_inner();
    let yaw = Quat::from_rotation_y(-(pose.yaw_deg.to_radians() as f32));
    let pitch = Quat::from_rotation_x(pose.pitch_deg.to_radians() as f32);
    transform.rotation = yaw * pitch;
}

pub fn reset_camera(
    pose: &mut YawPitch,
    buffers: &mut TelemetryBuffers,
    validation: ValidationState,
    timestamp_ns: u64,
) {
    pose.yaw_deg = 0.0;
    pose.pitch_deg = 0.0;
    if validation.is_running() {
        buffers.0.input_camera.push(InputCameraSample {
            timestamp_ns,
            yaw_deg: pose.yaw_deg,
            pitch_deg: pose.pitch_deg,
            // Raw mouse sequences start at 1; zero identifies a camera-reset sample.
            sequence_number: 0,
        });
    }
}

fn apply_sample(
    sample: &MouseSample,
    processed_dx: f64,
    processed_dy: f64,
    sensitivity: f64,
    pose: &mut YawPitch,
    live: &mut LiveInputStats,
    accumulate_stats: bool,
) -> InputCameraSample {
    let yaw_delta_deg = sense_math::yaw_delta_deg(processed_dx, sensitivity);
    pose.yaw_deg += yaw_delta_deg;
    pose.pitch_deg =
        sense_math::apply_pitch_delta(pose.pitch_deg, processed_dy, sensitivity);
    live.last_dx = sample.dx;
    live.last_dy = sample.dy;
    if accumulate_stats {
        live.net_dx = live.net_dx.saturating_add(i64::from(sample.dx));
        live.net_dy = live.net_dy.saturating_add(i64::from(sample.dy));
        live.abs_dx = live
            .abs_dx
            .saturating_add(u64::from(sample.dx.unsigned_abs()));
        live.total_yaw_delta_deg += yaw_delta_deg;
    }
    live.samples_this_frame += 1;
    InputCameraSample {
        timestamp_ns: sample.timestamp_ns,
        yaw_deg: pose.yaw_deg,
        pitch_deg: pose.pitch_deg,
        sequence_number: sample.sequence_number,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn processor_timing_uses_consecutive_qpc_timestamps_across_drains() {
        let mut timing = ProcessorTimingState::default();

        assert_eq!(timing.dt_s_for(1_000_000_000), 0.0);
        assert!((timing.dt_s_for(1_004_000_000) - 0.004).abs() < f64::EPSILON);
        assert_eq!(timing.last_raw_timestamp_ns, Some(1_004_000_000));
    }

    #[test]
    fn yaw_and_pitch_update_from_processed_deltas() {
        let sample = MouseSample {
            timestamp_ns: 1,
            dx: 10,
            dy: -4,
            buttons: 0,
            sequence_number: 1,
        };
        let mut pose = YawPitch {
            yaw_deg: 2.0,
            pitch_deg: 11.0,
        };
        let mut live = LiveInputStats::default();
        apply_sample(&sample, 10.0, -4.0, 0.5, &mut pose, &mut live, true);
        assert_eq!(pose.yaw_deg, 2.35);
        // Negative processed_dy → look up (+pitch_deg increases).
        assert_eq!(pose.pitch_deg, 11.14);
        assert_eq!((live.net_dx, live.net_dy, live.abs_dx), (10, -4, 10));

        let down_sample = MouseSample {
            timestamp_ns: 2,
            dx: 0,
            dy: 100,
            buttons: 0,
            sequence_number: 2,
        };
        apply_sample(&down_sample, 0.0, 100.0, 0.5, &mut pose, &mut live, true);
        // Positive processed_dy → look down (+pitch_deg decreases).
        assert!(pose.pitch_deg < 11.14);
    }

    #[test]
    fn running_camera_reset_clears_yaw_and_pitch() {
        let mut pose = YawPitch {
            yaw_deg: 42.0,
            pitch_deg: 15.0,
        };
        let mut buffers = TelemetryBuffers::default();

        reset_camera(&mut pose, &mut buffers, ValidationState::Running, 123);

        assert_eq!(pose.yaw_deg, 0.0);
        assert_eq!(pose.pitch_deg, 0.0);
        assert_eq!(
            buffers.0.input_camera,
            vec![InputCameraSample {
                timestamp_ns: 123,
                yaw_deg: 0.0,
                pitch_deg: 0.0,
                sequence_number: 0,
            }]
        );
    }
}

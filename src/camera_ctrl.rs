use bevy::prelude::*;
use sense_types::{InputCameraSample, InputProcessor, MouseSample, NoAcceleration};

use crate::{
    config::{ExperimentSettings, TelemetryBuffers, ValidationState},
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
}

#[derive(Resource)]
pub struct InputProcessorState(pub NoAcceleration);

impl Default for InputProcessorState {
    fn default() -> Self {
        Self(NoAcceleration)
    }
}

pub fn drain_mouse_to_camera(
    queue: Res<ArcMouseQueue>,
    mut camera: Single<&mut YawPitch, With<Camera3d>>,
    settings: Res<ExperimentSettings>,
    mut processor: ResMut<InputProcessorState>,
    mut live: ResMut<LiveInputStats>,
    mut buffers: ResMut<TelemetryBuffers>,
    validation: Res<ValidationState>,
    time: Res<Time>,
) {
    let samples = queue.0.drain_all();
    live.samples_this_frame = 0;
    let sample_dt_s = if samples.is_empty() {
        0.0
    } else {
        time.delta_secs_f64() / samples.len() as f64
    };
    let accumulate_stats = validation.is_running();
    for sample in samples {
        let (processed_dx, _) =
            processor
                .0
                .process(sample.dx as f64, sample.dy as f64, sample_dt_s);
        let camera_sample = apply_sample(
            &sample,
            processed_dx,
            settings.sensitivity,
            &mut camera,
            &mut live,
            accumulate_stats,
        );
        if accumulate_stats {
            buffers.0.mouse.push(sample);
            buffers.0.input_camera.push(camera_sample);
        }
    }
}

pub fn apply_yaw_transform(camera: Single<(&YawPitch, &mut Transform), With<Camera3d>>) {
    let (pose, mut transform) = camera.into_inner();
    transform.rotation = Quat::from_rotation_y(-(pose.yaw_deg.to_radians() as f32));
}

pub fn reset_camera(
    pose: &mut YawPitch,
    buffers: &mut TelemetryBuffers,
    validation: ValidationState,
    timestamp_ns: u64,
) {
    pose.yaw_deg = 0.0;
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
    sensitivity: f64,
    pose: &mut YawPitch,
    live: &mut LiveInputStats,
    accumulate_stats: bool,
) -> InputCameraSample {
    let yaw_delta_deg = sense_math::yaw_delta_deg(processed_dx, sensitivity);
    pose.yaw_deg += yaw_delta_deg;
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
    fn yaw_updates_and_pitch_remains_frozen() {
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
        apply_sample(&sample, 10.0, 0.5, &mut pose, &mut live, true);
        assert_eq!(pose.yaw_deg, 2.35);
        assert_eq!(pose.pitch_deg, 11.0);
        assert_eq!((live.net_dx, live.net_dy, live.abs_dx), (10, -4, 10));
    }

    #[test]
    fn running_camera_reset_records_zero_yaw_sample() {
        let mut pose = YawPitch {
            yaw_deg: 42.0,
            pitch_deg: 0.0,
        };
        let mut buffers = TelemetryBuffers::default();

        reset_camera(&mut pose, &mut buffers, ValidationState::Running, 123);

        assert_eq!(pose.yaw_deg, 0.0);
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

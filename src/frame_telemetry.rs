use bevy::prelude::*;
use sense_types::FrameSample;

use crate::config::{TelemetryBuffers, ValidationState};

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct LiveFrameStats {
    pub frame_time_s: f64,
    pub fps: f64,
}

pub fn record_frame_telemetry(
    time: Res<Time>,
    validation: Res<ValidationState>,
    mut live: ResMut<LiveFrameStats>,
    mut buffers: ResMut<TelemetryBuffers>,
) {
    let sample = frame_sample(sense_input_win::monotonic_now_ns(), time.delta_secs_f64());
    live.frame_time_s = sample.frame_time_s;
    live.fps = sample.fps;
    if validation.is_running() {
        buffers.0.frames.push(sample);
    }
}

fn frame_sample(timestamp_ns: u64, frame_time_s: f64) -> FrameSample {
    FrameSample {
        timestamp_ns,
        frame_time_s,
        fps: if frame_time_s > 0.0 {
            1.0 / frame_time_s
        } else {
            0.0
        },
    }
}

#[cfg(test)]
mod tests {
    use super::frame_sample;

    #[test]
    fn frame_sample_is_separate_and_finite() {
        assert_eq!(frame_sample(42, 0.002).fps, 500.0);
        assert_eq!(frame_sample(42, 0.0).fps, 0.0);
    }
}

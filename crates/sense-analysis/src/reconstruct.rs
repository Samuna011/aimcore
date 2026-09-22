use crate::geometry::{unwrap_yaw_series, wrap_to_180};
use crate::{AnalysisConfig, AnalysisError};
use sense_telemetry::AimTrialAnalysisBundle;
use sense_types::{AimCameraSampleRecord, AimInputSampleRecord};

#[derive(Debug, Clone, PartialEq)]
pub struct CameraPoint {
    pub timestamp_ns: u64,
    /// As stored in telemetry (may accumulate beyond ±180).
    pub wrapped_yaw_deg: f64,
    pub pitch_deg: f64,
    /// Continuous yaw after shortest-step unwrap (degrees).
    pub unwrapped_yaw_deg: f64,
    /// Shortest-step Δyaw from previous sample (0 at first).
    pub delta_yaw_deg: f64,
    pub delta_pitch_deg: f64,
    /// √(Δyaw²+Δpitch²) / dt — uses unwrapped yaw steps.
    pub angular_speed_deg_s: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputPoint {
    pub timestamp_ns: u64,
    pub sequence_number: u64,
    pub raw_dx: i32,
    pub raw_dy: i32,
    pub processed_dx: f64,
    pub processed_dy: f64,
    pub dt_ns: u64,
    pub dt_used_ns: u64,
    /// √(raw_dx²+raw_dy²) / (dt_ns → ms). Physical QPC speed; comparable across processors.
    pub physical_raw_speed: f64,
    /// √(processed_dx²+processed_dy²) / (dt_ns → ms).
    pub physical_processed_speed: f64,
    pub acceleration_scale: Option<f64>,
    /// Stored processor-path speed (`input_speed` column). Never invented for `none`.
    pub processor_input_speed: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReconstructionStats {
    pub max_abs_step_deg: f64,
    pub max_angular_speed_deg_s: f64,
    pub timestamp_monotonic: bool,
    /// Count of reconstruction defects (zero-dt, unwrap inconsistency, huge *single-sample* step).
    /// Does **not** count merely-fast legitimate flicks.
    pub discontinuity_count: u32,
    /// Times naïve `yaw_i - yaw_{i-1}` exceeded ±180° (wrap seam crossed); unwrapped path used instead.
    pub yaw_wrap_crossings: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReconstructedTrial {
    pub camera: Vec<CameraPoint>,
    pub inputs: Vec<InputPoint>,
    pub stats: ReconstructionStats,
}

pub fn reconstruct(
    bundle: &AimTrialAnalysisBundle,
    config: &AnalysisConfig,
) -> Result<ReconstructedTrial, AnalysisError> {
    let (camera, stats) = reconstruct_camera(&bundle.camera_samples, config)?;
    let inputs = reconstruct_inputs(&bundle.input_samples);
    Ok(ReconstructedTrial {
        camera,
        inputs,
        stats,
    })
}

fn reconstruct_camera(
    samples: &[AimCameraSampleRecord],
    config: &AnalysisConfig,
) -> Result<(Vec<CameraPoint>, ReconstructionStats), AnalysisError> {
    if samples.is_empty() {
        return Err(AnalysisError::InsufficientTelemetry(
            "no camera samples".into(),
        ));
    }

    let wrapped: Vec<f64> = samples.iter().map(|s| s.yaw_deg).collect();
    let unwrapped = unwrap_yaw_series(&wrapped);

    let mut out = Vec::with_capacity(samples.len());
    let mut max_abs_step = 0.0_f64;
    let mut max_speed = 0.0_f64;
    let mut timestamp_monotonic = true;
    let mut discontinuity_count = 0u32;
    let mut yaw_wrap_crossings = 0u32;

    for (i, s) in samples.iter().enumerate() {
        let (delta_yaw, delta_pitch, speed) = if i == 0 {
            (0.0, 0.0, 0.0)
        } else {
            let prev = &samples[i - 1];
            if s.timestamp_ns < prev.timestamp_ns {
                timestamp_monotonic = false;
            }
            let dt_ns = s.timestamp_ns.saturating_sub(prev.timestamp_ns);
            let dy = unwrapped[i] - unwrapped[i - 1];
            let dp = s.pitch_deg - prev.pitch_deg;

            // Naïve subtraction crossing ±180 is a wrap seam — expected in long spins;
            // we use unwrapped Δ. Count for diagnostics only (not “bad play”).
            let naive = s.yaw_deg - prev.yaw_deg;
            if naive.abs() > 180.0 {
                yaw_wrap_crossings += 1;
            }

            let wrapped_step = wrap_to_180(naive);
            if (wrapped_step - dy).abs() > 1e-6 {
                discontinuity_count += 1;
            }
            let step = (dy * dy + dp * dp).sqrt();
            max_abs_step = max_abs_step.max(step);

            // A single-sample unwrapped jump this large is still a glitch (not a 180° flick,
            // which spans many samples). High °/s across many samples is legitimate Behavior.
            if step > config.max_step_deg {
                discontinuity_count += 1;
            }

            let speed = if dt_ns == 0 {
                discontinuity_count += 1;
                0.0
            } else {
                let dt_eff_ns = dt_ns.max(config.min_dt_ns_for_speed);
                let dt_s = dt_eff_ns as f64 / 1e9;
                step / dt_s
            };
            max_speed = max_speed.max(speed);
            (dy, dp, speed)
        };

        out.push(CameraPoint {
            timestamp_ns: s.timestamp_ns,
            wrapped_yaw_deg: s.yaw_deg,
            pitch_deg: s.pitch_deg,
            unwrapped_yaw_deg: unwrapped[i],
            delta_yaw_deg: delta_yaw,
            delta_pitch_deg: delta_pitch,
            angular_speed_deg_s: speed,
        });
    }

    if !timestamp_monotonic {
        return Err(AnalysisError::ReconstructionFailed(
            "camera timestamps are not monotonic".into(),
        ));
    }

    let stats = ReconstructionStats {
        max_abs_step_deg: max_abs_step,
        max_angular_speed_deg_s: max_speed,
        timestamp_monotonic,
        discontinuity_count,
        yaw_wrap_crossings,
    };

    Ok((out, stats))
}

fn reconstruct_inputs(samples: &[AimInputSampleRecord]) -> Vec<InputPoint> {
    samples
        .iter()
        .map(|s| {
            // Physical speeds always use raw QPC dt_ns (not processor dt_used_ns).
            let dt_ms = if s.dt_ns > 0 {
                s.dt_ns as f64 / 1e6
            } else {
                0.0
            };
            let raw_mag = ((s.raw_dx as f64).powi(2) + (s.raw_dy as f64).powi(2)).sqrt();
            let proc_mag = (s.processed_dx.powi(2) + s.processed_dy.powi(2)).sqrt();
            let physical_raw_speed = if dt_ms > 0.0 { raw_mag / dt_ms } else { 0.0 };
            let physical_processed_speed = if dt_ms > 0.0 {
                proc_mag / dt_ms
            } else {
                0.0
            };
            InputPoint {
                timestamp_ns: s.timestamp_ns,
                sequence_number: s.sequence_number,
                raw_dx: s.raw_dx,
                raw_dy: s.raw_dy,
                processed_dx: s.processed_dx,
                processed_dy: s.processed_dy,
                dt_ns: s.dt_ns,
                dt_used_ns: s.dt_used_ns,
                physical_raw_speed,
                physical_processed_speed,
                acceleration_scale: s.acceleration_scale,
                processor_input_speed: s.input_speed,
            }
        })
        .collect()
}

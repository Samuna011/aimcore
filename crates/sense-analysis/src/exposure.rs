use crate::reconstruct::ReconstructedTrial;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExposureMetrics {
    pub raw_speed_mean: f64,
    pub raw_speed_peak: f64,
    pub processed_speed_mean: f64,
    pub processed_speed_peak: f64,
    pub acceleration_scale_mean: Option<f64>,
    pub acceleration_scale_peak: Option<f64>,
    pub gain_ratio_mean: f64,
    pub gain_ratio_peak: f64,
    pub cap_exposure: f64,
    pub cap_applicable: bool,
    pub processor_time_ns: u64,
}

pub fn exposure_for_interval(
    recon: &ReconstructedTrial,
    start_ns: u64,
    end_ns: u64,
    processor_id: &str,
    processor_config_json: &str,
) -> ExposureMetrics {
    let samples: Vec<_> = recon
        .inputs
        .iter()
        .filter(|s| s.timestamp_ns >= start_ns && s.timestamp_ns <= end_ns)
        .collect();

    let mut raw_sum = 0.0_f64;
    let mut raw_peak = 0.0_f64;
    let mut proc_sum = 0.0_f64;
    let mut proc_peak = 0.0_f64;
    let mut scale_sum = 0.0_f64;
    let mut scale_n = 0usize;
    let mut scale_peak = 0.0_f64;
    let mut gain_sum = 0.0_f64;
    let mut gain_peak = 0.0_f64;
    let mut processor_time = 0u64;
    let mut cap_hits = 0usize;

    let cap_scale = capped_scale_value(processor_id, processor_config_json);
    let cap_applicable = cap_scale.is_some();

    for s in &samples {
        raw_sum += s.raw_speed;
        raw_peak = raw_peak.max(s.raw_speed);
        proc_sum += s.processed_speed;
        proc_peak = proc_peak.max(s.processed_speed);
        processor_time = processor_time.saturating_add(s.dt_used_ns);

        let raw_mag = ((s.raw_dx as f64).powi(2) + (s.raw_dy as f64).powi(2)).sqrt();
        let proc_mag = (s.processed_dx.powi(2) + s.processed_dy.powi(2)).sqrt();
        let gain = if raw_mag < 1e-9 { 1.0 } else { proc_mag / raw_mag };
        gain_sum += gain;
        gain_peak = gain_peak.max(gain);

        if let Some(scale) = s.acceleration_scale {
            scale_sum += scale;
            scale_n += 1;
            scale_peak = scale_peak.max(scale);
            if let Some(cap) = cap_scale {
                if (scale - cap).abs() <= 1e-3 {
                    cap_hits += 1;
                }
            }
        }
    }

    let n = samples.len().max(1) as f64;
    ExposureMetrics {
        raw_speed_mean: raw_sum / n,
        raw_speed_peak: raw_peak,
        processed_speed_mean: proc_sum / n,
        processed_speed_peak: proc_peak,
        acceleration_scale_mean: if scale_n > 0 {
            Some(scale_sum / scale_n as f64)
        } else {
            None
        },
        acceleration_scale_peak: if scale_n > 0 { Some(scale_peak) } else { None },
        gain_ratio_mean: gain_sum / n,
        gain_ratio_peak: gain_peak,
        cap_exposure: if samples.is_empty() || !cap_applicable {
            0.0
        } else {
            cap_hits as f64 / samples.len() as f64
        },
        cap_applicable,
        processor_time_ns: processor_time,
    }
}

/// Peak/capped scale for Linear when caps active; `None` if not applicable.
fn capped_scale_value(processor_id: &str, processor_config_json: &str) -> Option<f64> {
    if processor_id != "rawaccel_linear" {
        return None;
    }
    let cfg = parse_linear_config(processor_config_json)?;
    let caps_active = match cfg.cap_mode {
        sense_accel::CapMode::Out => cfg.cap_y > 0.0,
        sense_accel::CapMode::In | sense_accel::CapMode::Io => cfg.cap_x > 0.0,
    };
    if !caps_active {
        return None;
    }
    let classic = sense_accel::ClassicLinearState::new(sense_accel::ClassicLinearArgs {
        acceleration: cfg.acceleration,
        input_offset: cfg.input_offset,
        gain: cfg.gain,
        cap_mode: cfg.cap_mode,
        cap_x: cfg.cap_x,
        cap_y: cfg.cap_y,
    });
    Some(classic.scale(1.0e6))
}

pub(crate) fn parse_linear_config(json: &str) -> Option<sense_accel::RawAccelLinearConfig> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let acceleration = v.get("acceleration")?.as_f64()?;
    let sensitivity_multiplier = v
        .get("sensitivity_multiplier")
        .and_then(|x| x.as_f64())
        .unwrap_or(1.0);
    let gain = v.get("gain").and_then(|x| x.as_bool()).unwrap_or(false);
    let input_offset = v.get("input_offset").and_then(|x| x.as_f64()).unwrap_or(0.0);
    let cap_mode = match v.get("cap_mode").and_then(|x| x.as_str()).unwrap_or("out") {
        "io" => sense_accel::CapMode::Io,
        "in" => sense_accel::CapMode::In,
        _ => sense_accel::CapMode::Out,
    };
    let cap_x = v.get("cap_x").and_then(|x| x.as_f64()).unwrap_or(0.0);
    let cap_y = v.get("cap_y").and_then(|x| x.as_f64()).unwrap_or(0.0);
    let polling_rate_hz = v
        .get("polling_rate_hz")
        .and_then(|x| x.as_u64())
        .unwrap_or(0) as u32;
    let cfg = sense_accel::RawAccelLinearConfig {
        acceleration,
        sensitivity_multiplier,
        gain,
        input_offset,
        cap_mode,
        cap_x,
        cap_y,
        polling_rate_hz,
    };
    cfg.validate().ok()?;
    Some(cfg)
}

pub(crate) fn expected_scale_for_sample(
    processor_id: &str,
    processor_config_json: &str,
    raw_dx: i32,
    raw_dy: i32,
    dt_used_ns: u64,
) -> Option<f64> {
    if processor_id == "none" {
        return Some(1.0);
    }
    if processor_id != "rawaccel_linear" {
        return None;
    }
    let cfg = parse_linear_config(processor_config_json)?;
    let dt_s = dt_used_ns as f64 / 1e9;
    if dt_s <= 0.0 {
        return Some(1.0);
    }
    let eval = sense_accel::eval_rawaccel_linear_config(
        raw_dx as f64,
        raw_dy as f64,
        dt_s,
        &cfg,
    );
    Some(eval.acceleration_scale)
}

//! Input processor trait, factory, and QPC-derived inter-sample timing.

mod classic_linear;

pub use classic_linear::*;

pub trait InputProcessor: Send {
    fn id(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn config_json(&self) -> String;
    fn process(&mut self, dx: f64, dy: f64, dt_s: f64) -> (f64, f64);
}

pub struct NoAcceleration;

impl InputProcessor for NoAcceleration {
    fn id(&self) -> &'static str {
        "none"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn config_json(&self) -> String {
        "{}".into()
    }

    fn process(&mut self, dx: f64, dy: f64, _dt_s: f64) -> (f64, f64) {
        (dx, dy)
    }
}

pub struct RawAccelLinear {
    pub acceleration: f64,
    pub sensitivity_multiplier: f64,
}

impl InputProcessor for RawAccelLinear {
    fn id(&self) -> &'static str {
        "rawaccel_linear"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn config_json(&self) -> String {
        format!(
            "{{\"acceleration\":{},\"sensitivity_multiplier\":{}}}",
            self.acceleration, self.sensitivity_multiplier
        )
    }

    fn process(&mut self, dx: f64, dy: f64, dt_s: f64) -> (f64, f64) {
        let e = eval_rawaccel_linear(dx, dy, dt_s, self.acceleration, self.sensitivity_multiplier);
        (e.processed_dx, e.processed_dy)
    }
}

pub fn create_processor(
    id: &str,
    acceleration: f64,
    sensitivity_multiplier: f64,
) -> Result<Box<dyn InputProcessor>, String> {
    match id {
        "none" => Ok(Box::new(NoAcceleration)),
        "rawaccel_linear" => Ok(Box::new(RawAccelLinear {
            acceleration,
            sensitivity_multiplier,
        })),
        _ => Err(format!("unknown processor id: {id}")),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinearEval {
    pub raw_dx: f64,
    pub raw_dy: f64,
    pub dt_ms: f64,
    pub input_speed: f64,
    pub acceleration_scale: f64,
    pub processed_dx: f64,
    pub processed_dy: f64,
    pub bypassed_nonpositive_dt: bool,
}

pub fn vector_speed_counts_per_ms(dx: f64, dy: f64, dt_ms: f64) -> f64 {
    (dx * dx + dy * dy).sqrt() / dt_ms
}

pub fn linear_acceleration_scale(v: f64, acceleration: f64) -> f64 {
    1.0 + acceleration * v
}

pub fn apply_whole(
    dx: f64,
    dy: f64,
    acceleration_scale: f64,
    sensitivity_multiplier: f64,
) -> (f64, f64) {
    let factor = acceleration_scale * sensitivity_multiplier;
    (dx * factor, dy * factor)
}

/// Raw Accel Linear (Legacy / Sensitivity), reproduced according to the official
/// implementation; mathematically equivalent to Classic with exponent 2 where the
/// documented equivalence applies.
/// Demonstrates documented mathematical behavior — not actual-driver parity.
pub fn eval_rawaccel_linear(
    dx: f64,
    dy: f64,
    dt_s: f64,
    acceleration: f64,
    sensitivity_multiplier: f64,
) -> LinearEval {
    let dt_ms = dt_s * 1000.0;
    if dt_ms <= 0.0 {
        return LinearEval {
            raw_dx: dx,
            raw_dy: dy,
            dt_ms,
            input_speed: 0.0,
            acceleration_scale: 1.0,
            processed_dx: dx,
            processed_dy: dy,
            bypassed_nonpositive_dt: true,
        };
    }

    let input_speed = vector_speed_counts_per_ms(dx, dy, dt_ms);
    let acceleration_scale = linear_acceleration_scale(input_speed, acceleration);
    let (processed_dx, processed_dy) =
        apply_whole(dx, dy, acceleration_scale, sensitivity_multiplier);

    LinearEval {
        raw_dx: dx,
        raw_dy: dy,
        dt_ms,
        input_speed,
        acceleration_scale,
        processed_dx,
        processed_dy,
        bypassed_nonpositive_dt: false,
    }
}

/// Inter-sample interval from consecutive raw QPC timestamps (nanoseconds).
///
/// Uses only monotonic raw mouse-event timestamps — never render-frame delta,
/// FPS, or VSync timing. First sample in a burst (no previous timestamp): `0.0`.
pub fn dt_s_from_timestamps(prev_ns: Option<u64>, current_ns: u64) -> f64 {
    match prev_ns {
        None => 0.0,
        Some(prev) => (current_ns.saturating_sub(prev)) as f64 / 1_000_000_000.0,
    }
}

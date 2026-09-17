//! Input processor trait, factory, and QPC-derived inter-sample timing.

mod classic_linear;

pub use classic_linear::*;

pub trait InputProcessor: Send {
    fn id(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn config_json(&self) -> String;
    fn process(&mut self, dx: f64, dy: f64, dt_s: f64) -> (f64, f64);
    /// Last Linear eval debug (rawaccel_linear only).
    fn last_linear_eval(&self) -> Option<&LinearEval> {
        None
    }
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

/// Raw Accel default time clamp floor when `polling_rate_hz == 0` (`DEFAULT_TIME_MIN`).
pub const RA_DEFAULT_TIME_MIN_MS: f64 = 1000.0 / 8000.0 / 2.0;
/// Raw Accel default time clamp ceiling (`DEFAULT_TIME_MAX`).
pub const RA_DEFAULT_TIME_MAX_MS: f64 = 100.0;

#[derive(Debug, Clone, PartialEq)]
pub struct RawAccelLinearConfig {
    pub acceleration: f64,
    pub sensitivity_multiplier: f64,
    pub gain: bool,
    pub input_offset: f64,
    pub cap_mode: CapMode,
    pub cap_x: f64,
    pub cap_y: f64,
    /// EXPLICIT trainer: when `> 0`, speed `dt_ms` floor is `1000 / polling_rate_hz`
    /// (same formula as RA Device poll). `0` uses [`RA_DEFAULT_TIME_MIN_MS`] only.
    pub polling_rate_hz: u32,
}

impl RawAccelLinearConfig {
    pub fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("acceleration", self.acceleration),
            ("sensitivity_multiplier", self.sensitivity_multiplier),
            ("input_offset", self.input_offset),
            ("cap_x", self.cap_x),
            ("cap_y", self.cap_y),
        ] {
            if !value.is_finite() {
                return Err(format!("{name} must be finite"));
            }
        }

        if self.input_offset < 0.0 {
            return Err("input_offset must be greater than or equal to 0".into());
        }

        match self.cap_mode {
            CapMode::Io if self.cap_x <= self.input_offset => {
                return Err("io cap_x must be greater than input_offset".into());
            }
            CapMode::Out
                if self.gain
                    && self.cap_y > 0.0
                    && self.cap_y != 1.0
                    && self.acceleration == 0.0 =>
            {
                return Err("gain out acceleration must be nonzero when cap_y is active".into());
            }
            _ => {}
        }

        Ok(())
    }

    /// Phase 1 Guide / verification: Sensitivity, inactive output cap.
    pub fn phase1_sensitivity(acceleration: f64, sensitivity_multiplier: f64) -> Self {
        Self {
            acceleration,
            sensitivity_multiplier,
            gain: false,
            input_offset: 0.0,
            cap_mode: CapMode::Out,
            cap_x: 0.0,
            cap_y: 0.0,
            polling_rate_hz: 0,
        }
    }

    /// Trainer play-oriented defaults: Gain + Output 2, a=0.007, m=1, poll floor 1000 Hz.
    pub fn trainer_default() -> Self {
        Self {
            acceleration: 0.007,
            sensitivity_multiplier: 1.0,
            gain: true,
            input_offset: 0.0,
            cap_mode: CapMode::Out,
            cap_x: 2.0,
            cap_y: 2.0,
            polling_rate_hz: 1000,
        }
    }
}

/// Speed-path time clamp (EXPLICIT trainer / RA-shaped). Does not alter bypass.
pub fn clamp_speed_dt_ms(raw_dt_ms: f64, polling_rate_hz: u32) -> f64 {
    let min_ms = if polling_rate_hz > 0 {
        1000.0 / f64::from(polling_rate_hz)
    } else {
        RA_DEFAULT_TIME_MIN_MS
    };
    raw_dt_ms.clamp(min_ms, RA_DEFAULT_TIME_MAX_MS)
}

pub struct RawAccelLinear {
    config: RawAccelLinearConfig,
    classic: ClassicLinearState,
    last_eval: Option<LinearEval>,
}

impl RawAccelLinear {
    pub fn new(config: RawAccelLinearConfig) -> Self {
        let classic = ClassicLinearState::new(ClassicLinearArgs {
            acceleration: config.acceleration,
            input_offset: config.input_offset,
            gain: config.gain,
            cap_mode: config.cap_mode,
            cap_x: config.cap_x,
            cap_y: config.cap_y,
        });
        Self {
            config,
            classic,
            last_eval: None,
        }
    }
}

impl InputProcessor for RawAccelLinear {
    fn id(&self) -> &'static str {
        "rawaccel_linear"
    }

    fn version(&self) -> &'static str {
        "1.2.0"
    }

    fn config_json(&self) -> String {
        let cap_mode = match self.config.cap_mode {
            CapMode::Out => "out",
            CapMode::In => "in",
            CapMode::Io => "io",
        };
        format!(
            concat!(
                "{{\"acceleration\":{},\"sensitivity_multiplier\":{},",
                "\"gain\":{},\"input_offset\":{},\"cap_mode\":\"{}\",",
                "\"cap_x\":{},\"cap_y\":{},\"polling_rate_hz\":{}}}"
            ),
            self.config.acceleration,
            self.config.sensitivity_multiplier,
            self.config.gain,
            self.config.input_offset,
            cap_mode,
            self.config.cap_x,
            self.config.cap_y,
            self.config.polling_rate_hz,
        )
    }

    fn process(&mut self, dx: f64, dy: f64, dt_s: f64) -> (f64, f64) {
        let e = eval_rawaccel_linear_with_state(dx, dy, dt_s, &self.config, &self.classic);
        let out = (e.processed_dx, e.processed_dy);
        self.last_eval = Some(e);
        out
    }

    fn last_linear_eval(&self) -> Option<&LinearEval> {
        self.last_eval.as_ref()
    }
}

pub fn create_processor(
    id: &str,
    config: &RawAccelLinearConfig,
) -> Result<Box<dyn InputProcessor>, String> {
    match id {
        "none" => Ok(Box::new(NoAcceleration)),
        "rawaccel_linear" => {
            config
                .validate()
                .map_err(|error| format!("invalid rawaccel_linear config: {error}"))?;
            Ok(Box::new(RawAccelLinear::new(config.clone())))
        }
        _ => Err(format!("unknown processor id: {id}")),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinearEval {
    pub raw_dx: f64,
    pub raw_dy: f64,
    /// QPC-derived interval before speed clamp.
    pub raw_dt_ms: f64,
    /// Interval used for `input_speed` (after clamp), or raw on bypass.
    pub dt_ms: f64,
    pub input_speed: f64,
    pub acceleration_scale: f64,
    pub processed_dx: f64,
    pub processed_dy: f64,
    pub bypassed_nonpositive_dt: bool,
    pub time_clamped: bool,
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
    let config = RawAccelLinearConfig::phase1_sensitivity(acceleration, sensitivity_multiplier);
    eval_rawaccel_linear_config(dx, dy, dt_s, &config)
}

pub fn eval_rawaccel_linear_config(
    dx: f64,
    dy: f64,
    dt_s: f64,
    config: &RawAccelLinearConfig,
) -> LinearEval {
    let classic = ClassicLinearState::new(ClassicLinearArgs {
        acceleration: config.acceleration,
        input_offset: config.input_offset,
        gain: config.gain,
        cap_mode: config.cap_mode,
        cap_x: config.cap_x,
        cap_y: config.cap_y,
    });
    eval_rawaccel_linear_with_state(dx, dy, dt_s, config, &classic)
}

fn eval_rawaccel_linear_with_state(
    dx: f64,
    dy: f64,
    dt_s: f64,
    config: &RawAccelLinearConfig,
    classic: &ClassicLinearState,
) -> LinearEval {
    let raw_dt_ms = dt_s * 1000.0;
    if raw_dt_ms <= 0.0 {
        return LinearEval {
            raw_dx: dx,
            raw_dy: dy,
            raw_dt_ms,
            dt_ms: raw_dt_ms,
            input_speed: 0.0,
            acceleration_scale: 1.0,
            processed_dx: dx,
            processed_dy: dy,
            bypassed_nonpositive_dt: true,
            time_clamped: false,
        };
    }

    let dt_ms = clamp_speed_dt_ms(raw_dt_ms, config.polling_rate_hz);
    let time_clamped = (dt_ms - raw_dt_ms).abs() > 1e-15;
    let input_speed = vector_speed_counts_per_ms(dx, dy, dt_ms);
    let acceleration_scale = classic.scale(input_speed);
    let (processed_dx, processed_dy) =
        apply_whole(dx, dy, acceleration_scale, config.sensitivity_multiplier);

    LinearEval {
        raw_dx: dx,
        raw_dy: dy,
        raw_dt_ms,
        dt_ms,
        input_speed,
        acceleration_scale,
        processed_dx,
        processed_dy,
        bypassed_nonpositive_dt: false,
        time_clamped,
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

use bevy::prelude::Resource;
use sense_accel::{CapMode, RawAccelLinearConfig};
use sense_telemetry::SessionBuffers;
use sense_types::{FovAxis, SensitivityConfig};

/// When `true`, cursor is locked for look/validation input.
/// When `false`, cursor is free so egui Validation Lab buttons can be clicked.
/// Toggle with Escape. Starts unlocked so Start/Reset are usable immediately.
#[derive(Resource, Debug, Clone, Copy)]
pub struct LookCapture {
    pub enabled: bool,
}

impl Default for LookCapture {
    fn default() -> Self {
        Self { enabled: false }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct ExperimentSettings {
    pub dpi: f64,
    pub sensitivity: f64,
    pub fov_degrees_h: f64,
    pub processor_id: String,
    pub acceleration: f64,
    pub sensitivity_multiplier: f64,
    pub gain: bool,
    pub input_offset: f64,
    pub cap_mode: CapMode,
    pub cap_x: f64,
    pub cap_y: f64,
    /// EXPLICIT trainer poll-period floor for speed dt (0 = RA default min only).
    pub polling_rate_hz: u32,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationState {
    Idle,
    Running,
}

impl Default for ValidationState {
    fn default() -> Self {
        Self::Idle
    }
}

impl ValidationState {
    pub fn is_running(self) -> bool {
        matches!(self, Self::Running)
    }
}

#[derive(Resource, Debug, Default)]
pub struct TelemetryBuffers(pub SessionBuffers);

impl Default for ExperimentSettings {
    fn default() -> Self {
        let rawaccel = RawAccelLinearConfig::trainer_default();
        // Declared hardware DPI for eDPI / cm/360 metadata.
        // Does NOT enter camera yaw math (degrees_per_count uses sensitivity only).
        Self {
            dpi: 3200.0,
            sensitivity: 0.09,
            fov_degrees_h: 103.0,
            processor_id: "none".into(),
            // Trainer default only; this is not the official Raw Accel default.
            acceleration: rawaccel.acceleration,
            sensitivity_multiplier: rawaccel.sensitivity_multiplier,
            gain: rawaccel.gain,
            input_offset: rawaccel.input_offset,
            cap_mode: rawaccel.cap_mode,
            cap_x: rawaccel.cap_x,
            cap_y: rawaccel.cap_y,
            polling_rate_hz: rawaccel.polling_rate_hz,
        }
    }
}

impl ExperimentSettings {
    pub fn sensitivity_config(&self) -> SensitivityConfig {
        SensitivityConfig {
            dpi: self.dpi,
            sensitivity: self.sensitivity,
            yaw_deg_per_count_at_sens_1: sense_math::VALORANT_YAW_DEG_PER_COUNT_AT_SENS_1,
            fov_axis: FovAxis::Horizontal,
            fov_degrees: self.fov_degrees_h,
        }
    }

    pub fn rawaccel_linear_config(&self) -> RawAccelLinearConfig {
        RawAccelLinearConfig {
            acceleration: self.acceleration,
            sensitivity_multiplier: self.sensitivity_multiplier,
            gain: self.gain,
            input_offset: self.input_offset,
            cap_mode: self.cap_mode,
            cap_x: self.cap_x,
            cap_y: self.cap_y,
            polling_rate_hz: self.polling_rate_hz,
        }
    }
}

#[cfg(test)]
mod tests {
    use sense_accel::{CapMode, RawAccelLinearConfig};

    use super::{ExperimentSettings, ValidationState};

    #[test]
    fn validation_starts_idle() {
        assert_eq!(ValidationState::default(), ValidationState::Idle);
    }

    #[test]
    fn processor_defaults_to_none() {
        assert_eq!(ExperimentSettings::default().processor_id, "none");
    }

    #[test]
    fn rawaccel_linear_settings_use_trainer_defaults() {
        let settings = ExperimentSettings::default();

        assert_eq!(settings.acceleration, 0.007);
        assert_eq!(settings.sensitivity_multiplier, 1.0);
        assert!(settings.gain);
        assert_eq!(settings.input_offset, 0.0);
        assert_eq!(settings.cap_mode, CapMode::Out);
        assert_eq!(settings.cap_x, 2.0);
        assert_eq!(settings.cap_y, 2.0);
        assert_eq!(settings.polling_rate_hz, 1000);
    }

    #[test]
    fn rawaccel_linear_config_copies_experiment_settings() {
        let settings = ExperimentSettings {
            acceleration: 0.125,
            sensitivity_multiplier: 0.75,
            gain: false,
            input_offset: 1.5,
            cap_mode: CapMode::Io,
            cap_x: 4.0,
            cap_y: 2.5,
            polling_rate_hz: 500,
            ..ExperimentSettings::default()
        };

        assert_eq!(
            settings.rawaccel_linear_config(),
            RawAccelLinearConfig {
                acceleration: 0.125,
                sensitivity_multiplier: 0.75,
                gain: false,
                input_offset: 1.5,
                cap_mode: CapMode::Io,
                cap_x: 4.0,
                cap_y: 2.5,
                polling_rate_hz: 500,
            }
        );
    }
}

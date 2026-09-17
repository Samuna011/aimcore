use bevy::prelude::Resource;
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
        // Declared hardware DPI for eDPI / cm/360 metadata.
        // Does NOT enter camera yaw math (degrees_per_count uses sensitivity only).
        Self {
            dpi: 3200.0,
            sensitivity: 0.09,
            fov_degrees_h: 103.0,
            processor_id: "none".into(),
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
}

#[cfg(test)]
mod tests {
    use super::{ExperimentSettings, ValidationState};

    #[test]
    fn validation_starts_idle() {
        assert_eq!(ValidationState::default(), ValidationState::Idle);
    }

    #[test]
    fn processor_defaults_to_none() {
        assert_eq!(ExperimentSettings::default().processor_id, "none");
    }
}

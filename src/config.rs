use bevy::prelude::Resource;
use sense_telemetry::SessionBuffers;
use sense_types::{FovAxis, SensitivityConfig};

#[derive(Resource, Debug, Clone)]
pub struct ExperimentSettings {
    pub dpi: f64,
    pub sensitivity: f64,
    pub fov_degrees_h: f64,
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
        Self {
            dpi: 1600.0,
            sensitivity: 0.175,
            fov_degrees_h: 103.0,
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
    use super::ValidationState;

    #[test]
    fn validation_starts_idle() {
        assert_eq!(ValidationState::default(), ValidationState::Idle);
    }
}

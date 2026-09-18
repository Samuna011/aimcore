//! Shared types: MouseSample, InputCameraSample, Session, Configuration, IDs.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MouseSample {
    pub timestamp_ns: u64,
    pub dx: i32,
    pub dy: i32,
    pub buttons: u32,
    pub sequence_number: u64,
}

/// Processed mouse delta after InputProcessor transform; pairs 1:1 with raw MouseSample.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessedMouseSample {
    pub timestamp_ns: u64,
    pub sequence_number: u64,
    pub processed_dx: f64,
    pub processed_dy: f64,
    pub processor_id: String,
    pub processor_version: String,
    pub processor_config_json: String,
}

/// Camera state immediately after applying one raw mouse sample (not render-frame).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputCameraSample {
    pub timestamp_ns: u64,
    pub yaw_deg: f64,
    pub pitch_deg: f64,
    pub sequence_number: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameSample {
    pub timestamp_ns: u64,
    pub frame_time_s: f64,
    pub fps: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FovAxis {
    Horizontal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensitivityConfig {
    pub dpi: f64,
    pub sensitivity: f64,
    pub yaw_deg_per_count_at_sens_1: f64,
    pub fov_axis: FovAxis,
    pub fov_degrees: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccelerationConfig {
    pub enabled: bool,
    pub model: String,
    pub processor_id: String,
    pub processor_version: String,
    pub processor_config_json: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayConfig {
    pub width: u32,
    pub height: u32,
    pub refresh_hz: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigurationRecord {
    pub id: String,
    pub sensitivity: SensitivityConfig,
    pub edpi: f64,
    pub display: DisplayConfig,
    pub accel: AccelerationConfig,
    pub polling_rate_hz: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub app_version: String,
    pub experiment_id: String,
    pub experiment_version: String,
    pub config_id: String,
    pub seed: u64,
    pub start_unix_ms: i64,
    pub end_unix_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputIntegrityReport {
    pub samples_received: u64,
    pub sequence_gaps: u64,
    pub duplicate_sequences: u64,
    pub out_of_order_samples: u64,
    pub timestamp_regressions: u64,
}

impl InputIntegrityReport {
    pub fn is_pipeline_suspect(&self) -> bool {
        self.sequence_gaps > 0
            || self.duplicate_sequences > 0
            || self.out_of_order_samples > 0
            || self.timestamp_regressions > 0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AimTrialRecord {
    pub id: String,
    pub app_version: String,
    pub experiment_id: String,
    pub experiment_version: String,
    pub trial_type: String,
    pub status: String,
    pub processor_id: String,
    pub processor_version: String,
    pub processor_config_json: String,
    pub dpi: f64,
    pub sensitivity: f64,
    pub polling_rate_hz: f64,
    pub fov_degrees_h: f64,
    pub task_config_json: String,
    pub metrics_json: String,
    pub start_unix_ms: i64,
    pub end_unix_ms: i64,
    pub start_timestamp_ns: u64,
    pub end_timestamp_ns: u64,
    pub duration_secs: f64,
    pub hits: u32,
    pub shots: u32,
    pub misses: u32,
    pub score_secs: f64,
    pub accuracy: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AimShotRecord {
    pub shot_index: u32,
    pub timestamp_ns: u64,
    pub hit: bool,
    pub yaw_deg: f64,
    pub pitch_deg: f64,
    pub target_x: f64,
    pub target_y: f64,
    pub target_z: f64,
    pub target_radius: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationResult {
    pub expected_counts: f64,
    pub observed_net_counts: f64,
    pub observed_abs_path_counts: f64,
    pub expected_degrees: f64,
    pub observed_degrees: f64,
    pub count_difference: f64,
    pub error_percent: f64,
    pub integrity: InputIntegrityReport,
}

//! VALORANT-profile angular sensitivity math. No rounding. No Bevy.

pub const VALORANT_YAW_DEG_PER_COUNT_AT_SENS_1: f64 = 0.07;

pub fn degrees_per_count(sensitivity: f64) -> f64 {
    sensitivity * VALORANT_YAW_DEG_PER_COUNT_AT_SENS_1
}

pub fn yaw_delta_deg(dx_counts: f64, sensitivity: f64) -> f64 {
    dx_counts * sensitivity * VALORANT_YAW_DEG_PER_COUNT_AT_SENS_1
}

/// UnverifiedPitchModel: hipfire pitch uses same 0.07 as yaw (UNCERTAIN).
/// Positive `processed_dy` → look down when `+pitch_deg` means look up.
pub fn pitch_delta_deg(processed_dy: f64, sensitivity: f64) -> f64 {
    -(processed_dy * sensitivity * VALORANT_YAW_DEG_PER_COUNT_AT_SENS_1)
}

pub const PITCH_LIMIT_DEG: f64 = 89.0;

/// Clamps pitch to ±[`PITCH_LIMIT_DEG`] (UnverifiedPitchModel, UNCERTAIN).
pub fn clamp_pitch_deg(pitch_deg: f64) -> f64 {
    pitch_deg.clamp(-PITCH_LIMIT_DEG, PITCH_LIMIT_DEG)
}

/// Applies [`pitch_delta_deg`] then [`clamp_pitch_deg`] (UnverifiedPitchModel, UNCERTAIN).
pub fn apply_pitch_delta(pitch_deg: f64, processed_dy: f64, sensitivity: f64) -> f64 {
    clamp_pitch_deg(pitch_deg + pitch_delta_deg(processed_dy, sensitivity))
}

pub fn edpi(dpi: f64, sensitivity: f64) -> f64 {
    dpi * sensitivity
}

pub fn counts_per_360(sensitivity: f64) -> f64 {
    360.0 / degrees_per_count(sensitivity)
}

pub fn inches_per_360(dpi: f64, sensitivity: f64) -> f64 {
    counts_per_360(sensitivity) / dpi
}

pub fn cm_per_360(dpi: f64, sensitivity: f64) -> f64 {
    inches_per_360(dpi, sensitivity) * 2.54
}

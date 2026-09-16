//! VALORANT-profile angular sensitivity math. No rounding. No Bevy.

pub const VALORANT_YAW_DEG_PER_COUNT_AT_SENS_1: f64 = 0.07;

pub fn degrees_per_count(sensitivity: f64) -> f64 {
    sensitivity * VALORANT_YAW_DEG_PER_COUNT_AT_SENS_1
}

pub fn yaw_delta_deg(dx_counts: f64, sensitivity: f64) -> f64 {
    dx_counts * sensitivity * VALORANT_YAW_DEG_PER_COUNT_AT_SENS_1
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

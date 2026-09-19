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

/// Smallest `t >= 0` such that `origin + t * normalize(dir)` hits the sphere; `None` if miss.
pub fn ray_sphere_hit_t(
    origin: [f64; 3],
    dir: [f64; 3],
    center: [f64; 3],
    radius: f64,
) -> Option<f64> {
    if radius < 0.0 || !radius.is_finite() {
        return None;
    }
    let dlen = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    if dlen <= f64::EPSILON {
        return None;
    }
    let dx = dir[0] / dlen;
    let dy = dir[1] / dlen;
    let dz = dir[2] / dlen;
    let ox = origin[0] - center[0];
    let oy = origin[1] - center[1];
    let oz = origin[2] - center[2];
    // |o + t d|^2 = r^2  =>  t^2 + 2(o·d)t + (o·o - r^2) = 0
    let b = ox * dx + oy * dy + oz * dz;
    let c = ox * ox + oy * oy + oz * oz - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let sqrt_disc = disc.sqrt();
    let t0 = -b - sqrt_disc;
    let t1 = -b + sqrt_disc;
    if t0 >= 0.0 {
        Some(t0)
    } else if t1 >= 0.0 {
        Some(t1)
    } else {
        None
    }
}

/// Analytic ray–sphere test for STATIC_CLICK (origin + dir, any length dir).
/// Returns true if there is an intersection with parameter `t >= 0`.
pub fn ray_sphere_hit(
    origin: [f64; 3],
    dir: [f64; 3],
    center: [f64; 3],
    radius: f64,
) -> bool {
    ray_sphere_hit_t(origin, dir, center, radius).is_some()
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

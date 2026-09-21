//! Angular geometry helpers (degrees). Matches aim-lab look convention.

use crate::ANALYSIS_CAMERA_ORIGIN;

const EPS: f64 = 1e-12;

/// Shortest signed delta on a 360° circle into (−180, 180].
pub fn wrap_to_180(delta_deg: f64) -> f64 {
    let mut d = delta_deg % 360.0;
    if d > 180.0 {
        d -= 360.0;
    } else if d <= -180.0 {
        d += 360.0;
    }
    d
}

/// Accumulate continuous yaw from wrapped samples (shortest-step unwrap).
pub fn unwrap_yaw_series(wrapped_yaw_deg: &[f64]) -> Vec<f64> {
    if wrapped_yaw_deg.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(wrapped_yaw_deg.len());
    out.push(wrapped_yaw_deg[0]);
    for i in 1..wrapped_yaw_deg.len() {
        let step = wrap_to_180(wrapped_yaw_deg[i] - wrapped_yaw_deg[i - 1]);
        out.push(out[i - 1] + step);
    }
    out
}

pub fn look_dir(yaw_deg: f64, pitch_deg: f64) -> [f64; 3] {
    let yaw = yaw_deg.to_radians();
    let pitch = pitch_deg.to_radians();
    let cp = pitch.cos();
    [yaw.sin() * cp, pitch.sin(), -yaw.cos() * cp]
}

/// Inverse of [`look_dir`] (yaw in (−180, 180], pitch in degrees).
pub fn yaw_pitch_from_dir(dir: [f64; 3]) -> (f64, f64) {
    let d = normalize(dir);
    let pitch = d[1].clamp(-1.0, 1.0).asin().to_degrees();
    let yaw = d[0].atan2(-d[2]).to_degrees();
    (yaw, pitch)
}

pub fn target_dir(origin: [f64; 3], target: [f64; 3]) -> [f64; 3] {
    normalize([
        target[0] - origin[0],
        target[1] - origin[1],
        target[2] - origin[2],
    ])
}

pub fn camera_origin() -> [f64; 3] {
    ANALYSIS_CAMERA_ORIGIN
}

pub fn angular_distance_deg(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = dot(normalize(a), normalize(b)).clamp(-1.0, 1.0);
    d.acos().to_degrees()
}

/// Planar path length from consecutive unwrapped (yaw, pitch) samples.
pub fn path_length_yaw_pitch(yaw: &[f64], pitch: &[f64]) -> f64 {
    assert_eq!(yaw.len(), pitch.len());
    let mut sum = 0.0;
    for i in 1..yaw.len() {
        let dy = yaw[i] - yaw[i - 1];
        let dp = pitch[i] - pitch[i - 1];
        sum += (dy * dy + dp * dp).sqrt();
    }
    sum
}

/// Signed endpoint error in unwrapped yaw/pitch space along the approach axis.
///
/// Approach = vector from movement-start (yaw,pitch) to target (unwrapped nearest).
/// Positive = past the target along that axis.
pub fn endpoint_error_yaw_pitch(
    start_yaw: f64,
    start_pitch: f64,
    target_yaw_wrapped: f64,
    target_pitch: f64,
    click_yaw: f64,
    click_pitch: f64,
) -> f64 {
    // Unwrap target yaw onto the continuous branch nearest the movement start.
    let target_yaw = start_yaw + wrap_to_180(target_yaw_wrapped - wrap_to_180(start_yaw));
    let ax = target_yaw - start_yaw;
    let ay = target_pitch - start_pitch;
    let a_len = (ax * ax + ay * ay).sqrt();
    if a_len < EPS {
        let dx = click_yaw - target_yaw;
        let dy = click_pitch - target_pitch;
        return (dx * dx + dy * dy).sqrt();
    }
    let ux = ax / a_len;
    let uy = ay / a_len;
    let cx = click_yaw - start_yaw;
    let cy = click_pitch - start_pitch;
    let progress_click = cx * ux + cy * uy;
    progress_click - a_len
}

/// Max positive past-target excursion along approach before click (else 0).
pub fn overshoot_yaw_pitch(
    start_yaw: f64,
    start_pitch: f64,
    target_yaw_wrapped: f64,
    target_pitch: f64,
    path_yaw: &[f64],
    path_pitch: &[f64],
) -> f64 {
    let mut max_pos = 0.0_f64;
    for i in 0..path_yaw.len() {
        let err = endpoint_error_yaw_pitch(
            start_yaw,
            start_pitch,
            target_yaw_wrapped,
            target_pitch,
            path_yaw[i],
            path_pitch[i],
        );
        if err > max_pos {
            max_pos = err;
        }
    }
    max_pos
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn length(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn scale(v: [f64; 3], s: f64) -> [f64; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

pub fn normalize(v: [f64; 3]) -> [f64; 3] {
    let n = length(v);
    if n < EPS {
        return [0.0, 0.0, -1.0];
    }
    scale(v, 1.0 / n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_179_to_neg179_is_plus_two() {
        assert!((wrap_to_180(-179.0 - 179.0) - 2.0).abs() < 1e-9);
        assert!((wrap_to_180((-179.0) - 179.0) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn unwrap_crosses_seam_continuously() {
        let wrapped = vec![170.0, 175.0, -175.0, -170.0];
        let u = unwrap_yaw_series(&wrapped);
        assert!((u[0] - 170.0).abs() < 1e-9);
        assert!((u[1] - 175.0).abs() < 1e-9);
        assert!((u[2] - 185.0).abs() < 1e-9);
        assert!((u[3] - 190.0).abs() < 1e-9);
    }
}

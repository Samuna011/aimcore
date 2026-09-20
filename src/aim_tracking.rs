//! TRACKING v1: horizontal strafe motion and hold-to-score helpers (pure logic).

pub const TRACKING_DURATION_SECS: f64 = 30.0;
pub const TRACKING_RADIUS: f32 = 0.25;
pub const TRACKING_Y: f32 = 1.6;
pub const TRACKING_Z: f32 = -6.0;
pub const TRACKING_X_MIN: f32 = -1.5;
pub const TRACKING_X_MAX: f32 = 1.5;
pub const TRACKING_SPEED: f32 = 1.2;
pub const TRACKING_REVERSE_DELAY_MIN_S: f64 = 1.5;
pub const TRACKING_REVERSE_DELAY_MAX_S: f64 = 3.5;
pub const TRACKING_TASK_VERSION: &str = "1";

/// Same LCG as STATIC_CLICK / GRIDSHOT: Mulberry-style advance, unit in [0, 1).
fn next_unit(rng: &mut u64) -> f64 {
    *rng = rng
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1);
    ((*rng >> 33) as f64) / (u32::MAX as f64 + 1.0)
}

pub fn tracking_task_config_json() -> String {
    format!(
        r#"{{"duration_secs":{},"radius":{},"y":{},"z":{},"x_min":{},"x_max":{},"speed":{},"reverse_delay_min_secs":{},"reverse_delay_max_secs":{},"scoring_rule":"hold_and_ray","rng":"lcg","rng_version":"1"}}"#,
        TRACKING_DURATION_SECS,
        TRACKING_RADIUS,
        TRACKING_Y,
        TRACKING_Z,
        TRACKING_X_MIN,
        TRACKING_X_MAX,
        TRACKING_SPEED,
        TRACKING_REVERSE_DELAY_MIN_S,
        TRACKING_REVERSE_DELAY_MAX_S,
    )
}

/// Step horizontal motion by `dt_s`. Returns new x, vx, and whether a wall bounce occurred.
pub fn step_strafe(
    x: f32,
    vx: f32,
    dt_s: f64,
    x_min: f32,
    x_max: f32,
) -> (f32, f32, bool) {
    if dt_s <= 0.0 {
        return (x, vx, false);
    }
    let mut new_x = x + vx * dt_s as f32;
    let mut new_vx = vx;
    let mut bounced = false;
    if new_x > x_max {
        new_x = x_max;
        new_vx = reverse_vx(vx);
        bounced = true;
    } else if new_x < x_min {
        new_x = x_min;
        new_vx = reverse_vx(vx);
        bounced = true;
    }
    (new_x, new_vx, bounced)
}

/// Sample next reverse delay seconds from LCG rng in `[min_s, max_s)`.
pub fn next_reverse_delay_s(rng: &mut u64, min_s: f64, max_s: f64) -> f64 {
    debug_assert!(min_s <= max_s);
    min_s + next_unit(rng) * (max_s - min_s)
}

/// Flip vx (preserve speed magnitude).
pub fn reverse_vx(vx: f32) -> f32 {
    -vx
}

/// If held && ray hits sphere at center, return `dt_ns` to add; else 0.
pub fn on_target_dt_ns(
    lmb_held: bool,
    origin: [f64; 3],
    dir: [f64; 3],
    center: [f64; 3],
    radius: f64,
    dt_ns: u64,
) -> u64 {
    if !lmb_held {
        return 0;
    }
    if sense_math::ray_sphere_hit(origin, dir, center, radius) {
        dt_ns
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_strafe_moves_without_bounce() {
        let (x, vx, bounced) = step_strafe(0.0, 1.2, 0.5, TRACKING_X_MIN, TRACKING_X_MAX);
        assert!((x - 0.6).abs() < 1e-5);
        assert!((vx - 1.2).abs() < 1e-5);
        assert!(!bounced);
    }

    #[test]
    fn step_strafe_bounces_at_x_max() {
        let (x, vx, bounced) = step_strafe(1.4, 1.2, 0.2, TRACKING_X_MIN, TRACKING_X_MAX);
        assert!((x - TRACKING_X_MAX).abs() < 1e-5);
        assert!((vx - (-1.2)).abs() < 1e-5);
        assert!(bounced);
    }

    #[test]
    fn step_strafe_bounces_at_x_min() {
        let (x, vx, bounced) = step_strafe(-1.4, -1.2, 0.2, TRACKING_X_MIN, TRACKING_X_MAX);
        assert!((x - TRACKING_X_MIN).abs() < 1e-5);
        assert!((vx - 1.2).abs() < 1e-5);
        assert!(bounced);
    }

    #[test]
    fn step_strafe_zero_dt_unchanged() {
        let (x, vx, bounced) = step_strafe(0.5, -1.2, 0.0, TRACKING_X_MIN, TRACKING_X_MAX);
        assert!((x - 0.5).abs() < 1e-5);
        assert!((vx - (-1.2)).abs() < 1e-5);
        assert!(!bounced);
    }

    #[test]
    fn reverse_vx_flips_sign() {
        assert!((reverse_vx(1.2) - (-1.2)).abs() < 1e-5);
        assert!((reverse_vx(-1.2) - 1.2).abs() < 1e-5);
    }

    #[test]
    fn next_reverse_delay_in_range() {
        let mut rng = 42u64;
        for _ in 0..100 {
            let d = next_reverse_delay_s(
                &mut rng,
                TRACKING_REVERSE_DELAY_MIN_S,
                TRACKING_REVERSE_DELAY_MAX_S,
            );
            assert!(d >= TRACKING_REVERSE_DELAY_MIN_S);
            assert!(d < TRACKING_REVERSE_DELAY_MAX_S);
        }
    }

    #[test]
    fn next_reverse_delay_same_seed_same_sequence() {
        let mut a = 99u64;
        let mut b = 99u64;
        let seq_a: Vec<f64> = (0..5)
            .map(|_| {
                next_reverse_delay_s(
                    &mut a,
                    TRACKING_REVERSE_DELAY_MIN_S,
                    TRACKING_REVERSE_DELAY_MAX_S,
                )
            })
            .collect();
        let seq_b: Vec<f64> = (0..5)
            .map(|_| {
                next_reverse_delay_s(
                    &mut b,
                    TRACKING_REVERSE_DELAY_MIN_S,
                    TRACKING_REVERSE_DELAY_MAX_S,
                )
            })
            .collect();
        assert_eq!(seq_a, seq_b);
    }

    #[test]
    fn on_target_dt_ns_zero_when_not_held() {
        let origin = [0.0, 1.6, 4.0];
        let dir = [0.0, 0.0, -1.0];
        let center = [0.0, 1.6, -6.0];
        assert_eq!(
            on_target_dt_ns(false, origin, dir, center, TRACKING_RADIUS as f64, 16_666_666),
            0
        );
    }

    #[test]
    fn on_target_dt_ns_zero_when_held_but_miss() {
        let origin = [0.0, 1.6, 4.0];
        let dir = [1.0, 0.0, 0.0];
        let center = [0.0, 1.6, -6.0];
        assert_eq!(
            on_target_dt_ns(true, origin, dir, center, TRACKING_RADIUS as f64, 16_666_666),
            0
        );
    }

    #[test]
    fn on_target_dt_ns_returns_dt_when_held_and_hit() {
        let origin = [0.0, 1.6, 4.0];
        let dir = [0.0, 0.0, -1.0];
        let center = [0.0, 1.6, -6.0];
        let dt = 16_666_666u64;
        assert_eq!(
            on_target_dt_ns(true, origin, dir, center, TRACKING_RADIUS as f64, dt),
            dt
        );
    }

    #[test]
    fn task_config_json_has_required_knobs() {
        assert_eq!(TRACKING_TASK_VERSION, "1");
        let j = tracking_task_config_json();
        assert!(j.contains("\"duration_secs\":30"));
        assert!(j.contains(&format!("\"radius\":{}", TRACKING_RADIUS)));
        assert!(j.contains(&format!("\"y\":{}", TRACKING_Y)));
        assert!(j.contains(&format!("\"z\":{}", TRACKING_Z)));
        assert!(j.contains(&format!("\"x_min\":{}", TRACKING_X_MIN)));
        assert!(j.contains(&format!("\"x_max\":{}", TRACKING_X_MAX)));
        assert!(j.contains(&format!("\"speed\":{}", TRACKING_SPEED)));
        assert!(j.contains("\"reverse_delay_min_secs\":1.5"));
        assert!(j.contains("\"reverse_delay_max_secs\":3.5"));
        assert!(j.contains("\"scoring_rule\":\"hold_and_ray\""));
        assert!(j.contains("\"rng\":\"lcg\""));
        assert!(j.contains("\"rng_version\":\"1\""));
    }
}

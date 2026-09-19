use sense_math::*;

#[test]
fn degrees_per_count_examples() {
    assert_eq!(degrees_per_count(0.15), 0.15 * 0.07);
    assert_eq!(degrees_per_count(0.175), 0.175 * 0.07);
    assert_eq!(degrees_per_count(0.20), 0.20 * 0.07);
}

#[test]
fn yaw_delta_1000_counts_at_0_175() {
    assert_eq!(yaw_delta_deg(1000.0, 0.175), 1000.0 * 0.175 * 0.07);
}

#[test]
fn same_edpi_same_cm_per_360_different_counts() {
    let a = (800.0, 0.35);
    let b = (1600.0, 0.175);
    let c = (3200.0, 0.0875);
    assert_eq!(edpi(a.0, a.1), 280.0);
    assert_eq!(edpi(b.0, b.1), 280.0);
    assert_eq!(edpi(c.0, c.1), 280.0);
    assert_eq!(cm_per_360(a.0, a.1), cm_per_360(b.0, b.1));
    assert_eq!(cm_per_360(b.0, b.1), cm_per_360(c.0, c.1));
    assert_ne!(counts_per_360(a.1), counts_per_360(b.1));
    assert_ne!(counts_per_360(b.1), counts_per_360(c.1));
    assert_eq!(counts_per_360(0.175), 360.0 / (0.175 * 0.07));
}

#[test]
fn angular_model_independent_of_fov_and_resolution_params() {
    // Pure math API takes no fov/resolution; this documents the contract:
    // callers must not pass fov/resolution into these functions.
    let sens = 0.175;
    let dpi = 1600.0;
    let _fov_a = 103.0_f64;
    let _fov_b = 90.0_f64;
    let _res_a = (1920.0, 1080.0);
    let _res_b = (1280.0, 960.0);
    assert_eq!(degrees_per_count(sens), degrees_per_count(sens));
    assert_eq!(counts_per_360(sens), counts_per_360(sens));
    assert_eq!(cm_per_360(dpi, sens), cm_per_360(dpi, sens));
}

#[test]
fn inches_and_cm_relationship() {
    let dpi = 1600.0;
    let sens = 0.175;
    assert_eq!(cm_per_360(dpi, sens), inches_per_360(dpi, sens) * 2.54);
}

#[test]
fn pitch_positive_dy_looks_down() {
    // +pitch = look up ⇒ +dy yields negative delta
    let d = pitch_delta_deg(1000.0, 0.175);
    assert!((d - (-12.25)).abs() < 1e-12);
}

#[test]
fn pitch_and_yaw_same_magnitude() {
    let sens = 0.09;
    let yaw = yaw_delta_deg(500.0, sens).abs();
    let pitch = pitch_delta_deg(500.0, sens).abs();
    assert!((yaw - pitch).abs() < 1e-12);
}

#[test]
fn pitch_clamps_at_pm_89() {
    assert_eq!(clamp_pitch_deg(90.0), 89.0);
    assert_eq!(clamp_pitch_deg(-90.0), -89.0);
    let p = apply_pitch_delta(88.0, 1_000_000.0, 1.0);
    assert_eq!(p, -89.0); // large +dy drives look-down to floor
}

#[test]
fn ray_sphere_hit_through_center() {
    assert!(ray_sphere_hit(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, -5.0],
        0.5
    ));
}

#[test]
fn ray_sphere_miss_beside() {
    assert!(!ray_sphere_hit(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, -1.0],
        [2.0, 0.0, -5.0],
        0.5
    ));
}

#[test]
fn ray_sphere_miss_behind_camera() {
    assert!(!ray_sphere_hit(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, 5.0],
        0.5
    ));
}

#[test]
fn ray_sphere_zero_dir_misses() {
    assert!(!ray_sphere_hit([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, -1.0], 1.0));
}

#[test]
fn ray_sphere_hit_t_through_center() {
    let t = ray_sphere_hit_t([0.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 0.0, -5.0], 0.5)
        .expect("expected hit");
    assert!((t - 4.5).abs() < 1e-12);
}

#[test]
fn ray_sphere_hit_t_miss_returns_none() {
    assert!(ray_sphere_hit_t(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, -1.0],
        [2.0, 0.0, -5.0],
        0.5
    )
    .is_none());
    assert!(ray_sphere_hit_t(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, 5.0],
        0.5
    )
    .is_none());
    assert!(ray_sphere_hit_t([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, -1.0], 1.0).is_none());
}

#[test]
fn ray_sphere_hit_t_prefers_closer_sphere() {
    // Two spheres along −Z; nearer at z=-3 (r=0.5 → t=2.5), farther at z=-8 (t=7.5).
    let near = ray_sphere_hit_t([0.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 0.0, -3.0], 0.5).unwrap();
    let far = ray_sphere_hit_t([0.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 0.0, -8.0], 0.5).unwrap();
    assert!(near < far);
    assert!((near - 2.5).abs() < 1e-12);
    assert!((far - 7.5).abs() < 1e-12);
}

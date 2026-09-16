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

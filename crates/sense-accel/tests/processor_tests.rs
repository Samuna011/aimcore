use sense_accel::*;

#[test]
fn no_acceleration_is_identity() {
    let mut p = NoAcceleration;
    assert_eq!(p.id(), "none");
    assert_eq!(p.version(), "1.0.0");
    assert_eq!(p.config_json(), "{}");
    assert_eq!(p.process(3.0, -2.0, 0.004), (3.0, -2.0));
}

#[test]
fn factory_accepts_none_rejects_unknown() {
    assert!(create_processor("none").is_ok());
    assert!(create_processor("raw_accel").is_err());
}

#[test]
fn dt_s_from_consecutive_raw_timestamps() {
    assert_eq!(dt_s_from_timestamps(None, 1_000_000_000), 0.0);
    assert_eq!(dt_s_from_timestamps(Some(1_000_000_000), 1_004_000_000), 0.004);
    // Must not use render-frame semantics — only timestamp math.
}

#[test]
fn guide_vector_30_40() {
    let e = eval_rawaccel_linear(30.0, 40.0, 0.001, 0.01, 0.5);
    assert!(!e.bypassed_nonpositive_dt);
    assert!((e.dt_ms - 1.0).abs() < 1e-12);
    assert!((e.input_speed - 50.0).abs() < 1e-9);
    assert!((e.acceleration_scale - 1.5).abs() < 1e-12);
    assert!((e.processed_dx - 22.5).abs() < 1e-9);
    assert!((e.processed_dy - 30.0).abs() < 1e-9);
}

#[test]
fn zero_vector_stable() {
    let e = eval_rawaccel_linear(0.0, 0.0, 0.001, 0.01, 0.5);
    assert_eq!((e.processed_dx, e.processed_dy), (0.0, 0.0));
}

#[test]
fn nonpositive_dt_trainer_bypass() {
    let e = eval_rawaccel_linear(30.0, 40.0, 0.0, 0.01, 0.5);
    assert!(e.bypassed_nonpositive_dt);
    assert_eq!(e.input_speed, 0.0);
    assert_eq!(e.acceleration_scale, 1.0);
    assert_eq!((e.processed_dx, e.processed_dy), (30.0, 40.0));
}

#[test]
fn axis_aligned_whole_not_by_component() {
    let e = eval_rawaccel_linear(50.0, 0.0, 0.001, 0.01, 0.5);
    assert!((e.processed_dx - 37.5).abs() < 1e-9);
    assert_eq!(e.processed_dy, 0.0);
}

#[test]
fn zero_acceleration_keeps_multiplier() {
    let e = eval_rawaccel_linear(30.0, 40.0, 0.001, 0.0, 0.5);
    assert!((e.acceleration_scale - 1.0).abs() < 1e-12);
    assert!((e.processed_dx - 15.0).abs() < 1e-9);
    assert!((e.processed_dy - 20.0).abs() < 1e-9);
}

#[test]
fn negative_vector_preserves_direction() {
    let e = eval_rawaccel_linear(-30.0, -40.0, 0.001, 0.01, 0.5);
    assert!((e.processed_dx - (-22.5)).abs() < 1e-9);
    assert!((e.processed_dy - (-30.0)).abs() < 1e-9);
}

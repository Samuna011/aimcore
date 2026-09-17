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
    let config = RawAccelLinearConfig::trainer_default();
    assert!(create_processor("none", &config).is_ok());
    assert!(create_processor("raw_accel", &config).is_err());
}

#[test]
fn factory_rawaccel_linear_guide_process() {
    let config = RawAccelLinearConfig::phase1_sensitivity(0.01, 0.5);
    let mut p = create_processor("rawaccel_linear", &config).unwrap();
    assert_eq!(p.id(), "rawaccel_linear");
    assert_eq!(p.version(), "1.2.0");
    let (ox, oy) = p.process(30.0, 40.0, 0.001);
    assert!((ox - 22.5).abs() < 1e-9);
    assert!((oy - 30.0).abs() < 1e-9);
}

#[test]
fn factory_none_ignores_accel_params() {
    let config = RawAccelLinearConfig::phase1_sensitivity(0.01, 0.5);
    let mut p = create_processor("none", &config).unwrap();
    assert_eq!(p.process(3.0, -2.0, 0.001), (3.0, -2.0));
}

#[test]
fn factory_still_rejects_unknown() {
    let config = RawAccelLinearConfig::trainer_default();
    assert!(create_processor("raw_accel", &config).is_err());
}

#[test]
fn factory_rawaccel_linear_v110_gain_default_process_finite() {
    let mut p =
        create_processor("rawaccel_linear", &RawAccelLinearConfig::trainer_default()).unwrap();
    assert_eq!(p.version(), "1.2.0");
    let (ox, oy) = p.process(30.0, 40.0, 0.001);
    assert!(ox.is_finite() && oy.is_finite());
}

#[test]
fn validate_accepts_in_with_zero_cap_x() {
    let config = RawAccelLinearConfig {
        cap_mode: CapMode::In,
        ..RawAccelLinearConfig::trainer_default()
    };
    assert!(config.validate().is_ok());
}

#[test]
fn factory_accepts_in_with_zero_cap_x() {
    let config = RawAccelLinearConfig {
        cap_mode: CapMode::In,
        ..RawAccelLinearConfig::trainer_default()
    };
    let mut p = create_processor("rawaccel_linear", &config).unwrap();
    let (ox, oy) = p.process(30.0, 40.0, 0.001);
    assert!(ox.is_finite() && oy.is_finite());
}

#[test]
fn factory_rejects_io_with_zero_cap_x() {
    let config = RawAccelLinearConfig {
        cap_mode: CapMode::Io,
        cap_x: 0.0,
        ..RawAccelLinearConfig::trainer_default()
    };

    let error = create_processor("rawaccel_linear", &config)
        .err()
        .expect("io cap_x=0 must be rejected");
    assert!(error.contains("cap_x must be greater than input_offset"));
}

#[test]
fn factory_accepts_valid_io_and_processes_finite() {
    let config = RawAccelLinearConfig {
        cap_mode: CapMode::Io,
        cap_x: 40.0,
        cap_y: 2.0,
        ..RawAccelLinearConfig::trainer_default()
    };

    let mut p = create_processor("rawaccel_linear", &config).unwrap();
    let (ox, oy) = p.process(30.0, 40.0, 0.001);
    assert!(ox.is_finite() && oy.is_finite());
}

#[test]
fn rawaccel_linear_config_json_includes_all_fields() {
    let p = RawAccelLinear::new(RawAccelLinearConfig::trainer_default());
    assert_eq!(
        p.config_json(),
        "{\"acceleration\":0.007,\"sensitivity_multiplier\":1,\"gain\":true,\"input_offset\":0,\"cap_mode\":\"out\",\"cap_x\":2,\"cap_y\":2,\"polling_rate_hz\":1000}"
    );
}

#[test]
fn dt_s_from_consecutive_raw_timestamps() {
    assert_eq!(dt_s_from_timestamps(None, 1_000_000_000), 0.0);
    assert_eq!(
        dt_s_from_timestamps(Some(1_000_000_000), 1_004_000_000),
        0.004
    );
    // Must not use render-frame semantics — only timestamp math.
}

#[test]
fn guide_vector_30_40() {
    let config = RawAccelLinearConfig::phase1_sensitivity(0.01, 0.5);
    let e = eval_rawaccel_linear_config(30.0, 40.0, 0.001, &config);
    assert!(!e.bypassed_nonpositive_dt);
    assert!(!e.time_clamped);
    assert!((e.raw_dt_ms - 1.0).abs() < 1e-12);
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
    let config = RawAccelLinearConfig::trainer_default();
    let e = eval_rawaccel_linear_config(30.0, 40.0, 0.0, &config);
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

#[test]
fn poll_period_floor_clamps_tiny_dt_and_cools_scale() {
    let config = RawAccelLinearConfig {
        polling_rate_hz: 1000,
        ..RawAccelLinearConfig::phase1_sensitivity(0.01, 1.0)
    };
    // 0.1 ms raw would be 10× hotter unclamped; floor to 1 ms → Guide-scale v=50.
    let e = eval_rawaccel_linear_config(50.0, 0.0, 0.0001, &config);
    assert!(e.time_clamped);
    assert!((e.raw_dt_ms - 0.1).abs() < 1e-12);
    assert!((e.dt_ms - 1.0).abs() < 1e-12);
    assert!((e.input_speed - 50.0).abs() < 1e-9);
    assert!((e.acceleration_scale - 1.5).abs() < 1e-12);
    assert!((e.processed_dx - 75.0).abs() < 1e-9);
}

#[test]
fn clamp_speed_dt_ms_matches_ra_defaults_when_poll_zero() {
    assert!((clamp_speed_dt_ms(0.01, 0) - RA_DEFAULT_TIME_MIN_MS).abs() < 1e-15);
    assert!((clamp_speed_dt_ms(200.0, 0) - RA_DEFAULT_TIME_MAX_MS).abs() < 1e-15);
    assert!((clamp_speed_dt_ms(1.0, 0) - 1.0).abs() < 1e-15);
}

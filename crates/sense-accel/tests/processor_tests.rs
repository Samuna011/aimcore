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

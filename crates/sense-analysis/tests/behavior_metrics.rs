use sense_analysis::geometry::{endpoint_error_yaw_pitch, overshoot_yaw_pitch};

#[test]
fn endpoint_error_positive_when_past_target_along_yaw() {
    let err_short = endpoint_error_yaw_pitch(0.0, 0.0, 10.0, 0.0, 5.0, 0.0);
    let err_past = endpoint_error_yaw_pitch(0.0, 0.0, 10.0, 0.0, 15.0, 0.0);
    assert!(err_short < 0.0, "undershoot should be negative, got {err_short}");
    assert!(err_past > 0.0, "overshoot past target should be positive, got {err_past}");
}

#[test]
fn overshoot_zero_when_never_past() {
    let path_yaw = [0.0, 2.0, 4.0, 6.0, 8.0, 9.0];
    let path_pitch = [0.0; 6];
    let o = overshoot_yaw_pitch(0.0, 0.0, 10.0, 0.0, &path_yaw, &path_pitch);
    assert!(o <= 1e-6, "never past → overshoot 0, got {o}");
}

use sense_accel::{CapMode, ClassicLinearArgs, ClassicLinearState};

fn approx(a: f64, b: f64, eps: f64) {
    assert!((a - b).abs() < eps, "{a} != {b}");
}

#[test]
fn legacy_uncapped_matches_phase1_linear() {
    let s = ClassicLinearState::new(ClassicLinearArgs {
        acceleration: 0.01,
        input_offset: 0.0,
        gain: false,
        cap_mode: CapMode::Out,
        cap_x: 0.0,
        cap_y: 0.0,
    });
    approx(s.scale(50.0), 1.5, 1e-12);
}

#[test]
fn legacy_output_cap_2_clamps_addend() {
    let s = ClassicLinearState::new(ClassicLinearArgs {
        acceleration: 0.01,
        input_offset: 0.0,
        gain: false,
        cap_mode: CapMode::Out,
        cap_x: 0.0,
        cap_y: 2.0,
    });
    approx(s.scale(50.0), 1.5, 1e-12);
    approx(s.scale(200.0), 2.0, 1e-12);
}

#[test]
fn gain_output_cap_2_knee() {
    let accel = 0.007;
    let s = ClassicLinearState::new(ClassicLinearArgs {
        acceleration: accel,
        input_offset: 0.0,
        gain: true,
        cap_mode: CapMode::Out,
        cap_x: 0.0,
        cap_y: 2.0,
    });
    let cap_x = 0.5 / accel;
    approx(cap_x, 71.42857142857143, 1e-9);
    approx(s.scale(50.0), 1.0 + accel * 50.0, 1e-12);

    let constant = (accel * cap_x - 1.0) * cap_x;
    let x = 100.0;
    let expected = constant / x + 1.0 + 1.0;
    approx(s.scale(x), expected, 1e-9);
}

#[test]
fn offset_returns_one_at_or_below_offset() {
    let s = ClassicLinearState::new(ClassicLinearArgs {
        acceleration: 0.01,
        input_offset: 10.0,
        gain: false,
        cap_mode: CapMode::Out,
        cap_x: 0.0,
        cap_y: 0.0,
    });
    approx(s.scale(10.0), 1.0, 1e-12);
    approx(s.scale(5.0), 1.0, 1e-12);
}

#[test]
fn gain_cap_in_and_io_construct_and_evaluate() {
    let in_state = ClassicLinearState::new(ClassicLinearArgs {
        acceleration: 0.01,
        input_offset: 0.0,
        gain: true,
        cap_mode: CapMode::In,
        cap_x: 40.0,
        cap_y: 0.0,
    });
    assert!(in_state.scale(20.0).is_finite());
    assert!(in_state.scale(80.0).is_finite());

    let io_state = ClassicLinearState::new(ClassicLinearArgs {
        acceleration: 0.01,
        input_offset: 0.0,
        gain: true,
        cap_mode: CapMode::Io,
        cap_x: 40.0,
        cap_y: 2.0,
    });
    assert!(io_state.scale(20.0).is_finite());
    assert!(io_state.scale(80.0).is_finite());
}

#[test]
fn legacy_cap_in_and_io_construct_and_evaluate() {
    let in_state = ClassicLinearState::new(ClassicLinearArgs {
        acceleration: 0.01,
        input_offset: 0.0,
        gain: false,
        cap_mode: CapMode::In,
        cap_x: 40.0,
        cap_y: 0.0,
    });
    assert!(in_state.scale(20.0).is_finite());
    assert!(in_state.scale(80.0).is_finite());

    let io_state = ClassicLinearState::new(ClassicLinearArgs {
        acceleration: 0.01,
        input_offset: 0.0,
        gain: false,
        cap_mode: CapMode::Io,
        cap_x: 40.0,
        cap_y: 2.0,
    });
    assert!(io_state.scale(20.0).is_finite());
    assert!(io_state.scale(80.0).is_finite());
}

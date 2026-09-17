//! Raw Accel Classic acceleration with exponent fixed at 2.0.
//!
//! Ported from RawAccelOfficial/rawaccel `common/accel-classic.hpp`.

const EXPONENT: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapMode {
    Out,
    In,
    Io,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassicLinearArgs {
    pub acceleration: f64,
    pub input_offset: f64,
    pub gain: bool,
    pub cap_mode: CapMode,
    pub cap_x: f64,
    pub cap_y: f64,
}

/// Precomputed state mirroring the fields initialized by the C++ classic
/// Legacy and Gain constructors.
#[derive(Debug, Clone)]
pub struct ClassicLinearState {
    accel_raised: f64,
    legacy_cap: f64,
    gain_cap_x: f64,
    gain_cap_y: f64,
    constant: f64,
    sign: f64,
    input_offset: f64,
    gain_mode: bool,
}

impl ClassicLinearState {
    pub fn new(args: ClassicLinearArgs) -> Self {
        if args.gain {
            Self::new_gain(args)
        } else {
            Self::new_legacy(args)
        }
    }

    /// Official sensitivity scale for input speed `x` (counts/ms).
    ///
    /// This does not apply the sensitivity multiplier.
    pub fn scale(&self, x: f64) -> f64 {
        if x <= self.input_offset {
            return 1.0;
        }

        if self.gain_mode {
            let output = if x < self.gain_cap_x {
                base_fn(x, self.accel_raised, self.input_offset)
            } else {
                self.constant / x + self.gain_cap_y
            };
            self.sign * output + 1.0
        } else {
            self.sign * base_fn(x, self.accel_raised, self.input_offset).min(self.legacy_cap) + 1.0
        }
    }

    fn new_legacy(args: ClassicLinearArgs) -> Self {
        let mut state = Self {
            accel_raised: 0.0,
            legacy_cap: f64::MAX,
            gain_cap_x: f64::MAX,
            gain_cap_y: f64::MAX,
            constant: 0.0,
            sign: 1.0,
            input_offset: args.input_offset,
            gain_mode: false,
        };

        match args.cap_mode {
            CapMode::Io => {
                state.legacy_cap = args.cap_y - 1.0;

                if state.legacy_cap < 0.0 {
                    state.legacy_cap = -state.legacy_cap;
                    state.sign = -state.sign;
                }

                let acceleration = base_accel(args.cap_x, state.legacy_cap, args.input_offset);
                state.accel_raised = acceleration.powf(EXPONENT - 1.0);
            }
            CapMode::In => {
                state.accel_raised = args.acceleration.powf(EXPONENT - 1.0);
                if args.cap_x > 0.0 {
                    state.legacy_cap = base_fn(args.cap_x, state.accel_raised, args.input_offset);
                }
            }
            CapMode::Out => {
                state.accel_raised = args.acceleration.powf(EXPONENT - 1.0);

                if args.cap_y > 0.0 {
                    state.legacy_cap = args.cap_y - 1.0;

                    if state.legacy_cap < 0.0 {
                        state.legacy_cap = -state.legacy_cap;
                        state.sign = -state.sign;
                    }
                }
            }
        }

        state
    }

    fn new_gain(args: ClassicLinearArgs) -> Self {
        let mut state = Self {
            accel_raised: 0.0,
            legacy_cap: f64::MAX,
            gain_cap_x: f64::MAX,
            gain_cap_y: f64::MAX,
            constant: 0.0,
            sign: 1.0,
            input_offset: args.input_offset,
            gain_mode: true,
        };

        match args.cap_mode {
            CapMode::Io => {
                state.gain_cap_x = args.cap_x;
                state.gain_cap_y = args.cap_y - 1.0;

                if state.gain_cap_y < 0.0 {
                    state.gain_cap_y = -state.gain_cap_y;
                    state.sign = -state.sign;
                }

                let acceleration =
                    gain_accel(state.gain_cap_x, state.gain_cap_y, args.input_offset);
                state.accel_raised = acceleration.powf(EXPONENT - 1.0);
                state.constant = (base_fn(state.gain_cap_x, state.accel_raised, args.input_offset)
                    - state.gain_cap_y)
                    * state.gain_cap_x;
            }
            CapMode::In => {
                state.accel_raised = args.acceleration.powf(EXPONENT - 1.0);
                if args.cap_x > 0.0 {
                    state.gain_cap_x = args.cap_x;
                    state.gain_cap_y = gain(state.gain_cap_x, args.acceleration, args.input_offset);
                    state.constant =
                        (base_fn(state.gain_cap_x, state.accel_raised, args.input_offset)
                            - state.gain_cap_y)
                            * state.gain_cap_x;
                }
            }
            CapMode::Out => {
                state.accel_raised = args.acceleration.powf(EXPONENT - 1.0);

                if args.cap_y > 0.0 {
                    state.gain_cap_y = args.cap_y - 1.0;

                    if state.gain_cap_y == 0.0 {
                        state.gain_cap_x = 0.0;
                    } else {
                        if state.gain_cap_y < 0.0 {
                            state.gain_cap_y = -state.gain_cap_y;
                            state.sign = -state.sign;
                        }

                        state.gain_cap_x =
                            gain_inverse(state.gain_cap_y, args.acceleration, args.input_offset);
                        state.constant =
                            (base_fn(state.gain_cap_x, state.accel_raised, args.input_offset)
                                - state.gain_cap_y)
                                * state.gain_cap_x;
                    }
                }
            }
        }

        state
    }
}

fn base_fn(x: f64, accel_raised: f64, input_offset: f64) -> f64 {
    accel_raised * (x - input_offset).powf(EXPONENT) / x
}

fn base_accel(x: f64, y: f64, input_offset: f64) -> f64 {
    (x * y * (x - input_offset).powf(-EXPONENT)).powf(1.0 / (EXPONENT - 1.0))
}

fn gain(x: f64, acceleration: f64, input_offset: f64) -> f64 {
    EXPONENT * (acceleration * (x - input_offset)).powf(EXPONENT - 1.0)
}

fn gain_inverse(y: f64, acceleration: f64, input_offset: f64) -> f64 {
    (acceleration * input_offset + (y / EXPONENT).powf(1.0 / (EXPONENT - 1.0))) / acceleration
}

fn gain_accel(x: f64, y: f64, input_offset: f64) -> f64 {
    -(y / EXPONENT).powf(1.0 / (EXPONENT - 1.0)) / (input_offset - x)
}

//! Input processor trait, factory, and QPC-derived inter-sample timing.

pub trait InputProcessor: Send {
    fn id(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn config_json(&self) -> String;
    fn process(&mut self, dx: f64, dy: f64, dt_s: f64) -> (f64, f64);
}

pub struct NoAcceleration;

impl InputProcessor for NoAcceleration {
    fn id(&self) -> &'static str {
        "none"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn config_json(&self) -> String {
        "{}".into()
    }

    fn process(&mut self, dx: f64, dy: f64, _dt_s: f64) -> (f64, f64) {
        (dx, dy)
    }
}

pub fn create_processor(id: &str) -> Result<Box<dyn InputProcessor>, String> {
    match id {
        "none" => Ok(Box::new(NoAcceleration)),
        _ => Err(format!("unknown processor id: {id}")),
    }
}

/// Inter-sample interval from consecutive raw QPC timestamps (nanoseconds).
///
/// Uses only monotonic raw mouse-event timestamps — never render-frame delta,
/// FPS, or VSync timing. First sample in a burst (no previous timestamp): `0.0`.
pub fn dt_s_from_timestamps(prev_ns: Option<u64>, current_ns: u64) -> f64 {
    match prev_ns {
        None => 0.0,
        Some(prev) => (current_ns.saturating_sub(prev)) as f64 / 1_000_000_000.0,
    }
}

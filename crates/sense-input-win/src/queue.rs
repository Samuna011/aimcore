use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use sense_types::MouseSample;

pub struct MouseQueue {
    samples: Mutex<VecDeque<MouseSample>>,
    next_sequence: AtomicU64,
}

impl MouseQueue {
    pub fn new() -> Self {
        Self {
            samples: Mutex::new(VecDeque::new()),
            next_sequence: AtomicU64::new(1),
        }
    }

    pub fn push_raw(&self, dx: i32, dy: i32, buttons: u32, timestamp_ns: u64) {
        self.push_sample(dx, dy, buttons, timestamp_ns);
    }

    pub(crate) fn push_sample(
        &self,
        dx: i32,
        dy: i32,
        buttons: u32,
        timestamp_ns: u64,
    ) -> MouseSample {
        let mut samples = self
            .samples
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let sequence_number = self.next_sequence.fetch_add(1, Ordering::Relaxed);
        let sample = MouseSample {
            timestamp_ns,
            dx,
            dy,
            buttons,
            sequence_number,
        };
        samples.push_back(sample.clone());
        sample
    }

    pub fn drain_all(&self) -> Vec<MouseSample> {
        let mut samples = self
            .samples
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        samples.drain(..).collect()
    }
}

impl Default for MouseQueue {
    fn default() -> Self {
        Self::new()
    }
}

//! Windows `WM_INPUT` capture and timestamped mouse queue.
//!
//! The experimental input stream must come from raw `WM_INPUT` samples.
//! Bevy mouse-motion events and cursor-position deltas are not substitutes:
//! they can obscure the one-event-per-sample ordering this crate preserves.

mod clock;
mod integrity;
mod queue;
mod wm_input;

pub use clock::monotonic_now_ns;
pub use integrity::IntegrityTracker;
pub use queue::MouseQueue;
pub use wm_input::{
    handle_wm_input, raw_input_read_failures, register_raw_mouse, registered_mouse_queue,
};

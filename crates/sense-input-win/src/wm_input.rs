use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::{size_of, MaybeUninit};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, MOUSE_MOVE_ABSOLUTE, MOUSE_STATE,
    RAWINPUT, RAWINPUTDEVICE, RAWINPUTDEVICE_FLAGS, RAWINPUTHEADER, RID_INPUT, RIM_TYPEMOUSE,
};

use crate::{monotonic_now_ns, IntegrityTracker, MouseQueue};

const HID_USAGE_PAGE_GENERIC: u16 = 0x01;
const HID_USAGE_GENERIC_MOUSE: u16 = 0x02;
static REGISTERED_QUEUES: OnceLock<Mutex<HashMap<isize, Arc<MouseQueue>>>> = OnceLock::new();
static RAW_INPUT_READ_FAILURES: AtomicU64 = AtomicU64::new(0);

pub fn register_raw_mouse(hwnd: isize, queue: Arc<MouseQueue>) -> Result<(), String> {
    let device = RAWINPUTDEVICE {
        usUsagePage: HID_USAGE_PAGE_GENERIC,
        usUsage: HID_USAGE_GENERIC_MOUSE,
        dwFlags: RAWINPUTDEVICE_FLAGS::default(),
        hwndTarget: HWND(hwnd as *mut c_void),
    };

    unsafe { RegisterRawInputDevices(&[device], size_of::<RAWINPUTDEVICE>() as u32) }
        .map_err(|error| format!("RegisterRawInputDevices failed: {error}"))?;

    // Retain the queue for the HWND so registration never silently discards it.
    // WM_INPUT callers should pass this same Arc's MouseQueue to handle_wm_input.
    store_registered_queue(hwnd, queue);
    Ok(())
}

/// Returns the queue retained for a successfully registered window.
pub fn registered_mouse_queue(hwnd: isize) -> Option<Arc<MouseQueue>> {
    registered_queues()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(&hwnd)
        .cloned()
}

/// Returns the number of malformed or failed `GetRawInputData` reads.
pub fn raw_input_read_failures() -> u64 {
    RAW_INPUT_READ_FAILURES.load(Ordering::Relaxed)
}

/// Processes relative mouse packets only; absolute packets are intentionally skipped.
pub fn handle_wm_input(lparam: isize, queue: &MouseQueue, tracker: &Mutex<IntegrityTracker>) {
    let mut raw = MaybeUninit::<RAWINPUT>::zeroed();
    let mut size = size_of::<RAWINPUT>() as u32;
    let copied = unsafe {
        GetRawInputData(
            HRAWINPUT(lparam as *mut c_void),
            RID_INPUT,
            Some(raw.as_mut_ptr().cast()),
            &mut size,
            size_of::<RAWINPUTHEADER>() as u32,
        )
    };
    if copied == u32::MAX || copied < size_of::<RAWINPUT>() as u32 {
        record_raw_input_read_failure(copied, size);
        return;
    }

    let raw = unsafe { raw.assume_init() };
    if raw.header.dwType != RIM_TYPEMOUSE.0 {
        return;
    }

    let mouse = unsafe { raw.data.mouse };
    let Some((dx, dy)) = relative_delta(mouse.usFlags, mouse.lLastX, mouse.lLastY) else {
        return;
    };
    let buttons = unsafe { mouse.Anonymous.ulButtons };
    let sample = queue.push_sample(dx, dy, buttons, monotonic_now_ns());
    tracker
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .observe(&sample);
}

fn registered_queues() -> &'static Mutex<HashMap<isize, Arc<MouseQueue>>> {
    REGISTERED_QUEUES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn store_registered_queue(hwnd: isize, queue: Arc<MouseQueue>) {
    registered_queues()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(hwnd, queue);
}

fn relative_delta(flags: MOUSE_STATE, dx: i32, dy: i32) -> Option<(i32, i32)> {
    (flags.0 & MOUSE_MOVE_ABSOLUTE.0 == 0).then_some((dx, dy))
}

fn record_raw_input_read_failure(copied: u32, expected: u32) {
    let previous = RAW_INPUT_READ_FAILURES.fetch_add(1, Ordering::Relaxed);
    if previous == 0 {
        eprintln!(
            "sense-input-win: GetRawInputData failed or returned too few bytes \
             (copied={copied}, expected_at_least={expected})"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_mouse_packets_do_not_produce_relative_deltas() {
        assert_eq!(relative_delta(MOUSE_MOVE_ABSOLUTE, 32_767, 65_535), None);
    }

    #[test]
    fn relative_mouse_packets_preserve_deltas() {
        assert_eq!(
            relative_delta(MOUSE_STATE::default(), -12, 34),
            Some((-12, 34))
        );
    }

    #[test]
    fn registration_storage_retains_shared_queue() {
        let hwnd = -12_345;
        let queue = Arc::new(MouseQueue::new());

        store_registered_queue(hwnd, Arc::clone(&queue));

        let stored = registered_mouse_queue(hwnd).expect("registered queue");
        assert!(Arc::ptr_eq(&queue, &stored));
    }
}

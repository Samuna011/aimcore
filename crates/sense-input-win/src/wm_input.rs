use std::ffi::c_void;
use std::mem::{size_of, MaybeUninit};
use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE,
    RAWINPUTDEVICE_FLAGS, RAWINPUTHEADER, RID_INPUT, RIM_TYPEMOUSE,
};

use crate::{monotonic_now_ns, IntegrityTracker, MouseQueue};

const HID_USAGE_PAGE_GENERIC: u16 = 0x01;
const HID_USAGE_GENERIC_MOUSE: u16 = 0x02;

pub fn register_raw_mouse(hwnd: isize, queue: Arc<MouseQueue>) -> Result<(), String> {
    let device = RAWINPUTDEVICE {
        usUsagePage: HID_USAGE_PAGE_GENERIC,
        usUsage: HID_USAGE_GENERIC_MOUSE,
        dwFlags: RAWINPUTDEVICE_FLAGS::default(),
        hwndTarget: HWND(hwnd as *mut c_void),
    };

    // The caller retains another Arc and supplies the queue to handle_wm_input.
    drop(queue);

    unsafe { RegisterRawInputDevices(&[device], size_of::<RAWINPUTDEVICE>() as u32) }
        .map_err(|error| format!("RegisterRawInputDevices failed: {error}"))
}

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
        return;
    }

    let raw = unsafe { raw.assume_init() };
    if raw.header.dwType != RIM_TYPEMOUSE.0 {
        return;
    }

    let mouse = unsafe { raw.data.mouse };
    let buttons = unsafe { mouse.Anonymous.ulButtons };
    let sample = queue.push_sample(mouse.lLastX, mouse.lLastY, buttons, monotonic_now_ns());
    tracker
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .observe(&sample);
}

use std::{
    ffi::c_void,
    sync::{Arc, Mutex},
};

use bevy::{
    prelude::*,
    window::{PrimaryWindow, RawHandleWrapper},
};
use raw_window_handle::RawWindowHandle;
use sense_input_win::{handle_wm_input, register_raw_mouse, IntegrityTracker, MouseQueue};
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Shell::{DefSubclassProc, SetWindowSubclass},
        WindowsAndMessaging::{WM_INPUT, WM_NCDESTROY},
    },
};

const RAW_INPUT_SUBCLASS_ID: usize = 0x5345_4E53_455F_5241;

#[derive(Resource, Clone)]
pub struct ArcMouseQueue(pub Arc<MouseQueue>);

impl Default for ArcMouseQueue {
    fn default() -> Self {
        Self(Arc::new(MouseQueue::new()))
    }
}

#[derive(Resource, Clone)]
pub struct InputIntegrityTracker(pub Arc<Mutex<IntegrityTracker>>);

impl Default for InputIntegrityTracker {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(IntegrityTracker::default())))
    }
}

pub struct RawInputPlugin;

impl Plugin for RawInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ArcMouseQueue>()
            .init_resource::<InputIntegrityTracker>()
            .add_systems(Startup, install_raw_input_hook);
    }
}

struct HookState {
    queue: Arc<MouseQueue>,
    tracker: Arc<Mutex<IntegrityTracker>>,
}

fn install_raw_input_hook(world: &mut World) {
    let raw_handle = {
        let mut windows = world.query_filtered::<&RawHandleWrapper, With<PrimaryWindow>>();
        windows
            .single(world)
            .expect("primary winit window must expose a raw handle")
            .get_window_handle()
    };
    let RawWindowHandle::Win32(win32) = raw_handle else {
        panic!("sense-maxer raw input requires a Win32 window");
    };
    let hwnd_value = win32.hwnd.get();
    let hwnd = HWND(hwnd_value as *mut c_void);
    let queue = Arc::clone(&world.resource::<ArcMouseQueue>().0);
    let tracker = Arc::clone(&world.resource::<InputIntegrityTracker>().0);

    register_raw_mouse(hwnd_value, Arc::clone(&queue))
        .unwrap_or_else(|error| panic!("raw mouse registration failed: {error}"));
    let state_ptr = Box::into_raw(Box::new(HookState { queue, tracker }));
    let installed = unsafe {
        SetWindowSubclass(
            hwnd,
            Some(raw_input_subclass_proc),
            RAW_INPUT_SUBCLASS_ID,
            state_ptr as usize,
        )
    };
    if !installed.as_bool() {
        unsafe { drop(Box::from_raw(state_ptr)) };
        panic!("SetWindowSubclass failed for the Bevy winit HWND");
    }
}

unsafe extern "system" fn raw_input_subclass_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    state_ptr: usize,
) -> LRESULT {
    let state = unsafe { &*(state_ptr as *const HookState) };
    if message == WM_INPUT {
        handle_wm_input(lparam.0, &state.queue, &state.tracker);
    }
    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    if message == WM_NCDESTROY {
        unsafe { drop(Box::from_raw(state_ptr as *mut HookState)) };
    }
    result
}

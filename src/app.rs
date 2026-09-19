use bevy::{
    prelude::*,
    window::{
        CursorGrabMode, CursorOptions, PresentMode, PrimaryWindow, WindowPlugin, WindowResolution,
    },
};
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};

use crate::{
    aim_trial::{spawn_aim_arena, spawn_aim_target, sync_aim_target, AimTrial},
    camera_ctrl::{
        apply_yaw_transform, drain_mouse_to_camera, ActiveInputProcessor, LiveInputStats,
        ProcessorTimingState,
    },
    config::{ExperimentSettings, LookCapture, TelemetryBuffers, ValidationState},
    frame_telemetry::{record_frame_telemetry, LiveFrameStats},
    input_plugin::RawInputPlugin,
    scene::{maintain_horizontal_fov, setup_scene},
    session::ValidationSession,
    validation_lab::draw_hud,
};

pub fn run() {
    App::new()
        .insert_resource(ExperimentSettings::default())
        .insert_non_send(ActiveInputProcessor::default())
        .init_resource::<ValidationState>()
        .init_resource::<TelemetryBuffers>()
        .init_resource::<ProcessorTimingState>()
        .init_resource::<LiveInputStats>()
        .init_resource::<LiveFrameStats>()
        .init_resource::<ValidationSession>()
        .init_resource::<LookCapture>()
        .init_resource::<crate::camera_ctrl::LookCapturePrev>()
        .init_resource::<AimTrial>()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "sense-maxer — VALORANT Validation Lab".into(),
                resolution: WindowResolution::new(1280, 720),
                // Adopted baseline (exp 0.5.2+): VSync OFF / uncapped — lower perceived aim latency.
                // WM_INPUT path unchanged; do not use frame time for accel dt_s.
                present_mode: PresentMode::AutoNoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins(RawInputPlugin)
        .add_systems(Startup, (setup_scene, spawn_aim_arena, spawn_aim_target))
        .add_systems(
            Update,
            (
                toggle_look_capture,
                apply_cursor_capture,
                drain_mouse_to_camera,
                apply_yaw_transform,
                sync_aim_target,
                record_frame_telemetry,
            )
                .chain(),
        )
        .add_systems(Update, maintain_horizontal_fov)
        .add_systems(EguiPrimaryContextPass, draw_hud)
        .run();
}

fn toggle_look_capture(keys: Res<ButtonInput<KeyCode>>, mut look: ResMut<LookCapture>) {
    if keys.just_pressed(KeyCode::Escape) {
        look.enabled = !look.enabled;
    }
}

fn apply_cursor_capture(
    look: Res<LookCapture>,
    window: Single<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
) {
    let (window, mut cursor) = window.into_inner();
    let capture = look.enabled && window.focused;
    cursor.grab_mode = if capture {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
    cursor.visible = !capture;
}

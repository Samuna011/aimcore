use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts};

use crate::{
    camera_ctrl::{LiveInputStats, YawPitch},
    config::ExperimentSettings,
    frame_telemetry::LiveFrameStats,
};

pub fn draw_hud(
    mut contexts: EguiContexts,
    settings: Res<ExperimentSettings>,
    live_input: Res<LiveInputStats>,
    live_frame: Res<LiveFrameStats>,
    pose: Single<&YawPitch, With<Camera3d>>,
    window: Single<&Window, With<PrimaryWindow>>,
) -> bevy::prelude::Result {
    let config = settings.sensitivity_config();
    let edpi = sense_math::edpi(config.dpi, config.sensitivity);
    let cm_per_360 = sense_math::cm_per_360(config.dpi, config.sensitivity);

    egui::Window::new("VALORANT VALIDATION LAB")
        .anchor(egui::Align2::LEFT_TOP, [12.0, 12.0])
        .resizable(false)
        .collapsible(false)
        .show(contexts.ctx_mut()?, |ui| {
            ui.strong("PITCH MODEL: UNVERIFIED");
            ui.colored_label(egui::Color32::YELLOW, "PITCH ROTATION: DISABLED");
            ui.separator();
            ui.monospace(format!("DPI: {:.0}", config.dpi));
            ui.monospace(format!("SENSITIVITY: {:.3}", config.sensitivity));
            ui.monospace(format!("eDPI: {edpi:.1}"));
            ui.monospace(format!("HFOV: {:.1} deg", config.fov_degrees));
            ui.monospace(format!(
                "RESOLUTION: {} x {}",
                window.physical_width(),
                window.physical_height()
            ));
            ui.monospace(format!(
                "YAW COEFFICIENT: {:.3} deg/count @ sens 1",
                config.yaw_deg_per_count_at_sens_1
            ));
            ui.monospace(format!("cm/360: {cm_per_360:.3}"));
            ui.monospace(format!(
                "counts/360: {:.3}",
                sense_math::counts_per_360(config.sensitivity)
            ));
            ui.separator();
            ui.monospace(format!(
                "LAST RAW: dx {:+}  dy {:+}",
                live_input.last_dx, live_input.last_dy
            ));
            ui.monospace(format!(
                "NET RAW:  dx {:+}  dy {:+}",
                live_input.net_dx, live_input.net_dy
            ));
            ui.monospace(format!("ABS X PATH: {}", live_input.abs_dx));
            ui.monospace(format!(
                "SAMPLES THIS FRAME: {}",
                live_input.samples_this_frame
            ));
            ui.monospace(format!("YAW: {:.6} deg", pose.yaw_deg));
            ui.monospace(format!("PITCH (FROZEN): {:.6} deg", pose.pitch_deg));
            ui.monospace(format!(
                "TOTAL YAW DELTA: {:.6} deg",
                live_input.total_yaw_delta_deg
            ));
            ui.separator();
            ui.monospace(format!("FPS: {:.1}", live_frame.fps));
            ui.monospace(format!(
                "FRAME TIME: {:.3} ms",
                live_frame.frame_time_s * 1_000.0
            ));
        });

    let context = contexts.ctx_mut()?;
    let center = context.content_rect().center();
    let painter = context.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("crosshair"),
    ));
    let color = egui::Color32::WHITE;
    painter.line_segment(
        [
            center + egui::vec2(-7.0, 0.0),
            center + egui::vec2(7.0, 0.0),
        ],
        (1.5, color),
    );
    painter.line_segment(
        [
            center + egui::vec2(0.0, -7.0),
            center + egui::vec2(0.0, 7.0),
        ],
        (1.5, color),
    );

    Ok(())
}

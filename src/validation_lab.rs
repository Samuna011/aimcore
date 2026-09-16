use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::config::ExperimentSettings;

pub fn draw_hud(
    mut contexts: EguiContexts,
    settings: Res<ExperimentSettings>,
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
                "YAW COEFFICIENT: {:.3} deg/count @ sens 1",
                config.yaw_deg_per_count_at_sens_1
            ));
            ui.monospace(format!("cm/360: {cm_per_360:.3}"));
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

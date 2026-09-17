use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts};

use crate::{
    camera_ctrl::{reset_camera, LiveInputStats, YawPitch},
    config::{ExperimentSettings, LookCapture, TelemetryBuffers, ValidationState},
    frame_telemetry::LiveFrameStats,
    input_plugin::InputIntegrityTracker,
    session::{database_path, end_validation, reset_counters, start_validation, ValidationSession},
};

pub fn draw_hud(
    mut contexts: EguiContexts,
    mut settings: ResMut<ExperimentSettings>,
    mut live_input: ResMut<LiveInputStats>,
    live_frame: Res<LiveFrameStats>,
    mut pose: Single<&mut YawPitch, With<Camera3d>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut validation: ResMut<ValidationState>,
    mut buffers: ResMut<TelemetryBuffers>,
    integrity: Res<InputIntegrityTracker>,
    mut session: ResMut<ValidationSession>,
    look: Res<LookCapture>,
) -> bevy::prelude::Result {
    egui::Window::new("VALORANT VALIDATION LAB")
        .anchor(egui::Align2::LEFT_TOP, [12.0, 12.0])
        .resizable(false)
        .collapsible(false)
        .show(contexts.ctx_mut()?, |ui| {
            ui.strong("PITCH MODEL: UNVERIFIED");
            ui.colored_label(egui::Color32::YELLOW, "PITCH ROTATION: DISABLED");
            if look.enabled {
                ui.colored_label(
                    egui::Color32::LIGHT_GREEN,
                    "LOOK MODE — press ESC to unlock cursor and use buttons",
                );
            } else {
                ui.colored_label(
                    egui::Color32::LIGHT_BLUE,
                    "UI MODE — cursor free; press ESC to lock cursor for look / 360°",
                );
            }
            ui.label(
                "Perform one continuous horizontal 360° in a single direction without reversing.",
            );
            ui.horizontal(|ui| {
                if ui.button("Reset Camera").clicked() {
                    reset_camera(
                        &mut pose,
                        &mut buffers,
                        *validation,
                        sense_input_win::monotonic_now_ns(),
                    );
                }
                if ui.button("Reset Counters").clicked() {
                    session.status_message =
                        match reset_counters(&mut live_input, &mut buffers, &integrity) {
                            Ok(()) => Some(
                                "Live counters, telemetry buffers, and integrity reset for this attempt."
                                    .into(),
                            ),
                            Err(error) => Some(format!("Reset counters failed: {error}")),
                        };
                }
            });
            ui.horizontal(|ui| {
                let start_clicked = ui
                    .add_enabled(
                        !validation.is_running(),
                        egui::Button::new("Start Validation"),
                    )
                    .clicked();
                let end_clicked = ui
                    .add_enabled(validation.is_running(), egui::Button::new("End Validation"))
                    .clicked();

                if start_clicked {
                    if let Err(error) = start_validation(
                        &settings,
                        window.physical_width(),
                        window.physical_height(),
                        None,
                        &mut session,
                        &mut validation,
                        &mut live_input,
                        &mut buffers,
                        &integrity,
                    ) {
                        session.status_message = Some(format!("Start failed: {error}"));
                    }
                }
                if end_clicked {
                    if let Err(error) = end_validation(
                        settings.sensitivity,
                        &mut session,
                        &mut validation,
                        &live_input,
                        &buffers,
                        &integrity,
                    ) {
                        session.status_message = Some(format!("End failed: {error}"));
                    }
                }
            });
            ui.monospace(format!(
                "STATE: {}",
                if validation.is_running() {
                    "RUNNING"
                } else {
                    "IDLE"
                }
            ));
            ui.monospace(format!("DATABASE: {}", database_path().display()));
            if let Some(message) = &session.status_message {
                ui.label(message);
            }
            ui.separator();
            ui.label("Declared mouse DPI (must match Logitech setting). Does not change yaw math.");
            ui.add_enabled_ui(!validation.is_running(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("DPI");
                    ui.add(
                        egui::DragValue::new(&mut settings.dpi)
                            .speed(50.0)
                            .range(100.0..=25600.0)
                            .fixed_decimals(0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Sensitivity");
                    ui.add(
                        egui::DragValue::new(&mut settings.sensitivity)
                            .speed(0.001)
                            .range(0.001..=10.0)
                            .fixed_decimals(3),
                    );
                });
            });
            if validation.is_running() {
                ui.small("DPI/sensitivity locked while validation is running (snapshotted at Start).");
            }
            let config = settings.sensitivity_config();
            let edpi = sense_math::edpi(config.dpi, config.sensitivity);
            let cm_per_360 = sense_math::cm_per_360(config.dpi, config.sensitivity);
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
            if let Some(result) = &session.last_result {
                ui.separator();
                ui.strong("VALIDATION RESULT");
                ui.monospace(format!("EXPECTED COUNTS: {:.6}", result.expected_counts));
                ui.monospace(format!(
                    "OBSERVED NET COUNTS: {:+.6}",
                    result.observed_net_counts
                ));
                ui.monospace(format!(
                    "OBSERVED ABS PATH: {:.6}",
                    result.observed_abs_path_counts
                ));
                ui.monospace(format!("EXPECTED DEGREES: {:.6}", result.expected_degrees));
                ui.monospace(format!("OBSERVED DEGREES: {:+.6}", result.observed_degrees));
                ui.monospace(format!("COUNT DIFFERENCE: {:+.6}", result.count_difference));
                ui.monospace(format!("ERROR: {:+.6}%", result.error_percent));
                ui.monospace(format!(
                    "SAMPLES RECEIVED: {}",
                    result.integrity.samples_received
                ));
                ui.monospace(format!("SEQUENCE GAPS: {}", result.integrity.sequence_gaps));
                ui.monospace(format!(
                    "DUPLICATE SEQUENCES: {}",
                    result.integrity.duplicate_sequences
                ));
                ui.monospace(format!(
                    "OUT OF ORDER: {}",
                    result.integrity.out_of_order_samples
                ));
                ui.monospace(format!(
                    "TIMESTAMP REGRESSIONS: {}",
                    result.integrity.timestamp_regressions
                ));
                ui.monospace(format!(
                    "PIPELINE SUSPECT: {}",
                    result.integrity.is_pipeline_suspect()
                ));
            }
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

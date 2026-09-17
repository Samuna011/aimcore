use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts};
use sense_accel::CapMode;

use crate::{
    camera_ctrl::{
        reset_camera, ActiveInputProcessor, LiveInputStats, ProcessorTimingState, YawPitch,
    },
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
    processor_runtime: (
        NonSendMut<ActiveInputProcessor>,
        ResMut<ProcessorTimingState>,
    ),
) -> bevy::prelude::Result {
    let (mut active_processor, mut timing) = processor_runtime;
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
                        match reset_counters(
                            &mut live_input,
                            &mut buffers,
                            &integrity,
                            &mut timing,
                        ) {
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
                        &mut active_processor,
                        &mut timing,
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
                        &active_processor,
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
            if validation.is_running() {
                ui.monospace(format!(
                    "PROCESSOR: {}",
                    active_processor.processor.id()
                ));
                ui.monospace(format!(
                    "PROCESSOR VERSION: {}",
                    active_processor.processor.version()
                ));
            } else {
                ui.monospace(format!("PROCESSOR: {}", settings.processor_id));
                let processor_config = settings.rawaccel_linear_config();
                let version =
                    sense_accel::create_processor(&settings.processor_id, &processor_config)
                        .map(|processor| processor.version().to_string())
                        .unwrap_or_else(|_| "?".into());
                ui.monospace(format!("PROCESSOR VERSION: {version}"));
            }
            ui.small("Under `none`, processed dx/dy equals raw (identity transform).");
            ui.small(
                "Gain on uses Linear Gain; Gain off uses Legacy/Sensitivity. Caps match official classic 1:1.",
            );
            ui.add_enabled_ui(!validation.is_running(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("Processor (Idle only)");
                    egui::ComboBox::from_id_salt("processor_id")
                        .selected_text(&settings.processor_id)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut settings.processor_id, "none".into(), "none");
                            ui.selectable_value(
                                &mut settings.processor_id,
                                "rawaccel_linear".into(),
                                "rawaccel_linear",
                            );
                        });
                });
                if settings.processor_id == "rawaccel_linear" {
                    ui.checkbox(&mut settings.gain, "Gain");
                    ui.horizontal(|ui| {
                        ui.label("Acceleration");
                        ui.add(
                            egui::DragValue::new(&mut settings.acceleration)
                                .speed(0.001)
                                .range(0.0..=10.0)
                                .fixed_decimals(3),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Sensitivity multiplier");
                        ui.add(
                            egui::DragValue::new(&mut settings.sensitivity_multiplier)
                                .speed(0.01)
                                .range(0.01..=10.0)
                                .fixed_decimals(2),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Cap mode");
                        egui::ComboBox::from_id_salt("rawaccel_cap_mode")
                            .selected_text(match settings.cap_mode {
                                CapMode::Out => "out",
                                CapMode::In => "in",
                                CapMode::Io => "io",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut settings.cap_mode,
                                    CapMode::Out,
                                    "out",
                                );
                                ui.selectable_value(
                                    &mut settings.cap_mode,
                                    CapMode::In,
                                    "in",
                                );
                                ui.selectable_value(
                                    &mut settings.cap_mode,
                                    CapMode::Io,
                                    "io",
                                );
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Cap X");
                        ui.add(
                            egui::DragValue::new(&mut settings.cap_x)
                                .speed(0.1)
                                .range(0.0..=10_000.0),
                        )
                        .on_hover_text("Input cap; used by in and io modes.");
                    });
                    ui.horizontal(|ui| {
                        ui.label("Cap Y");
                        ui.add(
                            egui::DragValue::new(&mut settings.cap_y)
                                .speed(0.1)
                                .range(0.0..=10_000.0),
                        )
                        .on_hover_text("Output cap; used by out and io modes.");
                    });
                    ui.horizontal(|ui| {
                        ui.label("Input offset");
                        ui.add(
                            egui::DragValue::new(&mut settings.input_offset)
                                .speed(0.1)
                                .range(0.0..=10_000.0),
                        );
                    });
                    ui.small(
                        "Trainer defaults: Gain on, Output cap 2, acceleration 0.007, multiplier 1.",
                    );
                }
            });
            if validation.is_running() {
                ui.small("Processor locked while validation is running (snapshotted at Start).");
            }
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
            ui.monospace(format!("PRESENT MODE: {:?}", window.present_mode));
            ui.small("VSync: ON (PresentMode::AutoVsync). FPS capped to display refresh. Input remains WM_INPUT-driven, not frame-locked.");
            ui.monospace(format!("FPS: {:.1}", live_frame.fps));
            ui.monospace(format!(
                "FRAME TIME: {:.3} ms",
                live_frame.frame_time_s * 1_000.0
            ));
            if let Some(result) = &session.last_result {
                ui.separator();
                ui.strong("VALIDATION RESULT");
                if session.last_result_processor_id.as_deref() != Some("none") {
                    ui.small(
                        "Under acceleration, M1 expected-counts is not a 360° proof; degrees use camera yaw delta.",
                    );
                }
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

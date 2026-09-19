use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts};
use sense_accel::CapMode;

use crate::{
    aim_gridshot::{
        start_gridshot_trial, GRIDSHOT_CONCURRENT, GRIDSHOT_DURATION_SECS, GRIDSHOT_TRIAL_TYPE,
    },
    aim_trial::{
        cancel_aim_trial, start_aim_trial, AimPhase, AimRunConfigSnapshot, AimTaskKind, AimTrial,
        AIM_HITS_TO_FINISH,
    },
    camera_ctrl::{
        reset_camera, ActiveInputProcessor, LiveInputStats, ProcessorTimingState, YawPitch,
    },
    config::{ExperimentSettings, LookCapture, TelemetryBuffers, ValidationState},
    frame_telemetry::LiveFrameStats,
    input_plugin::InputIntegrityTracker,
    session::{
        database_path, end_validation, reset_counters, start_validation, unix_time_ms,
        ValidationSession,
    },
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
    mut aim: ResMut<AimTrial>,
    processor_runtime: (
        NonSendMut<ActiveInputProcessor>,
        ResMut<ProcessorTimingState>,
    ),
) -> bevy::prelude::Result {
    let (mut active_processor, mut timing) = processor_runtime;
    // Idle (and not Armed): keep live processor in sync with HUD settings so feel
    // tests / SPEED DEBUG work without requiring Start Validation.
    if !validation.is_running() && aim.phase != AimPhase::Armed {
        let id_mismatch = active_processor.processor.id() != settings.processor_id.as_str();
        if settings.is_changed() || id_mismatch {
            if let Ok(processor) =
                sense_accel::create_processor(&settings.processor_id, &settings.rawaccel_linear_config())
            {
                active_processor.processor = processor;
            }
        }
    }

    let ctx = contexts.ctx_mut()?;
    if look.enabled {
        let rect = ctx.content_rect();
        let c = rect.center();
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("aim_crosshair"),
        ));
        let stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(230, 230, 230));
        painter.line_segment([egui::pos2(c.x - 10.0, c.y), egui::pos2(c.x + 10.0, c.y)], stroke);
        painter.line_segment([egui::pos2(c.x, c.y - 10.0), egui::pos2(c.x, c.y + 10.0)], stroke);
    }

    egui::Window::new("VALORANT VALIDATION LAB")
        .anchor(egui::Align2::LEFT_TOP, [12.0, 12.0])
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.strong("UnverifiedPitchModel");
            ui.small("Same 0.07 as yaw; +dy look down; ±89°; UNCERTAIN");
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
                    cancel_aim_trial(&mut aim);
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
            ui.separator();
            ui.strong("STATIC_CLICK (M3)");
            ui.small(
                "5 hits in a front cone (±25° yaw, −5°…+12° pitch, above floor). Hit destroys & respawns; miss keeps target. Score = time for 5 hits.",
            );
            ui.horizontal(|ui| {
                let can_start_aim =
                    !validation.is_running() && aim.phase == AimPhase::Idle;
                if ui
                    .add_enabled(can_start_aim, egui::Button::new("Start Aim Trial"))
                    .clicked()
                {
                    match unix_time_ms() {
                        Ok(start_ms) => {
                            let now = sense_input_win::monotonic_now_ns();
                            let config = AimRunConfigSnapshot::from_live(
                                &settings,
                                active_processor.processor.id(),
                                active_processor.processor.version(),
                                &active_processor.processor.config_json(),
                                window.physical_width(),
                                window.physical_height(),
                            );
                            if start_aim_trial(
                                &mut pose,
                                &mut aim,
                                *validation,
                                now,
                                start_ms,
                                now,
                                config,
                            )
                            {
                                session.status_message = Some(format!(
                                    "Aim run armed — destroy {AIM_HITS_TO_FINISH} green spheres (LMB). Miss keeps the same target."
                                ));
                            }
                        }
                        Err(error) => {
                            session.status_message =
                                Some(format!("Aim start failed: wall clock unavailable ({error})"));
                        }
                    }
                }
                if ui
                    .add_enabled(aim.phase == AimPhase::Armed, egui::Button::new("Cancel Aim"))
                    .clicked()
                {
                    cancel_aim_trial(&mut aim);
                }
            });
            ui.separator();
            ui.strong(format!("{GRIDSHOT_TRIAL_TYPE} (M4.a)"));
            ui.small(
                "60s timed run; 3 exclusive cells on a 3×3 wall; hit → immediate vacant respawn; miss unchanged. Score = hits / accuracy. After Start, press Esc to lock look/aim; Esc again returns to UI (cancels mid-run).",
            );
            ui.horizontal(|ui| {
                let can_start_grid =
                    !validation.is_running() && aim.phase == AimPhase::Idle;
                if ui
                    .add_enabled(can_start_grid, egui::Button::new("Start Gridshot"))
                    .clicked()
                {
                    match unix_time_ms() {
                        Ok(start_ms) => {
                            let now = sense_input_win::monotonic_now_ns();
                            let seed = now;
                            let config = AimRunConfigSnapshot::from_live(
                                &settings,
                                active_processor.processor.id(),
                                active_processor.processor.version(),
                                &active_processor.processor.config_json(),
                                window.physical_width(),
                                window.physical_height(),
                            );
                            if start_gridshot_trial(
                                &mut pose,
                                &mut aim,
                                *validation,
                                now,
                                start_ms,
                                seed,
                                config,
                            )
                            {
                                session.status_message = Some(format!(
                                    "Gridshot armed — {GRIDSHOT_DURATION_SECS:.0}s, {GRIDSHOT_CONCURRENT} live targets (LMB)."
                                ));
                            }
                        }
                        Err(error) => {
                            session.status_message = Some(format!(
                                "Gridshot start failed: wall clock unavailable ({error})"
                            ));
                        }
                    }
                }
                if ui
                    .add_enabled(
                        aim.phase == AimPhase::Armed && aim.task_kind == AimTaskKind::Gridshot,
                        egui::Button::new("Cancel Gridshot"),
                    )
                    .clicked()
                {
                    cancel_aim_trial(&mut aim);
                }
            });
            match aim.phase {
                AimPhase::Armed => {
                    let elapsed = sense_input_win::monotonic_now_ns()
                        .saturating_sub(aim.start_timestamp_ns) as f64
                        / 1e9;
                    let shots = aim.shot_log.len() as u32;
                    let accuracy = if shots == 0 {
                        0.0
                    } else {
                        aim.hits as f64 / shots as f64
                    };
                    match aim.task_kind {
                        AimTaskKind::StaticClick => {
                            ui.monospace(format!(
                                "AIM: Armed  HITS: {}/{AIM_HITS_TO_FINISH}  ELAPSED: {elapsed:.3} s",
                                aim.hits
                            ));
                        }
                        AimTaskKind::Gridshot => {
                            let remaining =
                                (GRIDSHOT_DURATION_SECS - elapsed).max(0.0);
                            ui.monospace(format!(
                                "GRIDSHOT: Armed  TIME LEFT: {remaining:.1} s  HITS: {}  SHOTS: {shots}  ACC: {:.1}%  LIVE: {}",
                                aim.hits,
                                accuracy * 100.0,
                                aim.live_targets.len()
                            ));
                        }
                    }
                    let proc_id = active_processor.processor.id();
                    let short_cfg = if proc_id == "none" {
                        "none".to_string()
                    } else {
                        let gain = if settings.gain { "on" } else { "off" };
                        let cap = match settings.cap_mode {
                            CapMode::Out => "out",
                            CapMode::In => "in",
                            CapMode::Io => "io",
                        };
                        format!(
                            "{proc_id} gain={gain} caps={cap}/{:.0}/{:.0}",
                            settings.cap_x, settings.cap_y
                        )
                    };
                    ui.monospace(format!("PROCESSOR: {short_cfg}"));
                }
                AimPhase::Idle => {
                    match aim.task_kind {
                        AimTaskKind::StaticClick => {
                            ui.monospace(format!(
                                "AIM: Idle  HITS: {}/{AIM_HITS_TO_FINISH}",
                                aim.hits
                            ));
                        }
                        AimTaskKind::Gridshot => {
                            ui.monospace(format!(
                                "GRIDSHOT: Idle  HITS: {}  LIVE: 0",
                                aim.hits
                            ));
                        }
                    }
                    if let Some(secs) = aim.score_secs {
                        let shots = aim.shot_log.len() as u32;
                        let accuracy = if shots == 0 {
                            0.0
                        } else {
                            aim.hits as f64 / shots as f64
                        };
                        match aim.task_kind {
                            AimTaskKind::StaticClick => {
                                ui.strong(format!(
                                    "RUN SCORE: {secs:.3} s  ({AIM_HITS_TO_FINISH} hits)"
                                ));
                            }
                            AimTaskKind::Gridshot => {
                                ui.strong(format!(
                                    "GRIDSHOT SCORE: {} hits in {secs:.3} s",
                                    aim.hits
                                ));
                            }
                        }
                        ui.monospace(format!(
                            "DURATION: {secs:.3} s  ACCURACY: {}/{} ({:.1}%)",
                            aim.hits,
                            shots,
                            accuracy * 100.0
                        ));
                    }
                    if let Some(persist) = &aim.last_persist {
                        if persist.saved_ok {
                            if let Some(id) = &persist.trial_id {
                                ui.monospace(format!("SAVED: {id}"));
                            }
                        } else if let Some(error) = &persist.error {
                            ui.colored_label(
                                egui::Color32::LIGHT_RED,
                                format!("SAVE FAILED: {error}"),
                            );
                        }
                    }
                }
            }
            match aim.last_hit {
                Some(true) => {
                    ui.monospace(format!(
                        "LAST SHOT: HIT  (yaw {:.3}, pitch {:.3})",
                        aim.last_yaw_deg, aim.last_pitch_deg
                    ));
                }
                Some(false) => {
                    let miss_note = match aim.task_kind {
                        AimTaskKind::StaticClick => " — same target",
                        AimTaskKind::Gridshot => " — targets unchanged",
                    };
                    ui.monospace(format!(
                        "LAST SHOT: MISS (yaw {:.3}, pitch {:.3}){miss_note}",
                        aim.last_yaw_deg, aim.last_pitch_deg
                    ));
                }
                None => {
                    ui.monospace("LAST SHOT: —");
                }
            }
            let settings_locked = validation.is_running() || aim.phase == AimPhase::Armed;
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
            ui.add_enabled_ui(!settings_locked, |ui| {
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
                        .on_hover_text("Input cap; used by in and io. For out, stored for UI; effective knee is derived from Cap Y + accel.");
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
                    ui.horizontal(|ui| {
                        ui.label("Poll rate Hz (speed dt floor)");
                        ui.add(
                            egui::DragValue::new(&mut settings.polling_rate_hz)
                                .speed(1)
                                .range(0..=8000),
                        )
                        .on_hover_text(
                            "EXPLICIT trainer: speed dt_ms floor = 1000/Hz when > 0. 0 uses RA default min 0.0625 ms.",
                        );
                    });
                    ui.small(
                        "Trainer defaults: Gain on, Output cap, Cap X/Y = 2, acceleration 0.007, multiplier 1, poll floor 1000 Hz.",
                    );
                    ui.small(
                        "Poll-period floor compensates user-mode WM_INPUT timestamps — not Device DPI.",
                    );
                }
            });
            if validation.is_running() {
                ui.small("Processor locked while validation is running (snapshotted at Start).");
            } else if aim.phase == AimPhase::Armed {
                ui.small("Processor locked while aim trial is Armed (snapshotted at Start Aim).");
            }
            if live_input.has_accel_debug {
                ui.separator();
                ui.strong("SPEED DEBUG (last sample)");
                ui.monospace(format!(
                    "DT_MS raw {:.4} → speed {:.4}{}",
                    live_input.last_raw_dt_ms,
                    live_input.last_speed_dt_ms,
                    if live_input.last_time_clamped {
                        " (CLAMPED)"
                    } else if live_input.last_bypassed_dt {
                        " (BYPASS)"
                    } else {
                        ""
                    }
                ));
                ui.monospace(format!(
                    "INPUT SPEED: {:.4} counts/ms",
                    live_input.last_input_speed
                ));
                ui.monospace(format!(
                    "ACCEL SCALE: {:.6}",
                    live_input.last_acceleration_scale
                ));
            }
            if let Some(message) = &session.status_message {
                ui.label(message);
            }
            ui.separator();
            ui.label("Declared mouse DPI (must match Logitech setting). Does not change yaw math.");
            ui.add_enabled_ui(!settings_locked, |ui| {
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
            } else if aim.phase == AimPhase::Armed {
                ui.small("DPI/sensitivity locked while aim trial is Armed (snapshotted at Start Aim).");
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
            ui.monospace(format!("PITCH: {:.6} deg", pose.pitch_deg));
            ui.monospace(format!(
                "TOTAL YAW DELTA: {:.6} deg",
                live_input.total_yaw_delta_deg
            ));
            ui.separator();
            ui.monospace(format!("PRESENT MODE: {:?}", window.present_mode));
            ui.small("VSync: OFF (PresentMode::AutoNoVsync). FPS uncapped (adopted exp 0.5.2). Input remains WM_INPUT-driven, not frame-locked.");
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

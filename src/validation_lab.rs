use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts};
use sense_accel::CapMode;

use crate::{
    aim_gridshot::{start_gridshot_trial, GRIDSHOT_CONCURRENT, GRIDSHOT_DURATION_SECS},
    aim_trial::{
        active_elapsed_ns, cancel_aim_trial, end_aim_pause, start_aim_trial, AimPhase,
        AimRunConfigSnapshot, AimTaskKind, AimTrial, AIM_EXPERIMENT_VERSION, AIM_HITS_TO_FINISH,
    },
    camera_ctrl::{
        reset_camera, ActiveInputProcessor, LiveInputStats, ProcessorTimingState, YawPitch,
    },
    config::{ExperimentSettings, LookCapture, TelemetryBuffers, ValidationState},
    frame_telemetry::LiveFrameStats,
    input_plugin::InputIntegrityTracker,
    lab_ui::{
        enter_playing_after_start, settings_are_locked, LabNested, LabScreen, LabUi,
        LightweightResult,
    },
    session::{
        database_path, end_validation, reset_counters, start_validation, unix_time_ms,
        ValidationSession,
    },
};

#[derive(Debug, Clone, Copy)]
enum HudAction {
    Start,
    Resume,
    Restart,
    ChangeTrial,
    OpenSettings,
    OpenLabTools,
    Back,
    Exit,
    StartValidation,
    EndValidation,
    ResetCamera,
    ResetCounters,
}

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
    mut look: ResMut<LookCapture>,
    mut aim: ResMut<AimTrial>,
    mut lab_ui: ResMut<LabUi>,
    mut app_exit: MessageWriter<AppExit>,
    processor_runtime: (
        NonSendMut<ActiveInputProcessor>,
        ResMut<ProcessorTimingState>,
    ),
) -> bevy::prelude::Result {
    let (mut active_processor, mut timing) = processor_runtime;
    if !validation.is_running() && aim.phase != AimPhase::Armed {
        let id_mismatch = active_processor.processor.id() != settings.processor_id.as_str();
        if settings.is_changed() || id_mismatch {
            if let Ok(processor) = sense_accel::create_processor(
                &settings.processor_id,
                &settings.rawaccel_linear_config(),
            ) {
                active_processor.processor = processor;
            }
        }
    }

    let ctx = contexts.ctx_mut()?;
    if lab_ui.screen == LabScreen::Playing {
        draw_crosshair(ctx);
        draw_playing_hud(ctx, &aim, &lab_ui, &live_input, &live_frame, &pose);
        return Ok(());
    }

    let title = match (lab_ui.screen, lab_ui.nested) {
        (LabScreen::Lobby, LabNested::None) => "Sense Maxer — Lobby",
        (LabScreen::Paused, LabNested::PauseHome) => "Paused",
        (_, LabNested::Settings) => "Settings",
        (_, LabNested::LabTools) => "Lab tools",
        _ => "Sense Maxer",
    };
    let settings_locked = settings_are_locked(aim.phase, *validation);
    let mut action = None;
    egui::Window::new(title)
        .anchor(egui::Align2::LEFT_TOP, [12.0, 12.0])
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| match lab_ui.nested {
            LabNested::None if lab_ui.screen == LabScreen::Lobby => {
                draw_lobby(
                    ui,
                    &mut lab_ui,
                    !validation.is_running() && aim.phase == AimPhase::Idle,
                    &session,
                    &mut action,
                );
            }
            LabNested::PauseHome if lab_ui.screen == LabScreen::Paused => {
                draw_pause_home(ui, &aim, &session, &mut action);
            }
            LabNested::Settings => {
                draw_settings(ui, &mut settings, settings_locked);
                if ui.button("Back").clicked() {
                    action = Some(HudAction::Back);
                }
            }
            LabNested::LabTools => {
                draw_lab_tools(ui, *validation, &aim, &session, &mut action);
            }
            _ => {
                ui.label("Returning to the main surface.");
                action = Some(HudAction::Back);
            }
        });

    match action {
        Some(HudAction::Start) => {
            if start_selected_trial(
                &mut lab_ui,
                &settings,
                &active_processor,
                &window,
                &mut pose,
                &mut aim,
                *validation,
                &mut session,
            ) {
                look.enabled = true;
            }
        }
        Some(HudAction::Resume) => {
            end_aim_pause(&mut aim, sense_input_win::monotonic_now_ns());
            timing.reset();
            lab_ui.screen = LabScreen::Playing;
            lab_ui.nested = LabNested::None;
            look.enabled = true;
        }
        Some(HudAction::Restart) => {
            cancel_aim_trial(&mut aim);
            if start_selected_trial(
                &mut lab_ui,
                &settings,
                &active_processor,
                &window,
                &mut pose,
                &mut aim,
                *validation,
                &mut session,
            ) {
                look.enabled = true;
            } else {
                lab_ui.screen = LabScreen::Lobby;
                lab_ui.nested = LabNested::None;
                look.enabled = false;
            }
        }
        Some(HudAction::ChangeTrial) => {
            cancel_aim_trial(&mut aim);
            lab_ui.screen = LabScreen::Lobby;
            lab_ui.nested = LabNested::None;
            look.enabled = false;
        }
        Some(HudAction::OpenSettings) => lab_ui.nested = LabNested::Settings,
        Some(HudAction::OpenLabTools) => lab_ui.nested = LabNested::LabTools,
        Some(HudAction::Back) => {
            lab_ui.nested = if lab_ui.screen == LabScreen::Paused {
                LabNested::PauseHome
            } else {
                LabNested::None
            };
        }
        Some(HudAction::Exit) => {
            cancel_aim_trial(&mut aim);
            app_exit.write(AppExit::Success);
        }
        Some(HudAction::StartValidation) => {
            if aim.phase != AimPhase::Armed {
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
                } else {
                    lab_ui.screen = LabScreen::Validating;
                    lab_ui.nested = LabNested::LabTools;
                    lab_ui.validation_look_enabled = false;
                    look.enabled = false;
                }
            }
        }
        Some(HudAction::EndValidation) => {
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
            } else {
                lab_ui.screen = LabScreen::Lobby;
                lab_ui.nested = LabNested::LabTools;
                lab_ui.validation_look_enabled = false;
                look.enabled = false;
            }
        }
        Some(HudAction::ResetCamera) => {
            reset_camera(
                &mut pose,
                &mut buffers,
                *validation,
                sense_input_win::monotonic_now_ns(),
            );
            session.status_message = Some("Camera reset.".into());
        }
        Some(HudAction::ResetCounters) => {
            match reset_counters(&mut live_input, &mut buffers, &integrity, &mut timing) {
                Ok(()) => session.status_message = Some("Validation counters reset.".into()),
                Err(error) => {
                    session.status_message = Some(format!("Counter reset failed: {error}"))
                }
            }
        }
        None => {}
    }

    Ok(())
}

fn draw_lobby(
    ui: &mut egui::Ui,
    lab_ui: &mut LabUi,
    can_start: bool,
    session: &ValidationSession,
    action: &mut Option<HudAction>,
) {
    ui.heading("Validation Lab");
    ui.label("Choose a trial, then enter the arena.");
    ui.separator();
    ui.strong("Trial");
    ui.radio_value(
        &mut lab_ui.selected_task,
        AimTaskKind::StaticClick,
        "Static Click",
    );
    ui.radio_value(&mut lab_ui.selected_task, AimTaskKind::Gridshot, "Gridshot");
    ui.horizontal(|ui| {
        if ui
            .add_enabled(can_start, egui::Button::new("Start"))
            .clicked()
        {
            *action = Some(HudAction::Start);
        }
        if ui.button("Settings").clicked() {
            *action = Some(HudAction::OpenSettings);
        }
        if ui.button("Lab tools").clicked() {
            *action = Some(HudAction::OpenLabTools);
        }
        if ui.button("Exit").clicked() {
            *action = Some(HudAction::Exit);
        }
    });
    if let Some(result) = &lab_ui.last_result {
        ui.separator();
        draw_last_result(ui, result);
    }
    if let Some(message) = &session.status_message {
        ui.separator();
        ui.label(message);
    }
}

fn draw_last_result(ui: &mut egui::Ui, result: &LightweightResult) {
    ui.strong("Last result");
    ui.monospace(format!(
        "{} · {} hits · {:.1}% · {:.3}s",
        result.task_label,
        result.hits,
        result.accuracy * 100.0,
        result.duration_secs,
    ));
    if result.save_failed {
        ui.colored_label(egui::Color32::LIGHT_RED, "SAVE FAILED");
    } else if let Some(id) = &result.saved_id {
        ui.colored_label(egui::Color32::LIGHT_GREEN, format!("SAVED: {id}"));
    }
}

fn draw_pause_home(
    ui: &mut egui::Ui,
    aim: &AimTrial,
    session: &ValidationSession,
    action: &mut Option<HudAction>,
) {
    ui.label(match aim.task_kind {
        AimTaskKind::StaticClick => "Static Click is paused.",
        AimTaskKind::Gridshot => "Gridshot is paused.",
    });
    if ui.button("Resume").clicked() {
        *action = Some(HudAction::Resume);
    }
    if ui.button("Restart").clicked() {
        *action = Some(HudAction::Restart);
    }
    if ui.button("Change trial").clicked() {
        *action = Some(HudAction::ChangeTrial);
    }
    ui.separator();
    if ui.button("Settings").clicked() {
        *action = Some(HudAction::OpenSettings);
    }
    if ui.button("Lab tools").clicked() {
        *action = Some(HudAction::OpenLabTools);
    }
    if ui.button("Exit").clicked() {
        *action = Some(HudAction::Exit);
    }
    if let Some(message) = &session.status_message {
        ui.separator();
        ui.label(message);
    }
}

fn draw_settings(ui: &mut egui::Ui, settings: &mut ExperimentSettings, locked: bool) {
    if locked {
        ui.colored_label(
            egui::Color32::YELLOW,
            "Locked while a trial is Armed or validation is running.",
        );
    }
    ui.add_enabled_ui(!locked, |ui| {
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
        ui.horizontal(|ui| {
            ui.label("Horizontal FOV");
            ui.add(
                egui::DragValue::new(&mut settings.fov_degrees_h)
                    .speed(0.1)
                    .range(30.0..=150.0)
                    .fixed_decimals(1),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Processor");
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
            setting_drag(
                ui,
                "Acceleration",
                &mut settings.acceleration,
                0.001,
                0.0..=10.0,
            );
            setting_drag(
                ui,
                "Sensitivity multiplier",
                &mut settings.sensitivity_multiplier,
                0.01,
                0.01..=10.0,
            );
            setting_drag(
                ui,
                "Input offset",
                &mut settings.input_offset,
                0.1,
                0.0..=10_000.0,
            );
            ui.horizontal(|ui| {
                ui.label("Cap mode");
                egui::ComboBox::from_id_salt("rawaccel_cap_mode")
                    .selected_text(match settings.cap_mode {
                        CapMode::Out => "out",
                        CapMode::In => "in",
                        CapMode::Io => "io",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut settings.cap_mode, CapMode::Out, "out");
                        ui.selectable_value(&mut settings.cap_mode, CapMode::In, "in");
                        ui.selectable_value(&mut settings.cap_mode, CapMode::Io, "io");
                    });
            });
            setting_drag(ui, "Cap X", &mut settings.cap_x, 0.1, 0.0..=10_000.0);
            setting_drag(ui, "Cap Y", &mut settings.cap_y, 0.1, 0.0..=10_000.0);
            ui.horizontal(|ui| {
                ui.label("Poll rate Hz");
                ui.add(
                    egui::DragValue::new(&mut settings.polling_rate_hz)
                        .speed(1)
                        .range(0..=8000),
                );
            });
        }
        ui.separator();
        ui.monospace(format!(
            "eDPI: {:.1} · counts/360: {:.1} · cm/360: {:.2}",
            sense_math::edpi(settings.dpi, settings.sensitivity),
            sense_math::counts_per_360(settings.sensitivity),
            sense_math::cm_per_360(settings.dpi, settings.sensitivity),
        ));
    });
}

fn setting_drag(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).speed(speed).range(range));
    });
}

fn draw_lab_tools(
    ui: &mut egui::Ui,
    validation: ValidationState,
    aim: &AimTrial,
    session: &ValidationSession,
    action: &mut Option<HudAction>,
) {
    ui.label("Separate 360° validation workflow.");
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                !validation.is_running() && aim.phase != AimPhase::Armed,
                egui::Button::new("Start Validation"),
            )
            .clicked()
        {
            *action = Some(HudAction::StartValidation);
        }
        if ui
            .add_enabled(validation.is_running(), egui::Button::new("End Validation"))
            .clicked()
        {
            *action = Some(HudAction::EndValidation);
        }
    });
    ui.horizontal(|ui| {
        if ui.button("Reset Camera").clicked() {
            *action = Some(HudAction::ResetCamera);
        }
        if ui.button("Reset Counters").clicked() {
            *action = Some(HudAction::ResetCounters);
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
    if let Some(message) = &session.status_message {
        ui.label(message);
    }
    if let Some(result) = &session.last_result {
        ui.separator();
        ui.strong("Last validation result");
        ui.monospace(format!(
            "Counts expected: {:.3} · observed net: {:+.3} · path: {:.3}",
            result.expected_counts, result.observed_net_counts, result.observed_abs_path_counts,
        ));
        ui.monospace(format!(
            "Degrees expected: {:.3}° · observed: {:+.3}° · error {:+.4}%",
            result.expected_degrees, result.observed_degrees, result.error_percent
        ));
        ui.monospace(format!(
            "Samples: {} · gaps: {} · suspect: {}",
            result.integrity.samples_received,
            result.integrity.sequence_gaps,
            result.integrity.is_pipeline_suspect(),
        ));
    }
    if ui
        .add_enabled(!validation.is_running(), egui::Button::new("Back"))
        .clicked()
    {
        *action = Some(HudAction::Back);
    }
}

fn draw_playing_hud(
    ctx: &egui::Context,
    aim: &AimTrial,
    lab_ui: &LabUi,
    live_input: &LiveInputStats,
    live_frame: &LiveFrameStats,
    pose: &YawPitch,
) {
    let elapsed = active_elapsed_ns(aim, sense_input_win::monotonic_now_ns()) as f64 / 1e9;
    let shots = aim.shot_log.len() as u32;
    let accuracy = if shots == 0 {
        0.0
    } else {
        aim.hits as f64 / shots as f64
    };
    egui::Area::new(egui::Id::new("playing_score"))
        .anchor(egui::Align2::CENTER_TOP, [0.0, 12.0])
        .show(ctx, |ui| {
            let line = match aim.task_kind {
                AimTaskKind::StaticClick => format!(
                    "STATIC CLICK · {}/{} hits · {}/{} shots · {:.1}% · {:.3}s",
                    aim.hits,
                    AIM_HITS_TO_FINISH,
                    aim.hits,
                    shots,
                    accuracy * 100.0,
                    elapsed,
                ),
                AimTaskKind::Gridshot => format!(
                    "GRIDSHOT · {} hits / {} shots · {:.1}% · {:.1}s",
                    aim.hits,
                    shots,
                    accuracy * 100.0,
                    (GRIDSHOT_DURATION_SECS - elapsed).max(0.0),
                ),
            };
            ui.monospace(line);
        });
    if !lab_ui.detail_overlay {
        return;
    }
    egui::Window::new("Run details")
        .anchor(egui::Align2::LEFT_TOP, [12.0, 12.0])
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            if let Some(snapshot) = &aim.config_snapshot {
                ui.monospace(format!(
                    "DPI {:.0} · SENS {:.3} · HFOV {:.1}°",
                    snapshot.dpi, snapshot.sensitivity, snapshot.fov_degrees_h
                ));
                ui.monospace(format!(
                    "PROCESSOR {} v{}",
                    snapshot.processor_id, snapshot.processor_version
                ));
                ui.small(short_config(&snapshot.processor_config_json));
                ui.monospace(format!(
                    "SEED {} · EXPERIMENT {}",
                    aim.random_seed, AIM_EXPERIMENT_VERSION
                ));
            }
            ui.separator();
            ui.monospace(format!(
                "FPS {:.1} · FRAME {:.3} ms",
                live_frame.fps,
                live_frame.frame_time_s * 1_000.0
            ));
            ui.monospace(format!(
                "RAW Δ {:+}, {:+} · YAW {:.3}° · PITCH {:.3}°",
                live_input.last_dx, live_input.last_dy, pose.yaw_deg, pose.pitch_deg
            ));
            ui.monospace(match aim.last_hit {
                Some(true) => "LAST SHOT: HIT",
                Some(false) => "LAST SHOT: MISS",
                None => "LAST SHOT: —",
            });
            ui.monospace(format!("DB: {}", database_path().display()));
        });
}

fn draw_crosshair(ctx: &egui::Context) {
    let center = ctx.content_rect().center();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("aim_crosshair"),
    ));
    let stroke = egui::Stroke::new(1.5, egui::Color32::WHITE);
    painter.line_segment(
        [
            center + egui::vec2(-7.0, 0.0),
            center + egui::vec2(7.0, 0.0),
        ],
        stroke,
    );
    painter.line_segment(
        [
            center + egui::vec2(0.0, -7.0),
            center + egui::vec2(0.0, 7.0),
        ],
        stroke,
    );
}

fn short_config(config: &str) -> String {
    const MAX_CHARS: usize = 96;
    let mut short: String = config.chars().take(MAX_CHARS).collect();
    if config.chars().count() > MAX_CHARS {
        short.push('…');
    }
    short
}

fn start_selected_trial(
    lab_ui: &mut LabUi,
    settings: &ExperimentSettings,
    active_processor: &ActiveInputProcessor,
    window: &Window,
    pose: &mut YawPitch,
    aim: &mut AimTrial,
    validation: ValidationState,
    session: &mut ValidationSession,
) -> bool {
    let start_ms = match unix_time_ms() {
        Ok(value) => value,
        Err(error) => {
            session.status_message = Some(format!(
                "Trial start failed: wall clock unavailable ({error})"
            ));
            return false;
        }
    };
    let now = sense_input_win::monotonic_now_ns();
    let config = AimRunConfigSnapshot::from_live(
        settings,
        active_processor.processor.id(),
        active_processor.processor.version(),
        &active_processor.processor.config_json(),
        window.physical_width(),
        window.physical_height(),
    );
    let started = match lab_ui.selected_task {
        AimTaskKind::StaticClick => {
            start_aim_trial(pose, aim, validation, now, start_ms, now, config)
        }
        AimTaskKind::Gridshot => {
            start_gridshot_trial(pose, aim, validation, now, start_ms, now, config)
        }
    };
    enter_playing_after_start(lab_ui, started);
    if started {
        session.status_message = Some(match lab_ui.selected_task {
            AimTaskKind::StaticClick => {
                format!("Static Click armed — destroy {AIM_HITS_TO_FINISH} targets.")
            }
            AimTaskKind::Gridshot => format!(
                "Gridshot armed — {GRIDSHOT_DURATION_SECS:.0}s, {GRIDSHOT_CONCURRENT} live targets."
            ),
        });
    }
    started
}

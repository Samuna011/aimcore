use bevy::prelude::*;

use crate::{
    aim_trial::{begin_aim_pause, end_aim_pause, AimPhase, AimTaskKind, AimTrial},
    config::ValidationState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LabScreen {
    #[default]
    Lobby,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum LabNested {
    #[default]
    None,
    Settings,
    LabTools,
    PauseHome,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct LightweightResult {
    pub task_label: String,
    pub hits: u32,
    pub shots: u32,
    pub accuracy: f64,
    pub duration_secs: f64,
    pub saved_id: Option<String>,
    pub save_failed: bool,
}

#[derive(Resource, Debug, Clone)]
pub struct LabUi {
    pub screen: LabScreen,
    pub nested: LabNested,
    pub selected_task: AimTaskKind,
    pub detail_overlay: bool,
    pub last_result: Option<LightweightResult>,
}

impl Default for LabUi {
    fn default() -> Self {
        Self {
            screen: LabScreen::Lobby,
            nested: LabNested::None,
            selected_task: AimTaskKind::StaticClick,
            detail_overlay: false,
            last_result: None,
        }
    }
}

/// Handle Esc / V input for the lab shell.
pub fn handle_lab_keys(
    keys: &ButtonInput<KeyCode>,
    ui: &mut LabUi,
    aim: &mut AimTrial,
    now_ns: u64,
) {
    if keys.just_pressed(KeyCode::KeyV) && ui.screen == LabScreen::Playing {
        ui.detail_overlay = !ui.detail_overlay;
    }
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    match ui.screen {
        LabScreen::Lobby => {}
        LabScreen::Playing => {
            begin_aim_pause(aim, now_ns);
            ui.screen = LabScreen::Paused;
            ui.nested = LabNested::PauseHome;
        }
        LabScreen::Paused => {
            end_aim_pause(aim, now_ns);
            ui.nested = LabNested::None;
            ui.screen = LabScreen::Playing;
        }
    }
}

pub fn look_should_be_enabled(ui: &LabUi) -> bool {
    ui.screen == LabScreen::Playing
}

pub fn enter_playing_after_start(ui: &mut LabUi, started: bool) {
    if started {
        ui.screen = LabScreen::Playing;
        ui.nested = LabNested::None;
    }
}

pub fn finish_trial_ui(ui: &mut LabUi, aim: &AimTrial) {
    let Some(persist) = aim.last_persist.as_ref() else {
        return;
    };
    ui.last_result = Some(LightweightResult {
        task_label: match aim.task_kind {
            AimTaskKind::StaticClick => "Static Click",
            AimTaskKind::Gridshot => "Gridshot",
        }
        .into(),
        hits: persist.hits,
        shots: persist.shots,
        accuracy: persist.accuracy,
        duration_secs: persist.score_secs,
        saved_id: persist.trial_id.clone(),
        save_failed: !persist.saved_ok,
    });
    ui.screen = LabScreen::Lobby;
    ui.nested = LabNested::None;
}

pub fn settings_are_locked(aim_phase: AimPhase, validation: ValidationState) -> bool {
    aim_phase == AimPhase::Armed || validation.is_running()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aim_trial::{AimPhase, AimTaskKind, AimTrial};
    use bevy::input::ButtonInput;
    use bevy::prelude::KeyCode;

    fn pressed(key: KeyCode) -> ButtonInput<KeyCode> {
        let mut keys = ButtonInput::default();
        keys.press(key);
        keys
    }

    fn armed_trial() -> AimTrial {
        let mut aim = AimTrial::default();
        aim.phase = AimPhase::Armed;
        aim.start_timestamp_ns = 1_000;
        aim.random_seed = 42;
        aim.current_target_id = "target_001".into();
        aim
    }

    #[test]
    fn default_is_static_click_lobby() {
        let ui = LabUi::default();

        assert_eq!(ui.screen, LabScreen::Lobby);
        assert_eq!(ui.nested, LabNested::None);
        assert_eq!(ui.selected_task, AimTaskKind::StaticClick);
        assert!(!ui.detail_overlay);
        assert!(ui.last_result.is_none());
        assert!(!look_should_be_enabled(&ui));
    }

    #[test]
    fn escape_pauses_without_aborting_trial() {
        let mut ui = LabUi {
            screen: LabScreen::Playing,
            ..Default::default()
        };
        let mut aim = armed_trial();
        let keys = pressed(KeyCode::Escape);

        handle_lab_keys(&keys, &mut ui, &mut aim, 11_000);

        assert_eq!(ui.screen, LabScreen::Paused);
        assert_eq!(ui.nested, LabNested::PauseHome);
        assert_eq!(aim.phase, AimPhase::Armed);
        assert_eq!(aim.paused_at_qpc, Some(11_000));
        assert_eq!(aim.random_seed, 42);
        assert_eq!(aim.current_target_id, "target_001");
        assert!(!look_should_be_enabled(&ui));
    }

    #[test]
    fn escape_resumes_from_nested_pause_and_accounts_time() {
        let mut ui = LabUi {
            screen: LabScreen::Paused,
            nested: LabNested::Settings,
            ..Default::default()
        };
        let mut aim = armed_trial();
        aim.paused_at_qpc = Some(11_000);
        let keys = pressed(KeyCode::Escape);

        handle_lab_keys(&keys, &mut ui, &mut aim, 31_000);

        assert_eq!(ui.screen, LabScreen::Playing);
        assert_eq!(ui.nested, LabNested::None);
        assert_eq!(aim.phase, AimPhase::Armed);
        assert_eq!(aim.paused_at_qpc, None);
        assert_eq!(aim.accumulated_pause_ns, 20_000);
        assert!(look_should_be_enabled(&ui));
    }

    #[test]
    fn escape_in_lobby_is_noop() {
        let mut ui = LabUi::default();
        let mut aim = armed_trial();
        let keys = pressed(KeyCode::Escape);

        handle_lab_keys(&keys, &mut ui, &mut aim, 11_000);

        assert_eq!(ui.screen, LabScreen::Lobby);
        assert_eq!(ui.nested, LabNested::None);
        assert_eq!(aim.phase, AimPhase::Armed);
        assert_eq!(aim.paused_at_qpc, None);
    }

    #[test]
    fn v_toggles_detail_only_while_playing() {
        let keys = pressed(KeyCode::KeyV);
        let mut aim = armed_trial();
        let mut playing = LabUi {
            screen: LabScreen::Playing,
            ..Default::default()
        };

        handle_lab_keys(&keys, &mut playing, &mut aim, 2_000);
        assert!(playing.detail_overlay);

        let mut paused = LabUi {
            screen: LabScreen::Paused,
            ..Default::default()
        };
        handle_lab_keys(&keys, &mut paused, &mut aim, 2_000);
        assert!(!paused.detail_overlay);
    }

    #[test]
    fn successful_start_enters_playing_and_keeps_overlay_preference() {
        let mut ui = LabUi {
            nested: LabNested::Settings,
            detail_overlay: true,
            ..Default::default()
        };

        enter_playing_after_start(&mut ui, true);

        assert_eq!(ui.screen, LabScreen::Playing);
        assert_eq!(ui.nested, LabNested::None);
        assert!(ui.detail_overlay);
    }

    #[test]
    fn failed_start_stays_on_current_surface() {
        let mut ui = LabUi {
            nested: LabNested::Settings,
            ..Default::default()
        };

        enter_playing_after_start(&mut ui, false);

        assert_eq!(ui.screen, LabScreen::Lobby);
        assert_eq!(ui.nested, LabNested::Settings);
    }

    #[test]
    fn completed_trial_returns_to_lobby_with_lightweight_result() {
        use crate::aim_trial::AimPersistStatus;

        let mut ui = LabUi {
            screen: LabScreen::Playing,
            nested: LabNested::None,
            selected_task: AimTaskKind::Gridshot,
            detail_overlay: true,
            last_result: None,
        };
        let mut aim = AimTrial::default();
        aim.task_kind = AimTaskKind::Gridshot;
        aim.last_persist = Some(AimPersistStatus {
            trial_id: Some("aim_000042".into()),
            score_secs: 60.0,
            hits: 42,
            shots: 46,
            accuracy: 42.0 / 46.0,
            saved_ok: true,
            error: None,
        });

        finish_trial_ui(&mut ui, &aim);

        assert_eq!(ui.screen, LabScreen::Lobby);
        assert_eq!(ui.nested, LabNested::None);
        let result = ui.last_result.as_ref().expect("result");
        assert_eq!(result.task_label, "Gridshot");
        assert_eq!((result.hits, result.shots), (42, 46));
        assert_eq!(result.saved_id.as_deref(), Some("aim_000042"));
        assert!(!result.save_failed);
        assert!(ui.detail_overlay);
    }

    #[test]
    fn failed_persist_is_exposed_as_save_failed() {
        use crate::aim_trial::AimPersistStatus;

        let mut ui = LabUi {
            screen: LabScreen::Playing,
            ..Default::default()
        };
        let mut aim = AimTrial::default();
        aim.task_kind = AimTaskKind::StaticClick;
        aim.last_persist = Some(AimPersistStatus {
            trial_id: None,
            score_secs: 1.25,
            hits: 5,
            shots: 7,
            accuracy: 5.0 / 7.0,
            saved_ok: false,
            error: Some("disk full".into()),
        });

        finish_trial_ui(&mut ui, &aim);

        let result = ui.last_result.as_ref().expect("result");
        assert_eq!(result.task_label, "Static Click");
        assert!(result.save_failed);
        assert!(result.saved_id.is_none());
    }

    #[test]
    fn settings_lock_for_armed_trial_or_running_validation() {
        assert!(settings_are_locked(AimPhase::Armed, ValidationState::Idle));
        assert!(settings_are_locked(
            AimPhase::Idle,
            ValidationState::Running
        ));
        assert!(!settings_are_locked(AimPhase::Idle, ValidationState::Idle));
    }
}

# Lab UI Shell (Lobby / Playing / Pause) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the always-on Validation Lab egui dump with Lobby → Playing → Paused screens, Esc hard-pause (gameplay + active clock freeze), V detail overlay, and Settings/Lab tools — ship as experiment `0.10.0`.

**Architecture:** New `LabUi` screen-state resource drives egui surfaces and Esc/V. Aim trials stay Armed across pause; `AimTrial` tracks pause accounting so Gridshot/STATIC_CLICK use **active** (pause-excluded) elapsed. LookCapture follows Playing vs Lobby/Paused; Esc never aborts.

**Tech Stack:** Bevy 0.19, bevy_egui, existing `AimTrial` / Gridshot / ValidationSession.

**Spec:** `docs/superpowers/specs/2026-09-19-lab-ui-shell-design.md`

## Global Constraints

- Esc: Lobby no-op; Playing → Paused; Paused → Resume. **Esc never aborts.**
- Pause freezes clock **and** look/shots/task end; no aim input/camera samples during pause; trial stays Armed.
- Resume preserves seed, live targets, counters, buffers, `start_timestamp_ns`.
- Restart = abort (no DB) + new seed + re-arm same task + Playing.
- Settings editable only when `aim.phase != Armed`; Lab Validation Start blocked while Armed.
- V = `LabUi.detail_overlay` UI preference; overlay read-only fields from `config_snapshot` while Armed.
- Persist toast only on Lobby after complete; aborts never SAVED.
- `EXPERIMENT_VERSION` / `AIM_EXPERIMENT_VERSION` → **`0.10.0`**; do **not** bump `task_version`.
- No new task types; no SQLite schema changes.

## File map

| File | Responsibility |
|------|----------------|
| `src/aim_trial.rs` | Pause fields + `active_elapsed_ns` / begin_pause / end_pause; score uses active time; clear pause on cancel |
| `src/aim_gridshot.rs` | End/finish use active elapsed (pause-aware) |
| `src/lab_ui.rs` (new) | `LabUi`, screen enums, Esc/V handlers, lightweight last result, maybe egui screen drawers |
| `src/camera_ctrl.rs` | Remove look falling-edge cancel; only drain aim while Playing (look on + not paused); pause-aware Gridshot end |
| `src/app.rs` | Init `LabUi`; replace bare Esc toggle with `handle_lab_ui_keys`; look follows `LabUi` / Playing |
| `src/validation_lab.rs` | Rewrite `draw_hud` for Lobby / Playing / Pause / Settings / Lab tools (or thin wrapper calling `lab_ui`) |
| `src/session.rs` | `EXPERIMENT_VERSION = "0.10.0"` |
| Docs | BASELINE, README, TELEMETRY_SCHEMA note `0.10.0` + pause-excluded duration |

---

### Task 1: Pause accounting (active elapsed)

**Files:**
- Modify: `src/aim_trial.rs`
- Modify: `src/aim_gridshot.rs`
- Test: unit tests in those modules

**Interfaces:**
```rust
// On AimTrial (new fields, default 0 / false / None):
pub accumulated_pause_ns: u64,
pub paused_at_qpc: Option<u64>, // Some while pause is open

/// Wall span minus completed pauses; if currently paused, freezes at paused_at.
pub fn active_elapsed_ns(trial: &AimTrial, now_ns: u64) -> u64 {
    let end = trial.paused_at_qpc.unwrap_or(now_ns);
    end.saturating_sub(trial.start_timestamp_ns)
        .saturating_sub(trial.accumulated_pause_ns)
}

pub fn begin_aim_pause(trial: &mut AimTrial, now_ns: u64) {
    if trial.phase != AimPhase::Armed || trial.paused_at_qpc.is_some() {
        return;
    }
    trial.paused_at_qpc = Some(now_ns);
}

pub fn end_aim_pause(trial: &mut AimTrial, now_ns: u64) {
    if let Some(at) = trial.paused_at_qpc.take() {
        trial.accumulated_pause_ns =
            trial.accumulated_pause_ns.saturating_add(now_ns.saturating_sub(at));
    }
}

// cancel_aim_trial / start paths: clear paused_at_qpc, accumulated_pause_ns = 0
```

Update Gridshot:
```rust
pub fn gridshot_should_end(trial: &AimTrial, now_ns: u64) -> bool {
    active_elapsed_ns(trial, now_ns) as f64 / 1e9 >= GRIDSHOT_DURATION_SECS
}

// finish_gridshot_trial: score_secs = active_elapsed_ns(trial, end_ns) / 1e9
```

Update STATIC_CLICK 5th-hit score in `apply_aim_shot` to use `active_elapsed_ns` the same way.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn active_elapsed_excludes_pause_interval() {
    let mut trial = AimTrial::default();
    trial.phase = AimPhase::Armed;
    trial.start_timestamp_ns = 1_000;
    // play 10s
    assert_eq!(active_elapsed_ns(&trial, 1_000 + 10_000_000_000), 10_000_000_000);
    begin_aim_pause(&mut trial, 1_000 + 10_000_000_000);
    // wall +30s while paused → still 10s active
    assert_eq!(active_elapsed_ns(&trial, 1_000 + 40_000_000_000), 10_000_000_000);
    end_aim_pause(&mut trial, 1_000 + 40_000_000_000);
    // +5s more play → 15s active
    assert_eq!(active_elapsed_ns(&trial, 1_000 + 45_000_000_000), 15_000_000_000);
}

#[test]
fn gridshot_end_ignores_paused_wall_time() {
    let mut trial = AimTrial::default();
    trial.phase = AimPhase::Armed;
    trial.task_kind = AimTaskKind::Gridshot;
    trial.start_timestamp_ns = 0;
    begin_aim_pause(&mut trial, 50_000_000_000); // 50s play then pause
    assert!(!gridshot_should_end(&trial, 200_000_000_000)); // huge wall, still paused at 50s
    end_aim_pause(&mut trial, 200_000_000_000);
    assert!(!gridshot_should_end(&trial, 200_000_000_000 + 9_000_000_000)); // 59s active
    assert!(gridshot_should_end(&trial, 200_000_000_000 + 10_000_000_000)); // 60s active
}
```

Update existing `gridshot_should_end(start, now)` call sites/tests to the new signature.

- [ ] **Step 2: Run tests — expect FAIL** (missing symbols / old signature)

Run: `cargo test -p sense-maxer active_elapsed -- --nocapture`
Expected: compile fail or test fail

- [ ] **Step 3: Implement fields + helpers; wire Gridshot + STATIC_CLICK score; clear on cancel/start**

- [ ] **Step 4: Run tests — expect PASS**

Run: `cargo test -p sense-maxer aim_ -- --nocapture`  
Expected: all aim/gridshot tests pass (update any tests that called old `gridshot_should_end(start, now)`)

- [ ] **Step 5: Commit**

```bash
git add src/aim_trial.rs src/aim_gridshot.rs
git commit -m "feat(aim): pause-excluded active elapsed for aim trials"
```

---

### Task 2: `LabUi` + Esc/V input (Esc never aborts)

**Files:**
- Create: `src/lab_ui.rs`
- Modify: `src/main.rs` (or module root) — `mod lab_ui;`
- Modify: `src/app.rs`
- Modify: `src/camera_ctrl.rs` — remove `on_look_disabled` falling-edge cancel; gate drain on Playing
- Modify: `src/aim_trial.rs` — keep `cancel_aim_trial`; `on_look_disabled` may become unused (delete or stop calling)

**Interfaces:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LabScreen {
    #[default]
    Lobby,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LabNested {
    #[default]
    None,
    Settings,
    LabTools,
    PauseHome, // only when screen == Paused
}

#[derive(Debug, Clone)]
pub struct LightweightResult {
    pub task_label: String, // "Gridshot" / "Static Click"
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

impl Default for LabUi { /* Lobby, None, StaticClick, detail_overlay false, last_result None */ }

/// Esc / V. Returns whether look should be enabled after handling.
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
            // Resume (even if nested Settings/LabTools)
            end_aim_pause(aim, now_ns);
            ui.nested = LabNested::None;
            ui.screen = LabScreen::Playing;
        }
    }
}

pub fn look_should_be_enabled(ui: &LabUi) -> bool {
    ui.screen == LabScreen::Playing
}
```

In `app.rs`:
```rust
.init_resource::<LabUi>()
// replace toggle_look_capture with:
fn handle_lab_ui_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<LabUi>,
    mut aim: ResMut<AimTrial>,
    mut look: ResMut<LookCapture>,
) {
    let now = sense_input_win::monotonic_now_ns();
    handle_lab_keys(&keys, &mut ui, &mut aim, now);
    look.enabled = look_should_be_enabled(&ui);
}
```

In `camera_ctrl.rs` `drain_mouse_to_camera`:
- Remove falling-edge `on_look_disabled` cancel block entirely.
- Early-return when `!look.enabled` (discard samples) **without** cancelling aim.
- Optionally take `Res<LabUi>` and also skip when `ui.screen != Playing` (belt-and-suspenders with look flag).
- Call `gridshot_should_end(&aim, sample.timestamp_ns)` (new signature).

- [ ] **Step 1: Failing tests** in `lab_ui.rs`

```rust
#[test]
fn esc_playing_pauses_without_cancel() {
    let mut ui = LabUi::default();
    ui.screen = LabScreen::Playing;
    let mut aim = AimTrial { phase: AimPhase::Armed, start_timestamp_ns: 0, ..Default::default() };
    let mut keys = ButtonInput::default();
    keys.press(KeyCode::Escape);
    // simulate just_pressed: use a small helper or call begin/end directly in unit test
    begin_aim_pause(&mut aim, 1_000);
    ui.screen = LabScreen::Paused;
    assert_eq!(aim.phase, AimPhase::Armed);
    assert!(aim.paused_at_qpc.is_some());
}

#[test]
fn esc_paused_resumes_same_seed() {
    let mut aim = AimTrial { phase: AimPhase::Armed, random_seed: 42, start_timestamp_ns: 0, ..Default::default() };
    begin_aim_pause(&mut aim, 5_000);
    end_aim_pause(&mut aim, 15_000);
    assert_eq!(aim.random_seed, 42);
    assert_eq!(aim.phase, AimPhase::Armed);
    assert!(aim.paused_at_qpc.is_none());
    assert_eq!(aim.accumulated_pause_ns, 10_000);
}

#[test]
fn v_toggles_overlay_preference_only() {
    let mut ui = LabUi::default();
    ui.screen = LabScreen::Playing;
    ui.detail_overlay = false;
    ui.detail_overlay = !ui.detail_overlay;
    assert!(ui.detail_overlay);
}
```

Also update/remove `look_falling_edge_only_not_level` test in `aim_trial.rs` — cancel-on-unlock is gone; replace with assert that `cancel_aim_trial` is only explicit.

- [ ] **Step 2: Implement `lab_ui.rs` + wire app/camera; delete cancel-on-unlock**

- [ ] **Step 3: `cargo test -p sense-maxer lab_ui` and full aim tests PASS**

- [ ] **Step 4: Commit**

```bash
git add src/lab_ui.rs src/app.rs src/camera_ctrl.rs src/aim_trial.rs src/main.rs
git commit -m "feat(app): LabUi screens and Esc pause without abort"
```

---

### Task 3: Lobby / Playing / Pause HUD surfaces

**Files:**
- Modify: `src/validation_lab.rs` (primary rewrite of `draw_hud`)
- Optionally move drawers into `src/lab_ui.rs` if `validation_lab.rs` stays readable
- Modify: start helpers to set `LabUi.screen = Playing` and `look.enabled = true` after successful start
- On Gridshot/STATIC_CLICK finish (persist path in `camera_ctrl` / finish helpers): set Lobby, fill `last_result`, clear Armed

**UI behavior (must match spec):**

**Lobby (`nested == None`):**
- Radio/select: Static Click | Gridshot → `ui.selected_task`
- Buttons: Start | Settings | Lab tools | Exit
- Start: match task → `start_aim_trial` / `start_gridshot_trial` with snapshot; on success `ui.screen = Playing`, `look.enabled = true`, `nested = None`
- Lightweight last_result + SAVED / SAVE FAILED if present
- No dense telemetry

**Lobby / Pause → Settings:**
- Existing ExperimentSettings editors (DPI, sens, HFOV, processor, rawaccel)
- `settings_locked = aim.phase == AimPhase::Armed` (also lock if validation running for processor rebuild rules as today)
- Back → Lobby None or PauseHome

**Lobby / Pause → Lab tools:**
- Start / End Validation + last validation result (existing session API)
- Start Validation disabled if `aim.phase == Armed`
- Back

**PauseHome:**
- Resume → `end_aim_pause`, Playing, look on
- Restart → `cancel_aim_trial`; re-call start for `ui.selected_task` with **new** seed; Playing
- Change trial → `cancel_aim_trial`; Lobby; nested None
- Settings / Lab tools / Exit to desktop (`AppExit` or `std::process` via Bevy `AppExit` event)

**Playing:**
- Crosshair (existing)
- Compact score from active elapsed
- If `detail_overlay`: snapshot fields + FPS/raw/yaw/pitch/last shot/DB — **not** live settings for DPI/sens/processor/HFOV/seed/version
- No Settings editors on Playing

**Finish → Lobby:**
When finish + persist sets `last_persist`, also:
```rust
ui.last_result = Some(LightweightResult { ... from trial + last_persist });
ui.screen = LabScreen::Lobby;
ui.nested = LabNested::None;
look.enabled = false;
```

Remove old always-on Validation Lab window title dump / Cancel Aim buttons (cancel only via Pause menu).

- [ ] **Step 1: Implement screen-gated `draw_hud`; wire Start/Restart/Change/Exit/finish→Lobby**

- [ ] **Step 2: Manual checklist while implementing (no automated UI test required):** Esc pause/resume; V overlay uses snapshot; settings locked while paused Armed; Start enters look

- [ ] **Step 3: `cargo test -p sense-maxer` PASS**

- [ ] **Step 4: Commit**

```bash
git add src/validation_lab.rs src/lab_ui.rs src/camera_ctrl.rs src/app.rs
git commit -m "feat(app): Lobby Playing Pause HUD shell"
```

---

### Task 4: Experiment `0.10.0` + docs stop

**Files:**
- Modify: `src/aim_trial.rs` — `AIM_EXPERIMENT_VERSION = "0.10.0"`
- Modify: `src/session.rs` — `EXPERIMENT_VERSION = "0.10.0"` (+ unit assert)
- Modify: `docs/BASELINE.md`, `README.md`, `docs/TELEMETRY_SCHEMA.md`
- Note pause-excluded active duration; Esc no longer cancels; UI shell COMPLETE / STOPPED

- [ ] **Step 1: Bump versions + docs**

- [ ] **Step 2: `cargo test -p sense-maxer` and `cargo test -p sense-telemetry` if version asserted PASS**

- [ ] **Step 3: Commit**

```bash
git add src/aim_trial.rs src/session.rs docs/BASELINE.md README.md docs/TELEMETRY_SCHEMA.md
git commit -m "docs(app): Lab UI shell experiment 0.10.0"
```

- [ ] **Step 4: STOP** — no tracking / 1wall6 in this milestone

---

## Spec coverage

| Spec item | Task |
|-----------|------|
| Esc pause/resume; never abort | 2, 3 |
| Pause freezes gameplay + samples + clock | 1, 2 |
| Resume preserves seed/targets/counters/buffers | 1, 2 |
| Restart destructive new seed | 3 |
| Change trial / Exit abort | 3 |
| Lobby Settings + Lab tools; Start → Playing + look | 3 |
| Settings lock on Armed | 3 |
| Lab tools mutual exclusion | 3 |
| V overlay from snapshot; preference on LabUi | 2, 3 |
| Persist toast on Lobby only | 3 |
| Active elapsed Gridshot/STATIC_CLICK | 1 |
| `0.10.0` + docs stop | 4 |

## Self-review notes

- No placeholders; pause helpers and `LabUi` signatures are explicit.
- `gridshot_should_end` signature change is called out for Task 1 call-site updates.
- Cancel-on-unlock removal is Task 2; HUD cancel buttons removed in Task 3.

# Aim History 3D Arena Replay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Lobby → History list + Bevy 3D arena replay of completed aim trials (camera samples, target events, shot flashes) at experiment `0.11.0`.

**Architecture:** Read-only SQLite list/load APIs; pure replay helpers (pose/targets at `t`); extend `LabScreen` with `HistoryList`/`HistoryReplay`; `AimReplay` resource + Update system drives `YawPitch` and aim spheres; egui transport HUD. No schema changes; no live mouse steering in replay.

**Tech Stack:** Bevy 0.19, bevy_egui, rusqlite via `sense-telemetry`, existing aim sphere pool.

**Spec:** `docs/superpowers/specs/2026-09-19-aim-history-replay-design.md`

## Global Constraints

- Read-only History — never insert/update/delete aim rows from History
- Camera pose = **hold** latest `aim_camera_samples` with `timestamp_ns ≤ t` (no interp in v1)
- Targets = net of `aim_target_events` with `timestamp_ns ≤ t` (spawn/despawn only; shots do not mutate occupancy)
- Esc in HistoryReplay = pause/resume **replay clock** only; Back unloads → HistoryList
- Mutual exclusion: no Start aim / Start Validation while HistoryList or HistoryReplay
- `aim_input_samples` not required for v1 drive
- `EXPERIMENT_VERSION` / `AIM_EXPERIMENT_VERSION` → **`0.11.0`** in final task; no `task_version` bump; no schema changes
- STATIC_CLICK + GRIDSHOT both supported

## File map

| File | Responsibility |
|------|----------------|
| `crates/sense-types/src/lib.rs` | `AimTrialSummary`, `AimTrialReplayBundle` (if not only in telemetry) |
| `crates/sense-telemetry/src/db.rs` (+ tests) | `list_aim_trials_summary`, `load_aim_trial_bundle` |
| `src/aim_replay.rs` (new) | Pure helpers + `AimReplay` resource + tick/apply systems |
| `src/lab_ui.rs` | `HistoryList` / `HistoryReplay` screens; Esc replay pause; look rules |
| `src/validation_lab.rs` | Lobby History button; HistoryList UI; Replay transport HUD |
| `src/camera_ctrl.rs` | Exclude HistoryReplay from live mouse→camera |
| `src/app.rs` | Register `AimReplay` + replay tick system |
| `src/session.rs` / `src/aim_trial.rs` | Version `0.11.0` (Task 4) |
| Docs | BASELINE / README / TELEMETRY |

---

### Task 1: DB list/load + pure replay helpers

**Files:**
- Modify: `crates/sense-types/src/lib.rs` (optional summary type) **or** define summary in telemetry
- Modify: `crates/sense-telemetry/src/db.rs` + tests
- Create: `src/aim_replay.rs` (pure functions + tests; resource stub OK)
- Modify: module tree (`src/main.rs` or lib modules)

**Interfaces:**
```rust
#[derive(Debug, Clone)]
pub struct AimTrialSummary {
    pub id: String,
    pub trial_type: String,
    pub hits: u32,
    pub shots: u32,
    pub accuracy: f64,
    pub score_secs: f64,
    pub experiment_version: String,
    pub end_unix_ms: i64,
}

#[derive(Debug, Clone)]
pub struct AimTrialReplayBundle {
    pub trial: AimTrialRecord,
    pub target_events: Vec<AimTargetEventRecord>, // ordered by timestamp_ns, then event_index
    pub shots: Vec<AimShotRecord>,                 // ordered by timestamp_ns / shot_index
    pub camera_samples: Vec<AimCameraSampleRecord>, // ordered by timestamp_ns
}

impl TelemetryDb {
    pub fn list_aim_trials_summary(&self, limit: usize) -> Result<Vec<AimTrialSummary>>;
    // ORDER BY end_unix_ms DESC LIMIT ?
    pub fn load_aim_trial_bundle(&self, id: &str) -> Result<AimTrialReplayBundle>;
    // Err if trial missing; children may be empty vecs but trial must exist
}

/// Absolute trial time t_ns (same domain as sample timestamps).
pub fn camera_pose_at(samples: &[AimCameraSampleRecord], t_ns: u64) -> (f64, f64) {
    // hold latest sample with timestamp_ns <= t_ns; if none, (0.0, 0.0)
}

/// Live target centers keyed by target_id after applying events with timestamp <= t_ns.
pub fn live_targets_at(
    events: &[AimTargetEventRecord],
    t_ns: u64,
) -> Vec<(String, f64, f64, f64)> {
    // spawn inserts/updates position; despawn removes; ignore other event_type for v1
}

pub fn shots_crossed(
    shots: &[AimShotRecord],
    prev_t: u64,
    t_ns: u64,
) -> Vec<&AimShotRecord> {
    // shots with prev_t < timestamp_ns <= t_ns (for flash on advance)
}
```

- [ ] **Step 1: Failing tests** — list order; load round-trip after `insert_completed_aim_trial`; `camera_pose_at` hold; `live_targets_at` spawn/despawn netting; `shots_crossed` window

- [ ] **Step 2: Implement DB + pure helpers**

- [ ] **Step 3: `cargo test -p sense-telemetry` and `cargo test -p sense-maxer aim_replay` PASS**

- [ ] **Step 4: Commit**

```bash
git add crates/sense-types crates/sense-telemetry src/aim_replay.rs src/main.rs
git commit -m "feat(telemetry): aim trial list/load and replay helpers"
```

---

### Task 2: LabUi History screens + mutual exclusion

**Files:**
- Modify: `src/lab_ui.rs`
- Modify: `src/validation_lab.rs` (Lobby History button + HistoryList panel; wire load → enter Replay)
- Modify: `src/camera_ctrl.rs` — `screen_accepts_mouse` stays false for History*
- Modify: Esc handling for HistoryReplay pause/resume of `AimReplay.playing` (not aim pause)

**Interfaces:**
```rust
// LabScreen adds:
HistoryList,
HistoryReplay,

// AimReplay (in aim_replay.rs):
pub struct AimReplay {
    pub bundle: Option<AimTrialReplayBundle>,
    pub playing: bool,
    pub speed: f32, // 1.0 | 2.0 | 4.0
    pub t_ns: u64,  // absolute QPC domain = trial timestamps
    pub load_error: Option<String>,
    // flash: Option<(hit: bool, until_ns or frames)>
}

pub fn enter_history_list(ui: &mut LabUi) { ui.screen = HistoryList; ui.nested = None; }
pub fn enter_history_replay(ui: &mut LabUi, replay: &mut AimReplay, bundle: AimTrialReplayBundle) {
    replay.bundle = Some(bundle);
    replay.playing = true;
    replay.speed = 1.0;
    replay.t_ns = bundle.trial.start_timestamp_ns;
    ui.screen = HistoryReplay;
}
pub fn leave_history_replay(ui: &mut LabUi, replay: &mut AimReplay, yaw: &mut YawPitch) {
    replay.bundle = None;
    replay.playing = false;
    yaw.yaw_deg = 0.0; yaw.pitch_deg = 0.0;
    ui.screen = HistoryList;
}

// handle_lab_keys: HistoryList Esc = no-op (or Back — prefer no-op; Back is button)
// HistoryReplay Esc = toggle replay.playing
// look_should_be_enabled: false for History*
```

HistoryList UI:
- On enter / refresh: `TelemetryDb::open(DATABASE_PATH)` → `list_aim_trials_summary(100)` (or inject via session helper)
- Rows + Replay button → `load_aim_trial_bundle` → `enter_history_replay` or show `load_error`
- Back → Lobby

Gate Lobby **History** and Start Validation / Start aim: disabled when `screen` is History* (already not Playing). Also disable History when Armed / Validating.

- [ ] **Step 1: Tests** for Esc toggles `playing` on HistoryReplay; look disabled; enter/leave resets pose fields on resource

- [ ] **Step 2: Implement screens + Lobby button**

- [ ] **Step 3: `cargo test -p sense-maxer` PASS**

- [ ] **Step 4: Commit** `feat(app): HistoryList and HistoryReplay LabUi screens`

---

### Task 3: Bevy replay tick + transport HUD + shot flash

**Files:**
- Modify: `src/aim_replay.rs` — `tick_aim_replay` system
- Modify: `src/app.rs` — register system in Update chain (after time, before or with sync_aim_target)
- Modify: `src/validation_lab.rs` — Replay HUD (play/pause, speed, scrub, readout)
- Modify: `src/aim_trial.rs` `sync_aim_target` **or** dedicated `sync_replay_targets` that writes live sphere transforms from `live_targets_at` while HistoryReplay (hide extras)

**Interfaces:**
```rust
pub fn tick_aim_replay(
    time: Res<Time>,
    mut replay: ResMut<AimReplay>,
    ui: Res<LabUi>,
    mut yaw: Single<&mut YawPitch, With<Camera3d>>,
) {
    if ui.screen != LabScreen::HistoryReplay { return; }
    let Some(bundle) = replay.bundle.as_ref() else { return; };
    let start = bundle.trial.start_timestamp_ns;
    let end = bundle.trial.end_timestamp_ns;
    if replay.playing {
        let dt_ns = (time.delta_secs_f64() * replay.speed as f64 * 1e9) as u64;
        let prev = replay.t_ns;
        replay.t_ns = (replay.t_ns.saturating_add(dt_ns)).min(end);
        // record shots_crossed(prev, t) for flash
        if replay.t_ns >= end { replay.playing = false; }
    }
    let (yaw_deg, pitch_deg) = camera_pose_at(&bundle.camera_samples, replay.t_ns);
    yaw.yaw_deg = yaw_deg as f32; // match YawPitch field types
    yaw.pitch_deg = pitch_deg as f32;
}

// Scrub: egui slider maps 0..1 → t_ns in [start, end]; sets playing=false optional
// Speed buttons set replay.speed to 1.0 / 2.0 / 4.0
```

Target sync while replaying: build `live_targets_at` → assign to existing `AimTarget` slot entities (same pool as Gridshot); hide unused slots (scale 0 or Visibility::Hidden).

Shot flash: short egui tint or world marker for ~0.15s real time when `shots_crossed` non-empty.

- [ ] **Step 1: Unit-test tick math with fake dt (pure function `advance_replay_t(playing, speed, dt_s, t, end) -> (t, playing)`)**

- [ ] **Step 2: Implement system + HUD + target sync**

- [ ] **Step 3: `cargo test -p sense-maxer` PASS**

- [ ] **Step 4: Commit** `feat(app): Bevy aim history arena replay`

- [ ] **Step 5: Manual** — complete a short STATIC_CLICK + GRIDSHOT; History → Replay; confirm camera moves, targets appear/despawn, hits flash; Esc pauses; Back to list

---

### Task 4: Experiment `0.11.0` + docs stop

**Files:**
- `src/aim_trial.rs` — `AIM_EXPERIMENT_VERSION = "0.11.0"`
- `src/session.rs` — `EXPERIMENT_VERSION = "0.11.0"`
- `docs/BASELINE.md`, `README.md`, `docs/TELEMETRY_SCHEMA.md`

- [ ] **Step 1: Bump + document History replay read-only reconstruct**

- [ ] **Step 2: Full `cargo test -p sense-maxer` (+ sense-telemetry) PASS**

- [ ] **Step 3: Commit** `docs(app): Aim History replay experiment 0.11.0`

- [ ] **Step 4: STOP** — no completeness charts / validation replay / delete

---

## Spec coverage

| Spec item | Task |
|-----------|------|
| list_aim_trials_summary / load bundle | 1 |
| camera_pose_at hold; live_targets_at; shots window | 1 |
| HistoryList / HistoryReplay screens; Esc pause replay | 2 |
| Mutual exclusion; no live look in History | 2 |
| Bevy tick, speed, scrub, target sync, shot flash | 3 |
| `0.11.0` docs stop | 4 |

## Self-review notes

- Target positions use `position_x/y/z` on events (already persisted).
- `YawPitch` field types must match existing `f32`/`f64` in `camera_ctrl` — check and cast consistently in Task 3.
- Opening DB from HUD: follow existing `persist_completed_aim_trial` / `DATABASE_PATH` pattern in `session.rs` (prefer thin wrappers `load_history_summaries()` there if cleaner than opening from egui directly).

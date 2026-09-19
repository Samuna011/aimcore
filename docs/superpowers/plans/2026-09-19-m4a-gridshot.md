# M4.a GRIDSHOT v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship timed Gridshot (3×3 exclusive cells, 3 live, 60s, hits+accuracy) on the M3.y aim telemetry runtime beside STATIC_CLICK.

**Architecture:** Keep shared Armed buffers / snapshot / persist. Introduce `AimTaskKind` (`StaticClick` | `Gridshot`) on `AimTrial`. Multi-sphere entities + grid occupancy. Closest ray–sphere hit. End on first QPC with elapsed ≥ 60s. `EXPERIMENT_VERSION` → `0.9.0`.

**Tech Stack:** Bevy 0.19, existing aim telemetry, sense-math ray–sphere (extend if needed for hit distance).

**Spec:** `docs/superpowers/specs/2026-09-19-m4a-gridshot-design.md`

## Global Constraints

- All GRIDSHOT V1 INVARIANTS from the spec (exclusive cells; 3 live except hit→respawn transition; IDs never reused; vacant-cell replacement among 6; miss unchanged; spawn/despawn only; shots own hit)
- Timer: `end` = first QPC where `(now - start) / 1e9 >= 60.0`; no post-boundary gameplay events
- Hit order same logical update: shot(hit) → despawn → spawn replacement; same `timestamp_ns`; no respawn delay
- Look reset yaw=0 pitch=0; geometry plane center `(0,1.6,-6)`, spacing 1.0, R=0.25
- Reuse M3.y five-layer persist; `trial_type = GRIDSHOT`
- Do not break STATIC_CLICK
- Stop after Gridshot — no tracking/1wall6

## File map

| File | Responsibility |
|------|----------------|
| `crates/sense-math/src/lib.rs` (+ tests) | `ray_sphere_hit_t` → `Option<f64>` closest positive t (or extend existing) |
| `src/aim_gridshot.rs` (new) | Grid cells, occupancy, start/hit/miss/timer pure logic |
| `src/aim_trial.rs` | `AimTaskKind`, multi-live state hooks, shared start/cancel/persist fields; keep STATIC_CLICK path |
| `src/camera_ctrl.rs` | Dispatch click + timer check by task kind; stop buffering after end |
| `src/app.rs` | Spawn N AimTarget spheres (or pool of 3); sync multi transforms |
| `src/validation_lab.rs` | Start Gridshot HUD + live stats |
| `src/session.rs` | `0.9.0`; persist already generic if `trial_type` set on record |
| Docs | BASELINE / README / TELEMETRY note GRIDSHOT |

---

### Task 1: Ray–sphere hit distance + grid pure logic

**Files:**
- Modify: `crates/sense-math/src/lib.rs`, `crates/sense-math/tests/math_tests.rs`
- Create: `src/aim_gridshot.rs`
- Modify: `src/main.rs` or module tree to `mod aim_gridshot`

**Interfaces:**
```rust
/// Smallest t >= 0 such that origin + t*normalize(dir) hits sphere; None if miss.
pub fn ray_sphere_hit_t(origin: [f64;3], dir: [f64;3], center: [f64;3], radius: f64) -> Option<f64>;

pub const GRIDSHOT_DURATION_SECS: f64 = 60.0;
pub const GRIDSHOT_CONCURRENT: usize = 3;
pub const GRIDSHOT_SPACING: f32 = 1.0;
pub const GRIDSHOT_DEPTH_Z: f32 = -6.0;
pub const GRIDSHOT_TASK_VERSION: &str = "1";

pub fn gridshot_cell_center(row: i32, col: i32) -> Vec3; // row,col in {-1,0,1}

pub struct GridOccupancy { /* 9 cells → Option<target_id> or live list */ }

pub fn gridshot_task_config_json() -> String;

/// Deterministic: from rng pick 3 distinct (row,col); return sorted or insertion order as drawn.
pub fn pick_initial_cells(rng: &mut u64) -> [(i32,i32); 3];

pub fn pick_vacant_cell(rng: &mut u64, occupied: &[(i32,i32)]) -> (i32,i32);
```

- [ ] **Step 1: Failing tests** — hit_t closer sphere preferred; miss None; initial cells distinct; vacant never occupied; same seed → same initial cells

- [ ] **Step 2: Implement math + gridshot helpers**

- [ ] **Step 3: `cargo test -p sense-math` and unit tests in aim_gridshot PASS**

- [ ] **Step 4: Commit** `feat(aim): gridshot grid helpers and ray hit distance`

---

### Task 2: Gridshot runtime on AimTrial (multi-live + timer + clicks)

**Files:**
- Modify: `src/aim_trial.rs`
- Modify: `src/aim_gridshot.rs` (apply_shot / start / tick_end)
- Modify: `src/camera_ctrl.rs`

**Interfaces:**
```rust
pub enum AimTaskKind { StaticClick, Gridshot }

// AimTrial adds:
pub task_kind: AimTaskKind,
pub live_targets: Vec<LiveAimTarget>, // { target_id, row, col, center: Vec3 }
// STATIC_CLICK can keep using current_center as single-element or parallel path

pub fn start_gridshot_trial(...) -> bool; // seed, snapshot, spawn 3, trial_type GRIDSHOT on build

pub fn apply_gridshot_shot(trial, pose, timestamp_ns) -> GridshotShotResult;
// Miss: log shot hit=false; Hit: shot→despawn→vacant spawn new id; check timer after?

pub fn gridshot_should_end(start_ns, now_ns) -> bool {
  (now_ns - start_ns) as f64 / 1e9 >= 60.0
}

pub fn finish_gridshot_trial(trial, end_ns) -> bool; // set score_secs=elapsed, Idle, return true for persist
```

Timer: on each Armed sample in `drain_mouse_to_camera`, if Gridshot and `gridshot_should_end`, call finish **before** processing further gameplay for that sample (or: process sample only if not ended; if this sample’s timestamp crosses boundary, finish at this timestamp without applying a shot after end). Spec: first timestamp where elapsed≥60 ends; no post-trial gameplay — if the crossing sample is a click, do **not** count the shot after end; end first.

Recommended order per sample while Gridshot Armed:
1. If `gridshot_should_end(start, sample.timestamp_ns)` → finish + persist; skip shot on this sample; stop buffering after finish (phase Idle).
2. Else buffer input/camera as today.
3. Else if LMB → apply_gridshot_shot.

STATIC_CLICK path unchanged (5th hit finish).

`build_completed_aim_trial_record` must use `trial_type` / task_config from `task_kind`.

- [ ] **Step 1: Tests** — invariants after hit; ID reuse forbidden; miss occupancy stable; timer ends at 60e9 ns; same seed initial cells; STATIC_CLICK still passes

- [ ] **Step 2: Implement + wire camera_ctrl**

- [ ] **Step 3: `cargo test -p sense-maxer` PASS**

- [ ] **Step 4: Commit** `feat(aim): GRIDSHOT runtime with exclusive cells and 60s end`

---

### Task 3: Multi-sphere render sync + HUD + 0.9.0 + docs

**Files:**
- Modify: `src/app.rs` / `aim_trial.rs` spawn — pool of 3 `AimTarget` entities (or one component `AimTargetSlot(usize)`)
- Modify: `sync_aim_target` → sync all live centers / hide extras
- Modify: `src/validation_lab.rs` — Start Gridshot button + HUD (time left, hits, accuracy, live 3)
- Modify: `src/session.rs` — `EXPERIMENT_VERSION = "0.9.0"`
- Docs: BASELINE, README, brief TELEMETRY note

- [ ] **Step 1: Visual sync + HUD**

- [ ] **Step 2: Version bump + docs stop**

- [ ] **Step 3: Full test suites PASS**

- [ ] **Step 4: Commit** `feat(app): GRIDSHOT HUD, multi-target sync, experiment 0.9.0`

- [ ] **Step 5: Manual** — play 60s run; confirm DB `trial_type=GRIDSHOT`; 3 live visually; cancel → no row; STATIC_CLICK still works

---

## Spec coverage

| Spec item | Task |
|-----------|------|
| Exclusive 3×3 / vacant replacement | 1, 2 |
| Deterministic initial 3 + never reuse IDs | 1, 2 |
| shot→despawn→spawn same timestamp | 2 |
| 60s QPC boundary | 2 |
| Closest ray hit | 1, 2 |
| Look reset 0,0 + geometry | 2, 3 |
| Telemetry GRIDSHOT + persist | 2, 3 |
| HUD + 0.9.0 + stop | 3 |
| STATIC_CLICK preserved | 2, 3 |

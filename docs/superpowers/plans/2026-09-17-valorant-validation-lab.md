# VALORANT Input Validation Lab (M1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a Windows-native Bevy Validation Lab that proves raw `WM_INPUT` → VALORANT yaw math → yaw-only FPS camera → telemetry → SQLite, then stop.

**Architecture:** Cargo workspace with pure crates (`sense-math`, `sense-types`, `sense-input-win`, `sense-telemetry`) and a Bevy binary that drains a timestamped mouse queue independently of render FPS, applies `yaw_delta = dx × sens × 0.07`, records immutable raw samples plus `InputCameraSample`, and drives an egui Validation Lab HUD.

**Tech Stack:** Rust 1.96+, Bevy `0.19.1`, `bevy_egui` (version matching Bevy 0.19), `rusqlite` (bundled), `windows` crate for `WM_INPUT` / QPC, `serde`/`serde_json` for config snapshots.

**Spec:** `docs/superpowers/specs/2026-09-17-valorant-validation-lab-design.md`

## Global Constraints

- Platform: Windows only (M1)
- Renderer: Bevy + native GPU; egui HUD in same window
- Input: Windows `WM_INPUT` raw relative; never cursor-pos deltas for the experimental stream
- Timestamps: monotonic QPC → nanoseconds for samples; wall clock only for session metadata
- Yaw: `yaw_delta_deg = raw_dx × sensitivity × 0.07`; no internal rounding
- Pitch: telemetry only; camera pitch not updated; HUD shows `PITCH MODEL: UNVERIFIED` and `PITCH ROTATION: DISABLED`
- Same eDPI ⇒ same cm/360; counts/360 differs by sensitivity
- FOV: `fov_axis = Horizontal`, `fov_degrees = 103`; angular model independent of FOV/resolution
- Raw data immutable; derived data separate; `InputCameraSample` ≠ render-frame camera pose
- Queue: never accumulate-and-discard samples
- SQLite: batched transactions; never one commit per mouse event
- No CSV/JSON export, no aim tasks, no accel curves beyond identity, no ML, no end-to-end latency claims
- Default config: DPI 1600, sens 0.175, eDPI 280, HFOV 103°, accel OFF
- Package/binary name: `sense-maxer` (workspace root folder may contain spaces; crate names use hyphens)

---

## File Structure

```
sense-maxer/                          # workspace root (this folder)
  Cargo.toml                          # workspace
  .gitignore
  README.md
  crates/
    sense-math/
      Cargo.toml
      src/lib.rs                      # degrees_per_count, edpi, counts/cm/inches per 360
      tests/math_tests.rs
    sense-types/
      Cargo.toml
      src/lib.rs                      # samples, config, session, integrity, InputProcessor
    sense-input-win/
      Cargo.toml
      src/lib.rs                      # re-exports
      src/clock.rs                    # QPC monotonic ns
      src/queue.rs                    # MouseQueue
      src/integrity.rs                # gap/dup/ooo/regression counters
      src/wm_input.rs                 # RegisterRawInputDevices + WM_INPUT
    sense-telemetry/
      Cargo.toml
      src/lib.rs
      src/db.rs                       # schema + batched writer
      src/buffers.rs                  # in-memory session buffers
  src/
    main.rs
    app.rs                            # Bevy App builder
    config.rs                         # default ExperimentalConfig resource
    scene.rs                          # floor, light, placeholder mesh, crosshair
    camera_ctrl.rs                    # yaw-only apply from drained samples
    fov.rs                            # HFOV → perspective projection
    input_plugin.rs                   # bridge WM_INPUT queue into Bevy
    validation_lab.rs                 # egui HUD + Start/End/Reset flow
    frame_telemetry.rs                # FrameSample recording
  docs/
    ARCHITECTURE.md
    VALORANT_INPUT_MODEL.md
    TELEMETRY_SCHEMA.md
    EXPERIMENT_MODEL.md
  data/                               # runtime SQLite (gitignored)
```

---

### Task 1: Workspace scaffold + architecture docs

**Files:**
- Create: `Cargo.toml`, `.gitignore`, `README.md`
- Create: `crates/sense-math/Cargo.toml`, `crates/sense-math/src/lib.rs` (empty `pub fn stub()` ok)
- Create: `crates/sense-types/Cargo.toml`, `crates/sense-types/src/lib.rs` (empty)
- Create: `crates/sense-input-win/Cargo.toml`, `crates/sense-input-win/src/lib.rs` (empty)
- Create: `crates/sense-telemetry/Cargo.toml`, `crates/sense-telemetry/src/lib.rs` (empty)
- Create: `src/main.rs` (placeholder `fn main() { println!("sense-maxer scaffold"); }`)
- Create: `docs/ARCHITECTURE.md`, `docs/VALORANT_INPUT_MODEL.md`, `docs/TELEMETRY_SCHEMA.md`, `docs/EXPERIMENT_MODEL.md`

**Interfaces:**
- Consumes: approved design spec
- Produces: compilable workspace; docs describing M1 architecture before feature code

- [ ] **Step 1: Write root workspace `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = [
    ".",
    "crates/sense-math",
    "crates/sense-types",
    "crates/sense-input-win",
    "crates/sense-telemetry",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"

[workspace.dependencies]
sense-math = { path = "crates/sense-math" }
sense-types = { path = "crates/sense-types" }
sense-input-win = { path = "crates/sense-input-win" }
sense-telemetry = { path = "crates/sense-telemetry" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[package]
name = "sense-maxer"
version.workspace = true
edition.workspace = true

[dependencies]
sense-math = { workspace = true }
sense-types = { workspace = true }
sense-input-win = { workspace = true }
sense-telemetry = { workspace = true }
```

- [ ] **Step 2: Add `.gitignore`**

```
/target
/data
**/*.db
.DS_Store
```

- [ ] **Step 3: Create each crate `Cargo.toml` with `name`, `version.workspace`, `edition.workspace`, and empty `src/lib.rs` (`//! crate docs`). Root `src/main.rs` prints scaffold message.**

- [ ] **Step 4: Write `docs/ARCHITECTURE.md`** covering: Bevy process diagram; why `sense-math` is pure; why `WM_INPUT` not cursor; queue vs render FPS; `InputCameraSample` vs future render camera sample; M1 pitch disabled; SQLite batching; what is out of scope.

- [ ] **Step 5: Write `docs/VALORANT_INPUT_MODEL.md`** with formulas, units, same-eDPI table (cm/360 shared, counts/360 differs), and provenance section for `0.07` marked **UNCERTAIN** / community-derived until empirically confirmed against VALORANT (source, date checked, claim, confidence). State M1 does not silently compensate.

- [ ] **Step 6: Write `docs/TELEMETRY_SCHEMA.md`** for tables: `configurations`, `sessions`, `raw_mouse_events`, `input_camera_samples`, `frame_samples`, `validation_results` + integrity fields; raw vs derived; future tables listed as non-M1.

- [ ] **Step 7: Write `docs/EXPERIMENT_MODEL.md`** for Validation Lab flow, session/config IDs, experiment_id=`validation_lab`, experiment_version=`0.1.0`, signed net counts rule, integrity → pipeline-suspect.

- [ ] **Step 8: Verify build**

Run: `cargo build`
Expected: success (placeholder binary)

- [ ] **Step 9: Commit** (only if user asked for commits in this session; otherwise skip)

```bash
git add Cargo.toml .gitignore README.md crates src docs
git commit -m "chore: scaffold workspace and M1 architecture docs"
```

---

### Task 2: `sense-math` with TDD

**Files:**
- Create: `crates/sense-math/src/lib.rs`
- Create: `crates/sense-math/tests/math_tests.rs`
- Modify: `crates/sense-math/Cargo.toml` (no extra deps)

**Interfaces:**
- Consumes: none
- Produces:

```rust
pub const VALORANT_YAW_DEG_PER_COUNT_AT_SENS_1: f64 = 0.07;

pub fn degrees_per_count(sensitivity: f64) -> f64;
pub fn yaw_delta_deg(dx_counts: f64, sensitivity: f64) -> f64;
pub fn edpi(dpi: f64, sensitivity: f64) -> f64;
pub fn counts_per_360(sensitivity: f64) -> f64;
pub fn inches_per_360(dpi: f64, sensitivity: f64) -> f64;
pub fn cm_per_360(dpi: f64, sensitivity: f64) -> f64;
```

- [ ] **Step 1: Write failing tests** in `crates/sense-math/tests/math_tests.rs`

```rust
use sense_math::*;

#[test]
fn degrees_per_count_examples() {
    assert_eq!(degrees_per_count(0.15), 0.15 * 0.07);
    assert_eq!(degrees_per_count(0.175), 0.175 * 0.07);
    assert_eq!(degrees_per_count(0.20), 0.20 * 0.07);
}

#[test]
fn yaw_delta_1000_counts_at_0_175() {
    assert_eq!(yaw_delta_deg(1000.0, 0.175), 1000.0 * 0.175 * 0.07);
}

#[test]
fn same_edpi_same_cm_per_360_different_counts() {
    let a = (800.0, 0.35);
    let b = (1600.0, 0.175);
    let c = (3200.0, 0.0875);
    assert_eq!(edpi(a.0, a.1), 280.0);
    assert_eq!(edpi(b.0, b.1), 280.0);
    assert_eq!(edpi(c.0, c.1), 280.0);
    assert_eq!(cm_per_360(a.0, a.1), cm_per_360(b.0, b.1));
    assert_eq!(cm_per_360(b.0, b.1), cm_per_360(c.0, c.1));
    assert_ne!(counts_per_360(a.1), counts_per_360(b.1));
    assert_ne!(counts_per_360(b.1), counts_per_360(c.1));
    assert_eq!(counts_per_360(0.175), 360.0 / (0.175 * 0.07));
}

#[test]
fn angular_model_independent_of_fov_and_resolution_params() {
    // Pure math API takes no fov/resolution; this documents the contract:
    // callers must not pass fov/resolution into these functions.
    let sens = 0.175;
    let dpi = 1600.0;
    let _fov_a = 103.0_f64;
    let _fov_b = 90.0_f64;
    let _res_a = (1920.0, 1080.0);
    let _res_b = (1280.0, 960.0);
    assert_eq!(degrees_per_count(sens), degrees_per_count(sens));
    assert_eq!(counts_per_360(sens), counts_per_360(sens));
    assert_eq!(cm_per_360(dpi, sens), cm_per_360(dpi, sens));
}

#[test]
fn inches_and_cm_relationship() {
    let dpi = 1600.0;
    let sens = 0.175;
    assert_eq!(cm_per_360(dpi, sens), inches_per_360(dpi, sens) * 2.54);
}
```

- [ ] **Step 2: Run tests — expect FAIL**

Run: `cargo test -p sense-math`
Expected: compile failure / undefined functions

- [ ] **Step 3: Implement `crates/sense-math/src/lib.rs`**

```rust
//! VALORANT-profile angular sensitivity math. No rounding. No Bevy.

pub const VALORANT_YAW_DEG_PER_COUNT_AT_SENS_1: f64 = 0.07;

pub fn degrees_per_count(sensitivity: f64) -> f64 {
    sensitivity * VALORANT_YAW_DEG_PER_COUNT_AT_SENS_1
}

pub fn yaw_delta_deg(dx_counts: f64, sensitivity: f64) -> f64 {
    dx_counts * degrees_per_count(sensitivity)
}

pub fn edpi(dpi: f64, sensitivity: f64) -> f64 {
    dpi * sensitivity
}

pub fn counts_per_360(sensitivity: f64) -> f64 {
    360.0 / degrees_per_count(sensitivity)
}

pub fn inches_per_360(dpi: f64, sensitivity: f64) -> f64 {
    counts_per_360(sensitivity) / dpi
}

pub fn cm_per_360(dpi: f64, sensitivity: f64) -> f64 {
    inches_per_360(dpi, sensitivity) * 2.54
}
```

- [ ] **Step 4: Run tests — expect PASS**

Run: `cargo test -p sense-math`
Expected: all tests pass

- [ ] **Step 5: Commit** (if committing)

```bash
git add crates/sense-math
git commit -m "feat(sense-math): VALORANT yaw and eDPI formulas with tests"
```

---

### Task 3: `sense-types` domain types + `NoAcceleration`

**Files:**
- Create/Modify: `crates/sense-types/src/lib.rs`
- Modify: `crates/sense-types/Cargo.toml` — add `serde`, `sense-math`
- Create: `crates/sense-types/tests/types_smoke.rs` (construct + serialize roundtrip)

**Interfaces:**
- Consumes: `sense_math::edpi`, `VALORANT_YAW_DEG_PER_COUNT_AT_SENS_1`
- Produces: types below + `InputProcessor` / `NoAcceleration`

```rust
pub struct MouseSample {
    pub timestamp_ns: u64,
    pub dx: i32,
    pub dy: i32,
    pub buttons: u32,
    pub sequence_number: u64,
}

/// Camera state immediately after applying one raw mouse sample (not render-frame).
pub struct InputCameraSample {
    pub timestamp_ns: u64,
    pub yaw_deg: f64,
    pub pitch_deg: f64,
    pub sequence_number: u64,
}

pub struct FrameSample {
    pub timestamp_ns: u64,
    pub frame_time_s: f64,
    pub fps: f64,
}

pub enum FovAxis { Horizontal }

pub struct SensitivityConfig {
    pub dpi: f64,
    pub sensitivity: f64,
    pub yaw_deg_per_count_at_sens_1: f64, // 0.07
    pub fov_axis: FovAxis,
    pub fov_degrees: f64, // 103 horizontal
}

pub struct AccelerationConfig {
    pub enabled: bool,
    pub model: String, // "none" for M1
}

pub struct DisplayConfig {
    pub width: u32,
    pub height: u32,
    pub refresh_hz: f64,
}

pub struct ConfigurationRecord { /* id, dpi, sens, edpi, fov, display, accel, polling_rate_hz: Option<f64> */ }
pub struct SessionRecord { /* id, app_version, experiment_id, experiment_version, config_id, seed, start/end wall times */ }

pub struct InputIntegrityReport {
    pub samples_received: u64,
    pub sequence_gaps: u64,
    pub duplicate_sequences: u64,
    pub out_of_order_samples: u64,
    pub timestamp_regressions: u64,
}

impl InputIntegrityReport {
    pub fn is_pipeline_suspect(&self) -> bool { /* any non-zero integrity fault */ }
}

pub struct ValidationResult {
    pub expected_counts: f64,
    pub observed_net_counts: f64,      // signed Σ dx
    pub observed_abs_path_counts: f64, // Σ |dx| (telemetry only)
    pub expected_degrees: f64,
    pub observed_degrees: f64,
    pub count_difference: f64,
    pub error_percent: f64,
    pub integrity: InputIntegrityReport,
}

pub trait InputProcessor {
    fn process(&mut self, dx: f64, dy: f64, dt_s: f64) -> (f64, f64);
}

pub struct NoAcceleration;
impl InputProcessor for NoAcceleration {
    fn process(&mut self, dx: f64, dy: f64, _dt_s: f64) -> (f64, f64) { (dx, dy) }
}
```

- [ ] **Step 1: Write failing smoke test** that constructs `MouseSample`, `InputCameraSample`, `ValidationResult`, and asserts `NoAcceleration` identity.

- [ ] **Step 2: Run `cargo test -p sense-types` — expect FAIL**

- [ ] **Step 3: Implement types in `lib.rs` with `#[derive(Debug, Clone, Serialize, Deserialize)]` where appropriate. Do not use `any`/untyped maps for core telemetry.**

- [ ] **Step 4: `cargo test -p sense-types` — expect PASS**

- [ ] **Step 5: Commit** (if committing)

```bash
git add crates/sense-types
git commit -m "feat(sense-types): telemetry and config domain types"
```

---

### Task 4: `sense-input-win` — clock, queue, integrity, WM_INPUT

**Files:**
- Create: `crates/sense-input-win/src/{lib.rs,clock.rs,queue.rs,integrity.rs,wm_input.rs}`
- Modify: `crates/sense-input-win/Cargo.toml`:

```toml
[dependencies]
sense-types = { workspace = true }
windows = { version = "0.58", features = [
  "Win32_Foundation",
  "Win32_UI_Input",
  "Win32_UI_WindowsAndMessaging",
  "Win32_System_Performance",
] }
```

**Interfaces:**
- Consumes: `sense_types::MouseSample`, `InputIntegrityReport`
- Produces:

```rust
pub fn monotonic_now_ns() -> u64;

pub struct MouseQueue { /* Mutex<VecDeque<MouseSample>> + AtomicU64 sequence */ }
impl MouseQueue {
    pub fn new() -> Self;
    pub fn push_raw(&self, dx: i32, dy: i32, buttons: u32, timestamp_ns: u64);
    pub fn drain_all(&self) -> Vec<MouseSample>; // preserves order; empties queue
}

pub struct IntegrityTracker { /* last_seq, last_ts, counters */ }
impl IntegrityTracker {
    pub fn observe(&mut self, sample: &MouseSample);
    pub fn report(&self) -> InputIntegrityReport;
    pub fn reset(&mut self);
}

/// Installs raw input on `hwnd`. Pushes to shared `Arc<MouseQueue>`.
pub fn register_raw_mouse(hwnd: isize, queue: Arc<MouseQueue>) -> Result<(), String>;
/// Call from window proc / message hook when `msg == WM_INPUT`.
pub fn handle_wm_input(lparam: isize, queue: &MouseQueue, tracker: &Mutex<IntegrityTracker>);
```

- [ ] **Step 1: Write failing unit tests** in `crates/sense-input-win/tests/queue_integrity.rs`:

```rust
#[test]
fn drain_preserves_every_sample_order() { /* push 3; drain; len 3; order match; second drain empty */ }

#[test]
fn integrity_detects_gap_duplicate_ooo_regression() {
    // observe seq 1,3 → gap; seq 3 again → duplicate; seq 2 → ooo; ts decreasing → regression
}
```

- [ ] **Step 2: `cargo test -p sense-input-win` — expect FAIL**

- [ ] **Step 3: Implement `clock.rs`** using `QueryPerformanceCounter` / `QueryPerformanceFrequency` → ns (`count * 1_000_000_000 / freq`).

- [ ] **Step 4: Implement `queue.rs` and `integrity.rs` to pass tests. `push_raw` assigns `sequence_number` via atomic fetch_add(1). Never coalesce dx/dy.**

- [ ] **Step 5: Implement `wm_input.rs`**: `RegisterRawInputDevices` for `RIM_TYPEMOUSE` with `RIDEV_INPUTSINK` optional only if needed for focus; prefer foreground raw input with cursor captured by Bevy. Parse `RAWMOUSE.lLastX/lLastY` as dx/dy. On each event call `monotonic_now_ns()`, `queue.push_raw`, and integrity observe.

- [ ] **Step 6: Document in crate docs that Bevy mouse motion events must not be used as the experimental stream.**

- [ ] **Step 7: `cargo test -p sense-input-win` — expect PASS**

- [ ] **Step 8: Commit** (if committing)

```bash
git add crates/sense-input-win
git commit -m "feat(sense-input-win): WM_INPUT queue, QPC clock, integrity"
```

---

### Task 5: `sense-telemetry` — SQLite schema + batched writer

**Files:**
- Create: `crates/sense-telemetry/src/{lib.rs,db.rs,buffers.rs}`
- Modify: `crates/sense-telemetry/Cargo.toml` — `rusqlite = { version = "0.32", features = ["bundled"] }`, `sense-types`, `serde_json`
- Create: `crates/sense-telemetry/tests/db_roundtrip.rs`

**Interfaces:**
- Consumes: types from `sense-types`
- Produces:

```rust
pub struct TelemetryDb { /* Connection */ }
impl TelemetryDb {
    pub fn open(path: &Path) -> Result<Self, String>;
    pub fn migrate(&self) -> Result<(), String>;
    pub fn upsert_configuration(&self, cfg: &ConfigurationRecord) -> Result<(), String>;
    pub fn insert_session_start(&self, session: &SessionRecord) -> Result<(), String>;
    pub fn end_session(&self, session_id: &str, end_unix_ms: i64) -> Result<(), String>;
    pub fn insert_validation_result(&self, session_id: &str, result: &ValidationResult) -> Result<(), String>;
    /// Single transaction: batch insert raw mouse + input camera + frame samples.
    pub fn flush_buffers(&self, session_id: &str, buffers: &SessionBuffers) -> Result<(), String>;
}

pub struct SessionBuffers {
    pub mouse: Vec<MouseSample>,
    pub input_camera: Vec<InputCameraSample>,
    pub frames: Vec<FrameSample>,
}
impl SessionBuffers {
    pub fn clear(&mut self);
}
```

SQL (create in `migrate`):

```sql
CREATE TABLE configurations (
  id TEXT PRIMARY KEY,
  dpi REAL NOT NULL,
  sensitivity REAL NOT NULL,
  edpi REAL NOT NULL,
  yaw_deg_per_count_at_sens_1 REAL NOT NULL,
  fov_axis TEXT NOT NULL,
  fov_degrees REAL NOT NULL,
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  refresh_hz REAL NOT NULL,
  acceleration_enabled INTEGER NOT NULL,
  acceleration_model TEXT NOT NULL,
  polling_rate_hz REAL,
  snapshot_json TEXT NOT NULL
);

CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  configuration_id TEXT NOT NULL,
  app_version TEXT NOT NULL,
  experiment_id TEXT NOT NULL,
  experiment_version TEXT NOT NULL,
  random_seed INTEGER NOT NULL,
  start_unix_ms INTEGER NOT NULL,
  end_unix_ms INTEGER,
  FOREIGN KEY(configuration_id) REFERENCES configurations(id)
);

CREATE TABLE raw_mouse_events (
  session_id TEXT NOT NULL,
  sequence_number INTEGER NOT NULL,
  timestamp_ns INTEGER NOT NULL,
  dx INTEGER NOT NULL,
  dy INTEGER NOT NULL,
  buttons INTEGER NOT NULL,
  PRIMARY KEY(session_id, sequence_number)
);

CREATE TABLE input_camera_samples (
  session_id TEXT NOT NULL,
  sequence_number INTEGER NOT NULL,
  timestamp_ns INTEGER NOT NULL,
  yaw_deg REAL NOT NULL,
  pitch_deg REAL NOT NULL,
  PRIMARY KEY(session_id, sequence_number)
);

CREATE TABLE frame_samples (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  timestamp_ns INTEGER NOT NULL,
  frame_time_s REAL NOT NULL,
  fps REAL NOT NULL
);

CREATE TABLE validation_results (
  session_id TEXT PRIMARY KEY,
  expected_counts REAL NOT NULL,
  observed_net_counts REAL NOT NULL,
  observed_abs_path_counts REAL NOT NULL,
  expected_degrees REAL NOT NULL,
  observed_degrees REAL NOT NULL,
  count_difference REAL NOT NULL,
  error_percent REAL NOT NULL,
  samples_received INTEGER NOT NULL,
  sequence_gaps INTEGER NOT NULL,
  duplicate_sequences INTEGER NOT NULL,
  out_of_order_samples INTEGER NOT NULL,
  timestamp_regressions INTEGER NOT NULL,
  pipeline_suspect INTEGER NOT NULL
);

CREATE INDEX idx_raw_mouse_session ON raw_mouse_events(session_id);
CREATE INDEX idx_input_cam_session ON input_camera_samples(session_id);
CREATE INDEX idx_frame_session ON frame_samples(session_id);
```

- [ ] **Step 1: Write failing roundtrip test**: open `:memory:` DB, migrate, insert config+session, push 2 mouse + 2 input camera + 1 frame, `flush_buffers`, insert `ValidationResult`, query counts == 2/2/1.

- [ ] **Step 2: `cargo test -p sense-telemetry` — expect FAIL**

- [ ] **Step 3: Implement migrate + batched + flush in one transaction with prepared statements.**

- [ ] **Step 4: Tests PASS**

- [ ] **Step 5: Commit** (if committing)

```bash
git add crates/sense-telemetry
git commit -m "feat(sense-telemetry): SQLite schema and batched flush"
```

---

### Task 6: Bevy app — scene, HFOV camera, egui shell (no WM_INPUT yet)

**Files:**
- Modify: root `Cargo.toml` dependencies:

```toml
bevy = "0.19.1"
bevy_egui = "0.35"  # if incompatible, pin the crates.io bevy_egui release that declares bevy 0.19
```

(Verify `bevy_egui` compatibility at implement time; adjust version to the latest that supports Bevy 0.19.)

- Create: `src/{main.rs,app.rs,config.rs,scene.rs,fov.rs,validation_lab.rs}` (HUD stub)
- Modify: docs if FOV projection formula needs a short section in `ARCHITECTURE.md`

**Interfaces:**
- Consumes: `sense_math`, `sense_types::SensitivityConfig`
- Produces: Bevy `App` that opens a window, draws floor + crosshair, applies horizontal FOV 103°

```rust
pub fn vertical_fov_radians(horizontal_fov_deg: f64, aspect: f64) -> f32 {
    // vfov = 2 * atan(tan(hfov/2) / aspect)  where aspect = width/height
}
```

Default resource:

```rust
pub struct ExperimentSettings {
    pub dpi: f64,            // 1600
    pub sensitivity: f64,    // 0.175
    pub fov_degrees_h: f64,  // 103
}
```

- [ ] **Step 1: Implement `fov.rs` unit test** (can live in `src/fov.rs` with `#[cfg(test)]`): aspect 16/9 vs 4/3 change vfov; hfov constant.

- [ ] **Step 2: Implement minimal Bevy app**: `Camera3d` + `Transform` at eye height; gray floor plane; dim clear color; simple unlit cube far ahead as visual reference; egui panel showing static text `PITCH MODEL: UNVERIFIED` / `PITCH ROTATION: DISABLED` and config numbers from `sense_math`.

- [ ] **Step 3: Disable Bevy's default cursor motion → camera** (do not use `MouseMotion` for yaw). Capture cursor when focused.

- [ ] **Step 4: Run `cargo run` manually — window opens, HUD visible, look stays fixed when moving mouse (until Task 7).**

- [ ] **Step 5: Commit** (if committing)

```bash
git add src Cargo.toml Cargo.lock docs
git commit -m "feat(app): Bevy scene, HFOV projection, egui shell"
```

---

### Task 7: Wire WM_INPUT → drain → yaw-only camera + live HUD + frame telemetry

**Files:**
- Create: `src/input_plugin.rs`, `src/camera_ctrl.rs`, `src/frame_telemetry.rs`
- Modify: `src/app.rs`, `src/validation_lab.rs`, `src/config.rs`

**Interfaces:**
- Consumes: `MouseQueue::drain_all`, `sense_math::yaw_delta_deg`, `NoAcceleration`, `SessionBuffers`
- Produces: systems:

```rust
fn drain_mouse_to_camera(
  queue: Res<ArcMouseQueue>,
  mut cam: Query<&mut YawPitch>,
  settings: Res<ExperimentSettings>,
  mut processor: ResMut<NoAcceleration>,
  mut live: ResMut<LiveInputStats>,
  mut buffers: ResMut<SessionBuffers>,
  validating: Res<ValidationState>,
);
```

`YawPitch { yaw_deg: f64, pitch_deg: f64 }` — apply only yaw; ignore processed dy for camera; still record dy in `MouseSample` and live stats.

`LiveInputStats`: last_dx, last_dy, net_dx, net_dy, abs_dx, total_yaw_delta_deg, samples_this_frame.

Each drained sample while session active:

1. Optional `processor.process(dx, dy, dt)` (identity)
2. Push immutable `MouseSample` to buffer
3. `yaw += yaw_delta_deg(dx as f64, sensitivity)`
4. Push `InputCameraSample { timestamp_ns, yaw_deg, pitch_deg, sequence_number }`
5. Update live counters (signed net + abs path)

Render system: set `Transform` rotation from yaw only (Y-up).

Frame system: each frame push `FrameSample` using QPC and `Time::delta_secs`, compute fps = 1/dt.

egui live fields from `LiveInputStats` + `sense_math` derived cm/360, counts/360, eDPI.

- [ ] **Step 1: Obtain HWND from Bevy window** (winit raw handle) after window creation; call `register_raw_mouse`; store `Arc<MouseQueue>` as resource. Route `WM_INPUT` via winit/`windows` subclass or Bevy message hook — implement the approach that works on Bevy 0.19/winit without using cursor deltas. If subclassing is required, document the hook point in `ARCHITECTURE.md`.

- [ ] **Step 2: Implement drain system in `Update` (or fixed input schedule)** that always drains **all** samples; never folds into one delta.

- [ ] **Step 3: Update egui to show last dx/dy, nets, yaw, pitch (frozen), pitch banners, FPS/frame time separately.

- [ ] **Step 4: Manual check: move mouse → yaw rotates; move vertically → dy updates in HUD, pitch angle unchanged.**

- [ ] **Step 5: `cargo test` (workspace) still green; `cargo run` works.

- [ ] **Step 6: Commit** (if committing)

```bash
git add src docs crates
git commit -m "feat(app): WM_INPUT drain to yaw camera and live HUD"
```

---

### Task 8: Validation Lab session lifecycle + SQLite persistence + integrity report

**Files:**
- Modify: `src/validation_lab.rs`, `src/app.rs`, `src/config.rs`
- Create: `src/session.rs` (ID generation, start/end)

**Interfaces:**
- Consumes: `TelemetryDb`, `ValidationResult`, `InputIntegrityReport`, `sense_math::*`
- Produces: buttons wired as specified

**ID helpers:**

```rust
pub fn next_session_id(date: &str, seq: u32) -> String; // session_YYYYMMDD_000001
pub fn next_config_id(seq: u32) -> String;              // config_000001
```

**Start Validation:**

1. Snapshot `ConfigurationRecord` (dpi, sens, edpi via `sense_math::edpi`, fov, window size, refresh if available, accel off, app version `0.1.0`, experiment `validation_lab`/`0.1.0`, seed `0`)
2. Insert config + session start into SQLite (`data/sense_maxer.db`)
3. Reset integrity tracker; clear buffers; optionally reset counters
4. Set `ValidationState::Running`

**Reset Camera:** `yaw_deg = 0.0` (pitch unchanged)

**Reset Counters:** net/abs totals = 0; do not delete persisted rows

**End Validation:**

1. `expected_counts = counts_per_360(sens)`
2. `observed_net_counts = live.net_dx as f64`
3. `observed_abs_path_counts = live.abs_dx as f64`
4. `expected_degrees = 360.0`
5. `observed_degrees = yaw_delta_deg(observed_net_counts, sens)` (or equivalently net * deg/count)
6. `count_difference = observed_net_counts - expected_counts`
7. `error_percent = (count_difference / expected_counts) * 100.0`
8. Attach `integrity.report()`; `pipeline_suspect = integrity.is_pipeline_suspect()`
9. `flush_buffers` + `insert_validation_result` + `end_session`
10. Show results in egui; **never** adjust yaw constant based on error

HUD instruction text: user must perform one continuous horizontal 360° in a single direction without reversing.

- [ ] **Step 1: Implement session/config ID + Start/End/Reset button handlers.**

- [ ] **Step 2: On End, display expected/observed counts & degrees, difference, error %, integrity counters, pipeline-suspect flag.**

- [ ] **Step 3: Manual 360° run; confirm SQLite rows exist (`sqlite3 data/sense_maxer.db ".tables"` / select counts).**

- [ ] **Step 4: Commit** (if committing)

```bash
git add src
git commit -m "feat(validation): session lifecycle, discrepancy and integrity report"
```

---

### Task 9: Final docs sync + M1 stop checklist

**Files:**
- Modify: `docs/ARCHITECTURE.md`, `VALORANT_INPUT_MODEL.md`, `TELEMETRY_SCHEMA.md`, `EXPERIMENT_MODEL.md` to match final code (HWND hook method, exact crate versions, DB path)
- Create: `docs/M1_COMPLETION_REPORT.md` template filled with known values; leave 360° empirical results section for the operator to paste after first run
- Modify: `README.md` with build/run/test instructions

- [ ] **Step 1: Run full verification**

```bash
cargo test --workspace
cargo run --release
```

Expected: all tests pass; Validation Lab usable.

- [ ] **Step 2: Fill M1 completion report sections 1–10 and 12–14 from implementation; section 11 (360° results) marked “operator to complete on first hardware run”.**

- [ ] **Step 3: STOP. Do not start STATIC_CLICK or later phases.**

- [ ] **Step 4: Commit** (if committing)

```bash
git add docs README.md
git commit -m "docs: sync M1 architecture and completion report"
```

---

## Spec Coverage Checklist

| Spec requirement | Task |
|------------------|------|
| Bevy + egui + native 3D | 6–7 |
| WM_INPUT + QPC + queue every sample | 4, 7 |
| sense-math formulas + same-eDPI tests | 2 |
| Pitch disabled + HUD banners | 6–7 |
| HFOV 103 + derived VFOV | 6 |
| Angular model ≠ FOV/resolution | 2, docs |
| InputCameraSample naming/semantics | 3, 5, 7 |
| FrameSample separate | 5, 7 |
| SQLite batching + M1 tables | 5, 8 |
| Validation Lab flow + signed net counts | 8 |
| Input integrity counters | 4, 8 |
| Docs set | 1, 9 |
| Stop after M1 | 9 |
| No export / tasks / ML / pitch / latency claims | Global + Task 9 |

## Placeholder / consistency review

- No TBD steps; `bevy_egui` version verified at Task 6 implement time against Bevy 0.19.1.
- Type names consistent: `InputCameraSample`, `input_camera_samples`, `ValidationResult`, `InputIntegrityReport`.
- Commits optional per user standing rule — execute only when user requests git commits.

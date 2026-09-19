# M3.y Aim Telemetry Data Model Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Evolve STATIC_CLICK persistence into the five-layer reconstructable telemetry model (immutable trial snapshot + target_events + shots + input_samples + camera_samples), completed-only atomic flush, then stop.

**Architecture:** Extend `aim_trials` / `aim_shots`; add `aim_target_events`, `aim_input_samples`, `aim_camera_samples`. Buffer all streams while Armed; flush in one txn on 5th hit. Deterministic `random_seed` at Start. Raw `dt_ns` vs clamped `dt_used_ns` both persisted. Derived ML metrics out of scope.

**Tech Stack:** rusqlite, sense-types, sense-telemetry, sense-accel `LinearEval` / `last_linear_eval`, Bevy aim trial + `drain_mouse_to_camera`.

**Spec:** `docs/superpowers/specs/2026-09-19-m3y-aim-telemetry-data-model-design.md`

## Global Constraints

- Raw telemetry permanent; derived metrics (overshoot/jitter/RMS/…) **not** primary columns
- `aim_target_events` = lifecycle only (`spawn`/`despawn`/`direction_change`/`appear`/`disappear`) — **no** authoritative hit/miss
- `aim_shots.hit` = authoritative click outcome; add `target_id`
- `dt_ns` = raw QPC interval; `dt_used_ns` = after `clamp_speed_dt_ms`; `input_speed` uses `dt_used_ns`; never overwrite `dt_ns` with clamp
- Completed-only atomic insert of **all** five layers; abort discards buffers
- Trial id: `aim_{date}_{seq:06}` max-suffix+1 inside txn (not `COUNT(*)+1`)
- `EXPERIMENT_VERSION` → `0.8.0`
- No new task gameplay; stop after docs + tests + manual DB note
- Validation Lab session tables unchanged (aim uses `aim_*` streams)

## File map

| File | Responsibility |
|------|----------------|
| `crates/sense-types/src/lib.rs` | Expand records; new event/sample types |
| `crates/sense-telemetry/src/db.rs` | Migrate ALTER + CREATE; expand insert API |
| `crates/sense-telemetry/tests/db_roundtrip.rs` | Five-table round-trip + rollback |
| `src/aim_trial.rs` | Seed RNG, target_id, target_events buffer, clear all buffers on cancel |
| `src/camera_ctrl.rs` | While Armed: push input + camera samples; keep finish→persist |
| `src/session.rs` | Snapshot new trial fields; pass all buffers to insert; `0.8.0` |
| `src/validation_lab.rs` | Pass resolution/aspect into persist if needed; Start seed already via aim |
| `docs/BASELINE.md`, `TELEMETRY_SCHEMA.md`, `README.md` | 0.8.0 + M3.y stop |

---

### Task 1: Types + migrate + expanded insert

**Files:**
- Modify: `crates/sense-types/src/lib.rs`
- Modify: `crates/sense-telemetry/src/db.rs`
- Modify: `crates/sense-telemetry/tests/db_roundtrip.rs`

**Interfaces:**
```rust
// AimTrialRecord — ADD fields (keep existing):
pub pitch_model_id: String,
pub pitch_model_version: String,
pub pitch_config_json: String,
pub resolution_width: u32,
pub resolution_height: u32,
pub aspect_ratio: f64,
pub random_seed: u64,
pub task_version: String,
pub hardware_config_json: String,
pub view_config_json: String,

// AimShotRecord — ADD:
pub target_id: String,

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AimTargetEventRecord {
    pub target_id: String,
    pub event_index: u32,
    pub timestamp_ns: u64,
    pub event_type: String, // spawn|despawn|direction_change|appear|disappear
    pub position_x: f64,
    pub position_y: f64,
    pub position_z: f64,
    pub yaw_deg: Option<f64>,
    pub pitch_deg: Option<f64>,
    pub velocity_x: f64,
    pub velocity_y: f64,
    pub velocity_z: f64,
    pub event_data_json: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AimInputSampleRecord {
    pub timestamp_ns: u64,
    pub sequence_number: u64,
    pub raw_dx: i32,
    pub raw_dy: i32,
    pub processed_dx: f64,
    pub processed_dy: f64,
    pub dt_ns: u64,        // raw QPC interval; 0 for first sample
    pub dt_used_ns: u64,   // after clamp (or same as dt_ns for none / bypass policy documented in tests)
    pub input_speed: Option<f64>,
    pub acceleration_scale: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AimCameraSampleRecord {
    pub timestamp_ns: u64,
    pub yaw_deg: f64,
    pub pitch_deg: f64,
    pub yaw_delta_deg: f64,
    pub pitch_delta_deg: f64,
}

impl TelemetryDb {
    pub fn insert_completed_aim_trial(
        &self,
        utc_date: &str,
        trial: &AimTrialRecord,
        shots: &[AimShotRecord],
        target_events: &[AimTargetEventRecord],
        input_samples: &[AimInputSampleRecord],
        camera_samples: &[AimCameraSampleRecord],
    ) -> Result<String, String>;
}
```

Migration notes:
- `CREATE TABLE IF NOT EXISTS` for `aim_target_events`, `aim_input_samples`, `aim_camera_samples`
- For existing DBs: `ALTER TABLE aim_trials ADD COLUMN …` each new column (guard with pragma/table_info or try-add ignore duplicate); `ALTER TABLE aim_shots ADD COLUMN target_id TEXT NOT NULL DEFAULT ''` then app always writes real ids for new trials
- Fresh `CREATE` for `aim_trials` in brand-new DBs should include all columns (update the IF NOT EXISTS batch used when table first created — if table already exists from 0.7.0, only ALTER path runs)

- [ ] **Step 1: Failing tests** in `db_roundtrip.rs`

```rust
#[test]
fn insert_completed_aim_trial_roundtrips_five_layers() {
    // migrate; build trial with new snapshot fields + 1 spawn/despawn event,
    // 1 shot with target_id, 2 input samples (dt_ns raw != dt_used_ns on 2nd),
    // 2 camera samples; insert; SELECT counts and key columns including dt_ns vs dt_used_ns
}

#[test]
fn insert_completed_aim_trial_rolls_back_all_layers_on_child_failure() {
    // e.g. duplicate shot_index after parent+events inserted path — assert all five tables count 0
}
```

Update existing aim insert tests for new signature (empty slices OK for events/input/camera only if schema allows — prefer always passing at least empty `&[]`).

- [ ] **Step 2: Run — expect FAIL** (compile / missing API)

Run: `cargo test -p sense-telemetry --test db_roundtrip`

- [ ] **Step 3: Implement types + migrate + insert**

SQLite `aim_input_samples` columns: `dt_ns`, `dt_used_ns`, nullable `input_speed`, `acceleration_scale`.  
`aim_target_events`: store yaw/pitch as REAL NULL when `Option::None`.

- [ ] **Step 4: Tests PASS**

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(telemetry): M3.y five-layer aim schema and atomic insert"
```

---

### Task 2: Deterministic seed + target lifecycle + shot target_id

**Files:**
- Modify: `src/aim_trial.rs`
- Tests in same module

**Interfaces:**
```rust
pub const STATIC_CLICK_TASK_VERSION: &str = "1";

// AimTrial adds:
pub random_seed: u64,
pub target_events: Vec<AimTargetEventRecord>,
pub current_target_id: String,
pub next_target_ordinal: u32, // 1-based → target_001

pub fn format_target_id(ordinal: u32) -> String {
    format!("target_{ordinal:03}")
}

// start_aim_trial(..., random_seed: u64, ...):
//   trial.random_seed = random_seed;
//   trial.rng_state = random_seed; // or hash seed into LCG init — document: rng_state = seed
//   clear shot_log, target_events, input/camera buffers (Task 3 may own those fields)
//   spawn first target → push spawn event; set current_target_id

// On hit that respawns: despawn event for old id; new ordinal; spawn event
// On final hit: despawn (no respawn)
// Miss: shot only, no miss target-event

// AimShotRecord.target_id = current_target_id at click
```

`static_click_task_config_json()` must include `"rng":"lcg","rng_version":"1"`.

`build_completed_aim_trial_record` must set `random_seed`, `task_version`, new snapshot fields (Task 4 may fill pitch/resolution — stub empty/`""`/0 here if needed, or take expanded args).

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn same_seed_yields_same_spawn_centers() { /* two trials start with seed 42; first 3 centers equal */ }

#[test]
fn spawn_and_despawn_events_without_miss_lifecycle() {
    // miss → events still only spawn; hit → despawn + maybe next spawn
}

#[test]
fn shots_carry_target_id() { /* … */ }
```

- [ ] **Step 2: Implement + PASS `cargo test -p sense-maxer`**

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(aim): deterministic seed and aim_target_events for STATIC_CLICK"
```

---

### Task 3: Buffer input + camera samples while Armed

**Files:**
- Modify: `src/aim_trial.rs` (buffers on `AimTrial`)
- Modify: `src/camera_ctrl.rs` (`drain_mouse_to_camera`)

**Interfaces:**
```rust
// AimTrial:
pub input_log: Vec<AimInputSampleRecord>,
pub camera_log: Vec<AimCameraSampleRecord>,
input_seq: u64,
last_raw_timestamp_ns: Option<u64>,
last_yaw_for_cam: f64,  // or read previous from last camera sample
last_pitch_for_cam: f64,

pub fn push_aim_input_sample(...);
pub fn push_aim_camera_sample(...);
```

In `drain_mouse_to_camera`, **when `aim.phase == Armed`**, after `process` + `apply_sample`:

1. `dt_ns = timestamp.saturating_sub(prev)` or `0` if first  
2. From `last_linear_eval()`: `dt_used_ns = (e.dt_ms * 1e6) as u64`, `input_speed`, `acceleration_scale`; for `none` (no eval): `dt_used_ns = dt_ns`, speed/scale `None` or scale `Some(1.0)` — pick one and test it  
3. Push `AimInputSampleRecord` with raw dx/dy and processed  
4. Push `AimCameraSampleRecord` with pose + deltas from previous camera sample (0,0 first)

Cancel / start must clear `input_log` and `camera_log`.

Do **not** write DB here. Finish path still calls persist (Task 4 expands payload).

- [ ] **Step 1: Unit test** push helpers / Armed buffering via aiming at known process — prefer testing `push_*` helpers and that cancel clears logs

- [ ] **Step 2: Wire drain + PASS tests**

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(aim): buffer input and camera samples during Armed trials"
```

---

### Task 4: Full trial snapshot + persist all layers + 0.8.0

**Files:**
- Modify: `src/session.rs` (`EXPERIMENT_VERSION = "0.8.0"`, `persist_completed_aim_trial`, `build` helpers)
- Modify: `src/aim_trial.rs` (`build_completed_aim_trial_record` full fields)
- Modify: `src/validation_lab.rs` / `camera_ctrl` if resolution must be passed (read `PrimaryWindow` in persist or HUD Start)

**Snapshot constants:**
```rust
pitch_model_id = "unverified_0.1"
pitch_model_version = "1" // or match sense-math if versioned
pitch_config_json = {/* spec example */}
hardware_config_json = {"mouse_model":"","mouse_connection":"","firmware":"","display_refresh_hz":null}
view_config_json = {"projection":"perspective","horizontal_fov_deg":…,"vertical_fov_deg":null,"camera_mode":"yaw_pitch","presentation_mode":"AutoNoVsync"}
```

`persist_completed_aim_trial` must pass `target_events`, `input_log`, `camera_log`, `shot_log` into `insert_completed_aim_trial`.

Update `experiment_version` assert to `0.8.0`.

- [ ] **Step 1: Update builder tests for new fields + seed on record**

- [ ] **Step 2: Implement persist + resolution/aspect capture**

- [ ] **Step 3: `cargo test -p sense-maxer` and `cargo test -p sense-telemetry` PASS**

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(app): persist full M3.y aim telemetry on STATIC_CLICK complete"
```

---

### Task 5: Docs + stop

**Files:**
- `docs/BASELINE.md` — `0.8.0`, M3.y COMPLETE/STOPPED  
- `docs/TELEMETRY_SCHEMA.md` — document five tables, dt_ns vs dt_used_ns, seed, lifecycle vs shots  
- `README.md` — M3.y status; stop; next = task-specific spec  

- [ ] **Step 1: Update docs**

- [ ] **Step 2: Commit**

```bash
git commit -m "docs: M3.y aim telemetry data model shipped; stop"
```

- [ ] **Step 3: Manual checklist (operator)**

1. Complete STATIC_CLICK; note HUD score/hits/accuracy  
2. `SELECT` trial row snapshot fields + `random_seed`  
3. Counts: `aim_target_events`, `aim_shots`, `aim_input_samples`, `aim_camera_samples` > 0  
4. Spot-check one input row: `dt_ns` vs `dt_used_ns`  
5. Cancel mid-run → no new completed trial  
6. **Stop** — do not expand telemetry further; next spec is task-specific

---

## Spec coverage checklist

| Spec item | Task |
|-----------|------|
| Expanded `aim_trials` snapshot columns/JSON | 1, 4 |
| `random_seed` + deterministic RNG + task_version/rng in JSON | 2, 4 |
| `aim_target_events` lifecycle only | 1, 2 |
| `aim_shots.target_id` | 1, 2 |
| `aim_input_samples` + dt_ns/dt_used_ns | 1, 3 |
| `aim_camera_samples` | 1, 3 |
| Completed-only five-layer txn | 1, 4 |
| Abort clears buffers | 2, 3 |
| 0.8.0 + docs stop | 4, 5 |
| No derived ML columns / no new tasks | all |

## Consistency notes

- Insert API signature change is breaking for in-tree callers only (`session.rs` + tests) — update in Task 1/4 together as needed; Task 1 can temporarily require callers to pass `&[]` and fix compile in same commit by updating `session.rs` call site with empty logs until Task 3 fills them.
- Prefer fixing `session.rs` compile in Task 1 with empty buffers so the tree always builds.

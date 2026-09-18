# M3.x Aim Trial Persistence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist completed STATIC_CLICK aim trials to SQLite (`aim_trials` + `aim_shots`) transactionally, with HUD live/result/persist status — then stop M3.x.

**Architecture:** Shared type-discriminated aim layer in `sense-types` + `TelemetryDb::insert_completed_aim_trial`. In-memory shot buffer on `AimTrial`; write only on successful 5th hit. Abort never inserts. Raw/processed mouse tables and Validation Lab flush paths unchanged.

**Tech Stack:** rusqlite (existing), `sense-types`, `sense-telemetry`, Bevy resources, egui HUD.

**Spec:** `docs/superpowers/specs/2026-09-18-m3x-aim-trial-persistence-design.md`

## Global Constraints

- Dedicated `aim_trials` / `aim_shots` only — never stuff aim into validation/raw/processed tables
- Insert **completed** rows only; abort/cancel/quit while Armed → no DB write
- Trial id: `aim_{utc_date}_{seq:06}` where `seq = max(suffix for that day prefix) + 1` (**not** `COUNT(*) + 1`); allocate **inside** the write txn; PK conflict fails txn
- `experiment_id` = `aim_lab`; `experiment_version` = `0.7.0`; `trial_type` = `STATIC_CLICK`; `status` = `completed`
- Raw + processed input architecture unchanged
- Stop after this plan — no new trial types

## File map

| File | Responsibility |
|------|----------------|
| `crates/sense-types/src/lib.rs` | `AimTrialRecord`, `AimShotRecord` |
| `crates/sense-telemetry/src/db.rs` | migrate tables; `insert_completed_aim_trial`; next-id helper inside txn |
| `crates/sense-telemetry/tests/db_roundtrip.rs` | round-trip + abort-does-not-insert (via API absence) |
| `src/aim_trial.rs` | shot buffer, counters, `AimPersistStatus`, finish payload builder |
| `src/session.rs` | `persist_completed_aim_trial` (open DB, wall clock, call telemetry); bump `EXPERIMENT_VERSION` to `0.7.0` |
| `src/camera_ctrl.rs` | on finish from `apply_aim_shot`, call persist with settings/processor snapshot |
| `src/validation_lab.rs` | HUD: elapsed, accuracy, processor, persist line; abort clears false-complete messaging |
| `docs/BASELINE.md`, `docs/TELEMETRY_SCHEMA.md`, `README.md` | version + schema; M3.x stop |

---

### Task 1: Types + DB migrate + transactional insert

**Files:**
- Modify: `crates/sense-types/src/lib.rs`
- Modify: `crates/sense-telemetry/src/db.rs`
- Modify: `crates/sense-telemetry/tests/db_roundtrip.rs`

**Interfaces:**
- Produces:
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AimTrialRecord {
    pub id: String, // filled by insert API if empty? Prefer: insert allocates id and returns it
    pub app_version: String,
    pub experiment_id: String,
    pub experiment_version: String,
    pub trial_type: String,
    pub status: String, // always "completed"
    pub processor_id: String,
    pub processor_version: String,
    pub processor_config_json: String,
    pub dpi: f64,
    pub sensitivity: f64,
    pub polling_rate_hz: f64,
    pub fov_degrees_h: f64,
    pub task_config_json: String,
    pub metrics_json: String,
    pub start_unix_ms: i64,
    pub end_unix_ms: i64,
    pub start_timestamp_ns: u64,
    pub end_timestamp_ns: u64,
    pub duration_secs: f64,
    pub hits: u32,
    pub shots: u32,
    pub misses: u32,
    pub score_secs: f64,
    pub accuracy: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AimShotRecord {
    pub shot_index: u32,
    pub timestamp_ns: u64,
    pub hit: bool,
    pub yaw_deg: f64,
    pub pitch_deg: f64,
    pub target_x: f64,
    pub target_y: f64,
    pub target_z: f64,
    pub target_radius: f64,
}

impl TelemetryDb {
    /// Allocates `aim_{date}_{seq:06}` inside the txn (max suffix + 1).
    /// Returns allocated trial id. Rolls back entirely on any error / PK conflict.
    pub fn insert_completed_aim_trial(
        &self,
        utc_date: &str, // YYYYMMDD
        trial: &AimTrialRecord, // id ignored / empty; other fields required; status must be "completed"
        shots: &[AimShotRecord],
    ) -> Result<String, String>;
}
```

- [ ] **Step 1: Write failing round-trip test**

In `crates/sense-telemetry/tests/db_roundtrip.rs`, add a test that opens `:memory:` (or temp file), `migrate()`, builds one `AimTrialRecord` (id empty) + two `AimShotRecord`s, calls `insert_completed_aim_trial("20260918", &trial, &shots)`, then:

```sql
SELECT hits, shots, misses, score_secs, accuracy, trial_type, status, experiment_version
FROM aim_trials WHERE id = ?1
```

Assert values match; assert shot count = 2 ordered by `shot_index`; assert second insert same day yields `..._000002`.

Also assert: after migrate, `COUNT(*)` from a fake incomplete path is N/A — instead unit-test that calling insert with `status != "completed"` returns `Err` (guard).

- [ ] **Step 2: Run test — expect FAIL** (types/API missing)

Run: `cargo test -p sense-telemetry --test db_roundtrip -- insert_completed_aim_trial --nocapture`  
Expected: compile error or test fail on missing API

- [ ] **Step 3: Add types to `sense-types`**

Add `AimTrialRecord` and `AimShotRecord` as above. Re-export via existing crate root.

- [ ] **Step 4: Migrate DDL + insert implementation**

In `TelemetryDb::migrate`, after processed_mouse block, always:

```sql
CREATE TABLE IF NOT EXISTS aim_trials (
  id TEXT PRIMARY KEY,
  app_version TEXT NOT NULL,
  experiment_id TEXT NOT NULL,
  experiment_version TEXT NOT NULL,
  trial_type TEXT NOT NULL,
  status TEXT NOT NULL,
  processor_id TEXT NOT NULL,
  processor_version TEXT NOT NULL,
  processor_config_json TEXT NOT NULL,
  dpi REAL NOT NULL,
  sensitivity REAL NOT NULL,
  polling_rate_hz REAL NOT NULL,
  fov_degrees_h REAL NOT NULL,
  task_config_json TEXT NOT NULL,
  metrics_json TEXT NOT NULL,
  start_unix_ms INTEGER NOT NULL,
  end_unix_ms INTEGER NOT NULL,
  start_timestamp_ns INTEGER NOT NULL,
  end_timestamp_ns INTEGER NOT NULL,
  duration_secs REAL NOT NULL,
  hits INTEGER NOT NULL,
  shots INTEGER NOT NULL,
  misses INTEGER NOT NULL,
  score_secs REAL NOT NULL,
  accuracy REAL NOT NULL
);

CREATE TABLE IF NOT EXISTS aim_shots (
  trial_id TEXT NOT NULL,
  shot_index INTEGER NOT NULL,
  timestamp_ns INTEGER NOT NULL,
  hit INTEGER NOT NULL,
  yaw_deg REAL NOT NULL,
  pitch_deg REAL NOT NULL,
  target_x REAL NOT NULL,
  target_y REAL NOT NULL,
  target_z REAL NOT NULL,
  target_radius REAL NOT NULL,
  PRIMARY KEY(trial_id, shot_index),
  FOREIGN KEY(trial_id) REFERENCES aim_trials(id)
);

CREATE INDEX IF NOT EXISTS idx_aim_shots_trial ON aim_shots(trial_id);
```

Implement `next_aim_sequence(tx, prefix) -> u32` by selecting ids `LIKE prefix%`, parse suffix after prefix, `max + 1` (same spirit as `src/session.rs::next_sequence`). **Never** `COUNT(*)`.

`insert_completed_aim_trial`:
1. Reject if `trial.status != "completed"`
2. `BEGIN` (or `unchecked_transaction`)
3. `seq = next_aim_sequence`; `id = format!("aim_{utc_date}_{seq:06}")`
4. INSERT trial with that id
5. INSERT each shot with `trial_id = id`
6. COMMIT; return `id`
7. Any error → drop txn (rollback); return `Err`

- [ ] **Step 5: Run round-trip test — PASS**

Run: `cargo test -p sense-telemetry --test db_roundtrip`  
Expected: all pass including new aim tests

- [ ] **Step 6: Commit**

```bash
git add crates/sense-types/src/lib.rs crates/sense-telemetry/src/db.rs crates/sense-telemetry/tests/db_roundtrip.rs
git commit -m "feat(telemetry): aim_trials/aim_shots schema and completed-trial insert"
```

---

### Task 2: In-memory shot buffer + finish payload

**Files:**
- Modify: `src/aim_trial.rs`
- Test: same file `#[cfg(test)]`

**Interfaces:**
- Consumes: `AimTrialRecord`, `AimShotRecord` shapes (fields only; DB call is Task 3)
- Produces:
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct AimPersistStatus {
    pub trial_id: Option<String>,
    pub score_secs: f64,
    pub hits: u32,
    pub shots: u32,
    pub accuracy: f64,
    pub saved_ok: bool,
    pub error: Option<String>,
}

// On AimTrial, add:
// shot_log: Vec<AimShotRecord>  (or local buffer type then convert)
// start_unix_ms: i64  (set at start)
// last_persist: Option<AimPersistStatus>
// shots_total / use shot_log.len(); misses derived

pub fn static_click_task_config_json() -> String { /* serde_json or manual string matching spec */ }

pub fn build_completed_aim_trial_record(
    settings: &ExperimentSettings,
    processor_id: &str,
    processor_version: &str,
    processor_config_json: &str,
    trial: &AimTrial,
    end_unix_ms: i64,
    end_timestamp_ns: u64,
) -> AimTrialRecord; // id "", status completed, experiment_id aim_lab, version 0.7.0

pub fn cancel_aim_trial(trial: &mut AimTrial); // phase Idle; clear Armed buffers that would imply incomplete save; do NOT invent last_persist; clear any half-finished flags — keep last_persist only if it was from a prior successful/failed *completed* attempt (see below)
```

**Abort rule for `last_persist`:** On cancel, do **not** set a new persist status. Leave previous `last_persist` unchanged only if it referred to an earlier finished attempt; never set `saved_ok=true` for an abort. Clear in-progress shot_log / hits when cancelling so HUD cannot show the aborted run as a completed score unless `score_secs` was already set from a prior finish — on cancel mid-run, clear `score_secs` if phase was Armed without having finished (Armed cancel: clear hits, shot_log, score_secs; keep old `last_persist` from previous trial).

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn shots_buffer_on_hit_and_miss() { /* start; miss → 1 shot hit=false; hit → 2 shots; hits==1 */ }

#[test]
fn fifth_hit_sets_score_and_shot_count() { /* existing fifth_hit test + assert shot_log.len() == shots */ }

#[test]
fn cancel_armed_clears_run_without_persist_ok() {
    // start; one miss; cancel; phase Idle; score_secs None; last_persist unchanged/None; shot_log empty
}

#[test]
fn build_record_snapshots_task_and_processor() {
    // build_completed_aim_trial_record → trial_type STATIC_CLICK, experiment_version 0.7.0,
    // task_config_json contains hits_required 5, accuracy = hits/shots
}
```

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo test -p sense-maxer aim_trial::tests -- --nocapture`

- [ ] **Step 3: Implement buffer + builders + cancel semantics**

In `apply_aim_shot`: push `AimShotRecord` every click before hit/miss branch; on 5th hit set score as today.

Export `build_completed_aim_trial_record` using constants from aim_trial + settings.

- [ ] **Step 4: Tests PASS** — keep existing 20-style aim tests green; new ones pass

Run: `cargo test -p sense-maxer`

- [ ] **Step 5: Commit**

```bash
git add src/aim_trial.rs
git commit -m "feat(aim): buffer shots and build completed trial records"
```

---

### Task 3: Persist on complete + HUD

**Files:**
- Modify: `src/session.rs` (`EXPERIMENT_VERSION = "0.7.0"`; add `persist_completed_aim_trial`)
- Modify: `src/camera_ctrl.rs` (after `apply_aim_shot` returns true / finish)
- Modify: `src/validation_lab.rs` (HUD fields)
- Modify: `src/aim_trial.rs` if finish needs `start_unix_ms` at start via `unix_time_ms` passed from caller

**Interfaces:**
```rust
// session.rs
pub fn persist_completed_aim_trial(
    settings: &ExperimentSettings,
    active_processor: &ActiveInputProcessor, // or id/version/config strings
    trial: &mut AimTrial,
    end_unix_ms: i64,
    end_timestamp_ns: u64,
) -> AimPersistStatus;

// Uses open_database(), utc_date_from_unix_ms, TelemetryDb::insert_completed_aim_trial
// On Ok(id): AimPersistStatus { trial_id: Some(id), saved_ok: true, ... }
 // On Err(e): AimPersistStatus { trial_id: None, saved_ok: false, error: Some(e), score still filled from trial }
```

`drain_mouse_to_camera`: when `apply_aim_shot` indicates finished (returns true), call `persist_completed_aim_trial` and store into `trial.last_persist`. Need access to `ExperimentSettings` + `ActiveInputProcessor` (already in that system).

`start_aim_trial`: accept `start_unix_ms: i64` and store on trial; HUD Start passes `unix_time_ms()`.

**HUD (STATIC_CLICK section):**
- Armed: `AIM: Armed  HITS: k/5  ELAPSED: x.xxx s` + `PROCESSOR: {id}` short config (gain/caps or none)
- Idle after complete: score, hits/required, duration, accuracy (`hits/shots`)
- Persist: `SAVED: {trial_id}` or `SAVE FAILED: {error}` from `last_persist`
- Cancel: existing button; no new saved line for aborted run

- [ ] **Step 1: Failing unit test for persist status mapping** (optional pure function test if extract helper)

```rust
#[test]
fn persist_status_ok_and_err_shapes() { /* if helper exists */ }
```

Or integration-style: call `persist_completed_aim_trial` against temp DB in session tests — prefer temp path under `std::env::temp_dir()` only if `DATABASE_PATH` is injectable; **if not injectable in this task**, keep DB test coverage in Task 1 and manually verify; still unit-test `build_completed_aim_trial_record` (Task 2) + HUD-facing fields on `AimPersistStatus` after a fake assign.

Minimal: add `session` test that uses `TelemetryDb::open` temp file + migrate + insert via public API already tested; app wiring smoke via compile.

- [ ] **Step 2: Implement `persist_completed_aim_trial` + wire finish in `camera_ctrl`**

- [ ] **Step 3: HUD updates in `validation_lab.rs`**

- [ ] **Step 4: Bump `EXPERIMENT_VERSION` to `0.7.0`**; fix `experiment_version_captures_gain_and_cap_settings` assert

- [ ] **Step 5: `cargo test -p sense-maxer` and `cargo test -p sense-telemetry` PASS**

- [ ] **Step 6: Commit**

```bash
git add src/session.rs src/camera_ctrl.rs src/validation_lab.rs src/aim_trial.rs
git commit -m "feat(app): persist completed STATIC_CLICK trials and show HUD status"
```

---

### Task 4: Docs + stop M3.x

**Files:**
- Modify: `docs/BASELINE.md` — experiment version `0.7.0`; M3.x persistence complete / STOPPED
- Modify: `docs/TELEMETRY_SCHEMA.md` — replace Future `trials` with documented `aim_trials` / `aim_shots`; note completed-only
- Modify: `README.md` — M3.x status line
- Modify: `docs/ARCHITECTURE.md` only if it still says “no aim tasks” / future trials only — one short correction

- [ ] **Step 1: Update docs per spec Version / docs + Stop criteria**

- [ ] **Step 2: Commit**

```bash
git add docs/BASELINE.md docs/TELEMETRY_SCHEMA.md README.md docs/ARCHITECTURE.md
git commit -m "docs: M3.x aim trial persistence shipped; stop"
```

- [ ] **Step 3: Manual check (human / agent with app)**

1. Run app; complete 2–3 STATIC_CLICK runs; note HUD score, hits, accuracy, duration, `SAVED id`
2. `sqlite3 data/sense_maxer.db "SELECT id, hits, shots, score_secs, accuracy, status FROM aim_trials ORDER BY end_unix_ms;"`
3. Confirm rows match HUD; `SELECT COUNT(*) FROM aim_shots WHERE trial_id=...` equals `shots`
4. Start a run, Cancel mid-way; confirm no new completed row
5. Stop — do not start FLICK/TRACK or further M3 work

---

## Spec coverage checklist

| Spec item | Task |
|-----------|------|
| `aim_trials` / `aim_shots` DDL | 1 |
| max-prefix+1 id inside txn | 1 |
| completed-only transactional write | 1, 3 |
| Abort no insert | 2, 3 |
| Processor + DPI/sens/poll/FOV snapshot | 2, 3 |
| task_config_json STATIC_CLICK | 2 |
| Per-shot rows | 1, 2 |
| HUD live + persist | 3 |
| experiment 0.7.0 + docs + stop | 3, 4 |
| Raw/processed unchanged | all (no edits to those insert paths) |

## Placeholder / consistency review

- No TBD steps; id allocation rule matches spec; types named consistently `AimTrialRecord` / `AimShotRecord` / `insert_completed_aim_trial` / `AimPersistStatus`.
- `EXPERIMENT_VERSION` single constant in `session.rs` bumped to `0.7.0` for both validation sessions and aim rows (aim record also hardcodes or reads that constant when building).

# M2 InputProcessor Framework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `InputProcessor` a session-scoped experimental boundary with dual raw/processed telemetry, identity snapshots, and QPC-derived `dt_s` — with only `NoAcceleration` implemented.

**Architecture:** New thin `sense-accel` crate owns the trait, `NoAcceleration`, and `create_processor` factory. Bevy selects the processor at Start Validation only. Drain uses consecutive raw QPC timestamps for `dt_s` (never render delta). New `processed_mouse_events` SQLite table; M1 `raw_mouse_events` unchanged. VSync ON baseline retained.

**Tech Stack:** Existing workspace; new path crate `sense-accel`; `rusqlite`; Bevy 0.19 app wiring.

**Spec:** `docs/superpowers/specs/2026-09-17-m2-input-processor-design.md`

## Global Constraints

- VSync ON (`PresentMode::AutoVsync`); FPS capped to refresh; do not re-enable uncapped
- WM_INPUT / QPC / queue / integrity logic unchanged
- Yaw: `processed_dx × sensitivity × 0.07`; constant `0.07` UNCERTAIN; no compensation
- Pitch disabled
- `dt_s = (T_n - T_{n-1}) / 1e9` from raw sample timestamps; first sample `dt_s = 0.0`; never `Time::delta_*`
- Raw table immutable; processed is separate derived table with per-row processor_id/version/config_json
- Only processor `"none"` / `NoAcceleration` in M2
- No mid-session processor swap
- No Raw Accel math, aim tasks, pitch, export
- Stop after M2 verification

---

## File Structure

```
crates/sense-accel/
  Cargo.toml
  src/lib.rs              # trait, NoAcceleration, create_processor, dt_s helper
  tests/processor_tests.rs
crates/sense-types/src/lib.rs   # add ProcessedMouseSample; remove old InputProcessor (moved)
crates/sense-telemetry/
  src/buffers.rs          # + processed vec
  src/db.rs               # migrate + flush processed
  tests/db_roundtrip.rs
src/camera_ctrl.rs        # QPC dt_s; dual buffer; Box<dyn InputProcessor> resource
src/session.rs            # create_processor at Start; snapshot fields
src/config.rs             # processor_id setting (default "none")
src/validation_lab.rs     # HUD processor display; lock while Running
src/app.rs                # wire resources (no present_mode change)
docs/M2_PROCESSOR.md
docs/TELEMETRY_SCHEMA.md
docs/ARCHITECTURE.md
```

---

### Task 1: `sense-accel` crate — trait, factory, QPC `dt_s`

**Files:**
- Create: `crates/sense-accel/Cargo.toml`, `crates/sense-accel/src/lib.rs`, `crates/sense-accel/tests/processor_tests.rs`
- Modify: root `Cargo.toml` (workspace member + workspace.dependency)
- Modify: `crates/sense-types/src/lib.rs` — remove `InputProcessor` / `NoAcceleration` (moved)
- Modify: any crates that imported them from `sense-types` to use `sense-accel` temporarily or after Task 4

**Interfaces:**
- Produces:

```rust
pub trait InputProcessor: Send {
    fn id(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn config_json(&self) -> String;
    fn process(&mut self, dx: f64, dy: f64, dt_s: f64) -> (f64, f64);
}

pub struct NoAcceleration;

pub fn create_processor(id: &str) -> Result<Box<dyn InputProcessor>, String>;

/// Inter-sample dt from consecutive raw QPC timestamps (ns).
/// First sample / missing previous: returns 0.0.
pub fn dt_s_from_timestamps(prev_ns: Option<u64>, current_ns: u64) -> f64;
```

- [ ] **Step 1: Write failing tests** in `crates/sense-accel/tests/processor_tests.rs`

```rust
use sense_accel::*;

#[test]
fn no_acceleration_is_identity() {
    let mut p = NoAcceleration;
    assert_eq!(p.id(), "none");
    assert_eq!(p.version(), "1.0.0");
    assert_eq!(p.config_json(), "{}");
    assert_eq!(p.process(3.0, -2.0, 0.004), (3.0, -2.0));
}

#[test]
fn factory_accepts_none_rejects_unknown() {
    assert!(create_processor("none").is_ok());
    assert!(create_processor("raw_accel").is_err());
}

#[test]
fn dt_s_from_consecutive_raw_timestamps() {
    assert_eq!(dt_s_from_timestamps(None, 1_000_000_000), 0.0);
    assert_eq!(dt_s_from_timestamps(Some(1_000_000_000), 1_004_000_000), 0.004);
    // Must not use render-frame semantics — only timestamp math.
}
```

- [ ] **Step 2: Run `cargo test -p sense-accel` — expect FAIL**

- [ ] **Step 3: Implement `lib.rs` + Cargo.toml; add workspace member**

- [ ] **Step 4: Remove trait from `sense-types`; fix compile breaks with temporary re-exports or update imports to `sense_accel` in binary/tests**

- [ ] **Step 5: `cargo test -p sense-accel` and `cargo test --workspace` PASS**

- [ ] **Step 6: Commit** (if committing)

```bash
git add crates/sense-accel Cargo.toml crates/sense-types src
git commit -m "feat(sense-accel): InputProcessor trait, NoAcceleration, QPC dt_s"
```

---

### Task 2: `ProcessedMouseSample` + configuration snapshot fields

**Files:**
- Modify: `crates/sense-types/src/lib.rs`
- Modify: `crates/sense-types/tests/types_smoke.rs`

**Interfaces:**
- Produces:

```rust
pub struct ProcessedMouseSample {
    pub timestamp_ns: u64,
    pub sequence_number: u64,
    pub processed_dx: f64,
    pub processed_dy: f64,
    pub processor_id: String,
    pub processor_version: String,
    pub processor_config_json: String,
}
```

Extend `AccelerationConfig` (or `ConfigurationRecord`) to carry:

```rust
pub processor_id: String,           // "none"
pub processor_version: String,      // "1.0.0"
pub processor_config_json: String,  // "{}"
```

Keep `enabled: false` / `model: "none"` compatible or map `model` ↔ `processor_id` explicitly in session start.

- [ ] **Step 1: Add type + serde roundtrip test**

- [ ] **Step 2: Update `ConfigurationRecord` construction sites to compile**

- [ ] **Step 3: Tests PASS**

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(sense-types): ProcessedMouseSample and processor snapshot fields"
```

---

### Task 3: Telemetry buffers + `processed_mouse_events` SQLite

**Files:**
- Modify: `crates/sense-telemetry/src/buffers.rs`
- Modify: `crates/sense-telemetry/src/db.rs`
- Modify: `crates/sense-telemetry/tests/db_roundtrip.rs`
- Modify: `crates/sense-telemetry/Cargo.toml` if needed

**Interfaces:**
- `SessionBuffers.processed: Vec<ProcessedMouseSample>`
- `clear()` also clears `processed`
- `migrate()` adds:

```sql
CREATE TABLE IF NOT EXISTS processed_mouse_events (
  session_id TEXT NOT NULL,
  sequence_number INTEGER NOT NULL,
  timestamp_ns INTEGER NOT NULL,
  processed_dx REAL NOT NULL,
  processed_dy REAL NOT NULL,
  processor_id TEXT NOT NULL,
  processor_version TEXT NOT NULL,
  processor_config_json TEXT NOT NULL,
  PRIMARY KEY(session_id, sequence_number)
);
CREATE INDEX IF NOT EXISTS idx_processed_mouse_session ON processed_mouse_events(session_id);
```

Note: existing DBs may already have tables from non-IF-NOT-EXISTS migrate — use `CREATE TABLE IF NOT EXISTS` for the **new** table only; do not rewrite M1 table definitions in a breaking way. If current `migrate()` uses bare `CREATE TABLE` without IF NOT EXISTS, keep that behavior for old tables and append IF NOT EXISTS only for `processed_mouse_events` (or run additive execute after batch). Prefer additive second `execute_batch` for the new table so existing `data/sense_maxer.db` keeps working.

- Extend `flush_buffers` / `complete_validation` to insert processed rows in the **same** transaction.

- [ ] **Step 1: Failing roundtrip test** — flush 2 raw + 2 processed + assert counts; assert `processor_id == "none"`

- [ ] **Step 2: Implement buffers + migrate + flush**

- [ ] **Step 3: `cargo test -p sense-telemetry` PASS**

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(sense-telemetry): processed_mouse_events table and flush"
```

---

### Task 4: Bevy drain + session processor lock + QPC `dt_s`

**Files:**
- Modify: `src/camera_ctrl.rs`
- Modify: `src/session.rs`
- Modify: `src/config.rs`
- Modify: `src/app.rs`
- Modify: `src/validation_lab.rs` (minimal if HUD is Task 5)
- Modify: root `Cargo.toml` — depend on `sense-accel`

**Interfaces:**
- `ExperimentSettings.processor_id: String` default `"none"`
- Resource holding active processor:

```rust
pub struct ActiveInputProcessor {
    pub processor: Box<dyn sense_accel::InputProcessor>,
}
```

- Replace frame-based dt:

```rust
let mut prev_ts: Option<u64> = None; // or store on resource across frames while Running
for sample in samples {
    let dt_s = sense_accel::dt_s_from_timestamps(prev_ts, sample.timestamp_ns);
    prev_ts = Some(sample.timestamp_ns);
    let (pdx, pdy) = processor.process(sample.dx as f64, sample.dy as f64, dt_s);
    // yaw from pdx; push raw + ProcessedMouseSample { ..., processor_id, version, config_json }
}
```

Carry `prev_ts` on a resource (e.g. `ProcessorTimingState { last_raw_timestamp_ns: Option<u64> }`) reset on Start / Reset Counters so inter-frame sample pairs still get correct `dt_s`.

- `start_validation`: `create_processor(&settings.processor_id)?`; snapshot id/version/config into `AccelerationConfig` / configuration; install `ActiveInputProcessor`; reset timing state.
- While Running: do not change `processor_id` from HUD.

- [ ] **Step 1: Unit test** for drain helper or timing state: two timestamps → 0.004; first → 0.0

- [ ] **Step 2: Remove `time.delta_secs_f64()` from processor path entirely**

- [ ] **Step 3: Dual-buffer processed samples while Running**

- [ ] **Step 4: `cargo test --workspace` PASS**

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(app): session-locked processor and QPC-derived dt_s"
```

---

### Task 5: HUD + docs + M2 stop

**Files:**
- Modify: `src/validation_lab.rs`
- Create: `docs/M2_PROCESSOR.md`
- Modify: `docs/TELEMETRY_SCHEMA.md`, `docs/ARCHITECTURE.md`, `README.md` (M2 status note)
- Optional: bump `experiment_version` only if task algorithms change — for M2 processor framework, prefer `experiment_version = "0.2.0"` **or** keep validation_lab 0.1.0 and document processor snapshot separately; **use `experiment_version = "0.2.0"`** when processor telemetry is part of the experiment record.

**HUD:**

- `PROCESSOR: none`
- `PROCESSOR VERSION: 1.0.0`
- Short note: processed == raw under none
- Processor id editable only when Idle (combo/label; M2 only `"none"`)

- [ ] **Step 1: HUD fields + Idle-only selection (even if only one option)**

- [ ] **Step 2: Write `docs/M2_PROCESSOR.md`** (pipeline, dt_s rules, table, out of scope)

- [ ] **Step 3: Sync TELEMETRY_SCHEMA + ARCHITECTURE**

- [ ] **Step 4: Manual checklist in docs** — Start/End; confirm DB processed rows

- [ ] **Step 5: `cargo test --workspace` + `cargo build --release`**

- [ ] **Step 6: Commit + STOP**

```bash
git commit -m "docs: M2 processor framework complete; stop before Raw Accel"
```

**STOP.** Do not implement M2.x Raw Accel.

---

## Spec Coverage

| Spec item | Task |
|-----------|------|
| sense-accel trait/factory/NoAcceleration | 1 |
| QPC `dt_s` helper + drain wiring | 1, 4 |
| ProcessedMouseSample | 2 |
| processed_mouse_events + atomic flush | 3 |
| Session lock / snapshot | 4 |
| HUD | 5 |
| Docs + stop | 5 |
| VSync ON retained | Global (no Task changes present_mode) |
| No Raw Accel | Global |

## Placeholder / consistency review

- `dt_s` never from Bevy `Time`
- Processor fields named consistently: `processor_id`, `processor_version`, `processor_config_json`
- Raw table columns untouched

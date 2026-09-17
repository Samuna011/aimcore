# M2 InputProcessor Framework — Design Spec

**Date:** 2026-09-17  
**Status:** Draft for user review  
**Depends on:** M1 COMPLETE / STOPPED; `docs/BASELINE.md`  
**Scope:** Processor architecture as experimental boundary only — **no** Raw Accel mathematics

---

## 1. Purpose

Establish `InputProcessor` as a first-class, session-scoped experimental boundary between immutable raw mouse samples and yaw/camera application.

M2 proves:

```
WM_INPUT → Raw sample → InputProcessor → Processed dx/dy → Yaw → Camera
```

with dual telemetry (raw + processed), processor identity snapshotted per session, and **only** `NoAcceleration` implemented.

M2 does **not** introduce acceleration curves, Raw Accel reproduction, aim tasks, or pitch.

---

## 2. Locked Baseline (must not regress)

From `docs/BASELINE.md`:

| Setting | Value |
|---------|--------|
| VSync | **ON** (`PresentMode::AutoVsync`) |
| FPS | Capped to display refresh via VSync |
| Raw input | WM_INPUT + SetWindowSubclass |
| Timestamp | QPC → ns |
| Acceleration (until processors expand) | `NoAcceleration` |
| Pitch | Disabled |
| Yaw constant | `0.07` **UNCERTAIN** |
| HFOV | 103° horizontal |
| DPI / sensitivity | Configurable per experiment |

**History:** M1 Run 3 temporarily used uncapped / VSync-off for pipeline verification only. That is **not** the lasting baseline.

Input must remain event-driven and queue-drained independently of render FPS.

---

## 3. Architecture Decision

**Approach:** Extend in place (Approach 1 from brainstorming).

- Formalize / extend `InputProcessor` API (currently in `sense-types`).
- Add registry/factory (prefer small `sense-accel` crate **or** module under `sense-types` / `src/` — prefer thin `crates/sense-accel` so M2.x curves do not bloat types).
- Bevy session start selects processor once.
- New SQLite table `processed_mouse_events`; **do not** alter M1 `raw_mouse_events` columns.

---

## 4. Data Flow

```
WM_INPUT
  → QPC timestamp
  → MouseSample { dx, dy, timestamp_ns, ... }   // IMMUTABLE raw
  → InputProcessor::process(dx, dy, dt_s)
  → (processed_dx, processed_dy)
  → yaw_delta = processed_dx × sens × 0.07
  → camera + InputCameraSample
```

### `dt_s` definition (critical for M2.x)

`dt_s` is the inter-sample interval derived from **consecutive raw mouse-event QPC timestamps**, not from Bevy render-frame timing:

```
sample N-1 timestamp_ns = T1
sample N   timestamp_ns = T2

dt_s = (T2 - T1) / 1_000_000_000
```

Rules:

- `dt_s` **must** come from the raw sample stream’s monotonic QPC timestamps.
- `dt_s` **must not** be `Time::delta_secs()`, frame duration, or any quantity derived from FPS / VSync / present mode.
- For the first sample in a burst/session (no previous timestamp), use `dt_s = 0.0` (or skip velocity-dependent effects); document the chosen convention in code. Under `NoAcceleration`, the value is unused for the transform but the API still receives the correct interval.
- The processor API must not become accidentally tied to Bevy’s frame rate. This is required so M2.x Raw Accel (velocity-dependent) can use the same `process` signature without a frame-coupled `dt`.

Rules (general):

- Raw samples never mutated or overwritten.
- Yaw uses **processed** horizontal counts.
- Under `NoAcceleration`, processed == raw (identity), so M1 validation behavior is preserved.
- Mid-session processor swap is **forbidden**.

---

## 5. API

```rust
pub trait InputProcessor: Send {
    fn id(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn config_json(&self) -> String; // reproducible blob; "{}" for none
    fn process(&mut self, dx: f64, dy: f64, dt_s: f64) -> (f64, f64);
}

pub struct NoAcceleration;

impl InputProcessor for NoAcceleration {
    fn id(&self) -> &'static str { "none" }
    fn version(&self) -> &'static str { "1.0.0" }
    fn config_json(&self) -> String { "{}".into() }
    fn process(&mut self, dx: f64, dy: f64, _dt_s: f64) -> (f64, f64) { (dx, dy) }
}

pub fn create_processor(id: &str) -> Result<Box<dyn InputProcessor>, String>;
// M2: only "none" succeeds.
```

Registry/factory:

- Maps `"none"` → `NoAcceleration`.
- Unknown ids → error at session start (fail closed).

Session settings may expose `processor_id` (default `"none"`). Changing it while Idle is allowed; it takes effect only on **Start Validation**.

---

## 6. Types

```rust
pub struct ProcessedMouseSample {
    pub timestamp_ns: u64,       // same as paired raw sample
    pub sequence_number: u64,    // same as paired raw sample
    pub processed_dx: f64,
    pub processed_dy: f64,
    pub processor_id: String,
    pub processor_version: String,
    pub processor_config_json: String,
}
```

In-memory `SessionBuffers` gains `processed: Vec<ProcessedMouseSample>`.

Configuration / session snapshot fields:

- `processor_id`
- `processor_version`
- `processor_config_json`

(Or nested under existing acceleration fields if cleaner — must be queryable and snapshotted at Start.)

---

## 7. SQLite Schema (M2)

### Unchanged

`raw_mouse_events` — M1 immutable source. No new raw columns.

### New: `processed_mouse_events`

| Column | Type | Notes |
|--------|------|-------|
| `session_id` | TEXT | |
| `sequence_number` | INTEGER | pairs with raw |
| `timestamp_ns` | INTEGER | same as raw sample |
| `processed_dx` | REAL | |
| `processed_dy` | REAL | |
| `processor_id` | TEXT | e.g. `none` |
| `processor_version` | TEXT | e.g. `1.0.0` |
| `processor_config_json` | TEXT | e.g. `{}` |

Primary key: `(session_id, sequence_number)`.

Processor identity is stored **per row** so the transformation remains reproducible even if session config is incomplete.

Indexes: `session_id`.

### Migration

- Extend `TelemetryDb::migrate()` with `CREATE TABLE IF NOT EXISTS processed_mouse_events ...`.
- Extend atomic `complete_validation` (or flush path) to insert processed rows in the **same** transaction as raw/camera/frames/result.

---

## 8. Bevy / Session Wiring

1. **Idle:** HUD shows selected processor id (only `none` in M2). DPI/sens editable as today. Present mode remains AutoVsync.
2. **Start Validation:**  
   - `create_processor(selected_id)`  
   - Snapshot processor id/version/config into configuration + session  
   - Install session processor resource  
   - Existing integrity/buffer resets unchanged  
3. **Running:**  
   - Drain all raw samples  
   - For each sample: compute `dt_s` from consecutive raw QPC timestamps (see §4); `process(dx, dy, dt_s)` → record raw + processed + input camera  
   - Do **not** pass render-frame delta into the processor  
   - Processor UI locked  
4. **End Validation:** existing atomic completion + processed flush  
5. **Reset Counters:** clear processed buffer with other buffers; integrity reset unchanged  

Do **not** change WM_INPUT, QPC, queue, integrity classification logic, or yaw constant.

---

## 9. HUD

Display at minimum:

- `PROCESSOR: none` (id)
- `PROCESSOR VERSION: 1.0.0`
- Note that processed == raw under `none`

Validation Lab controls and 360° procedure remain functional.

---

## 10. Testing

- Unit: `NoAcceleration` identity; factory accepts `"none"`, rejects unknown.
- Unit: processed sample fields populated correctly for identity case.
- DB: migrate creates `processed_mouse_events`; flush round-trip inserts paired raw+processed rows.
- Unit: `dt_s` for sample N uses `(T_n - T_{n-1}) / 1e9`; first sample convention documented; never uses render delta.
- Existing M1 math / integrity / validation tests remain green.
- Manual: Start → look → End still works; HUD shows processor; DB has processed rows with `processor_id=none`.

---

## 11. Explicitly Out of Scope (M2)

- Raw Accel / Linear / Classic / Natural / Power / LUT curves  
- Mid-session processor hot-swap  
- Pitch enablement  
- Aim tasks (STATIC_CLICK, etc.)  
- CSV/JSON export  
- Changing VSync back to uncapped  
- Silent yaw-constant compensation  

---

## 12. Success Criteria

- Processor selected only at session start; frozen while Running  
- Raw table unchanged; processed table populated with id/version/config per row  
- Yaw uses processed dx; under `none`, behavior matches M1  
- Baseline VSync ON retained  
- No Raw Accel math  
- Docs updated: `BASELINE.md` (if needed), `TELEMETRY_SCHEMA.md`, `ARCHITECTURE.md`, short `docs/M2_PROCESSOR.md`  

---

## 13. Stop After M2

After M2 lands and is verified, **STOP**.

Do not automatically implement M2.x (Raw Accel reproduction). That is a separate design + approval cycle.

---

## Revision Notes

- **2026-09-17:** Approach 1; processed table B with per-row processor identity; VSync ON lasting baseline (uncapped was temporary M1 test only).
- **2026-09-17:** `dt_s` must be derived from consecutive raw mouse QPC timestamps, not render-frame / FPS / VSync timing (required for M2.x velocity-dependent processors).

# Telemetry Schema (M1 + M2 + M3.x + M3.y + M4.a + Lab UI shell + Aim History replay + TRACKING v1/v2)

**Date:** 2026-09-21  
**M2:** adds `processed_mouse_events`; validation sessions use `experiment_version` `0.12.2`  
**M3.x:** adds `aim_trials` / `aim_shots` for completed aim runs (`experiment_id` = `aim_lab`)  
**M3.y:** adds `aim_target_events`, `aim_input_samples`, `aim_camera_samples`; expands `aim_trials` snapshot  
**M4.a:** `trial_type = GRIDSHOT` on the same five tables  
**Lab UI shell:** aim + validation runs at `0.10.0`+ use pause-excluded `duration_secs` / `score_secs` (not wall-clock span)  
**Aim History replay:** read-only reconstruct from existing five-table aim telemetry via `list_aim_trials_summary` / `load_aim_trial_bundle`; no schema bump  
**TRACKING v1:** `trial_type = TRACKING`; hold∧ray sample scoring (`score_secs` = on-target seconds; `accuracy` = `score_secs / active_duration_secs`); `direction_change` target events; **no** `aim_shots` rows; shipped at **`0.12.0`**  
**LCG full-unit fix:** STATIC_CLICK / GRIDSHOT `task_version` / `rng_version` **`"2"`** (full [0,1) LCG); TRACKING stayed `"1"` at **`0.12.1`**  
**TRACKING v2:** hold rapid-fire at 20 Hz → `aim_shots`; `accuracy` = `hits/shots`; `score_secs` remains on-target hold seconds (secondary); `task_version` **`"2"`**; new completed trials write at **`0.12.2`**  
**Database path:** `data/sense_maxer.db` (gitignored)  
**Write pattern:** in-memory buffers during `ValidationState::Running`; batched flush on End Validation inside a single transaction; never one transaction per mouse event.

---

## Raw vs Derived

| Category | Definition | Mutability | Examples |
|----------|------------|------------|----------|
| **Raw** | Direct from input hardware/OS path | Immutable after capture | `raw_mouse_events.dx`, `raw_mouse_events.dy`, timestamps, sequence numbers |
| **Derived** | Computed from raw + configuration | Stored separately from raw | eDPI, expected counts/360, validation error %, integrity counters |

Rules:

- Never mutate raw samples after capture.
- Derived metrics are written to separate columns/tables.
- **M2:** store both raw and processed counts in separate tables; under `none` (`NoAcceleration`), processed equals raw.
- Mouse, input-camera, and frame samples are buffered only while a validation session is running.

---

## M1 Tables

Schema is created by `TelemetryDb::migrate()` in `crates/sense-telemetry/src/db.rs`.

### `configurations`

Snapshot of experimental settings at session start. Full struct also stored as JSON in `snapshot_json`.

| Column | Type | Unit / notes |
|--------|------|--------------|
| `id` | TEXT PK | e.g. `config_000001` |
| `dpi` | REAL | counts/inch |
| `sensitivity` | REAL | game sensitivity |
| `edpi` | REAL | derived: DPI × sensitivity |
| `yaw_deg_per_count_at_sens_1` | REAL | 0.07 (UNCERTAIN provenance) |
| `fov_axis` | TEXT | `Horizontal` in M1 |
| `fov_degrees` | REAL | degrees |
| `width` | INTEGER | pixels |
| `height` | INTEGER | pixels |
| `refresh_hz` | REAL | Hz (0 if undetected) |
| `acceleration_enabled` | INTEGER | 0/1 |
| `acceleration_model` | TEXT | e.g. `none` |
| `polling_rate_hz` | REAL | optional |
| `snapshot_json` | TEXT | full `ConfigurationRecord` JSON |

---

### `sessions`

One row per Validation Lab session (Start → End).

| Column | Type | Unit / notes |
|--------|------|--------------|
| `id` | TEXT PK | e.g. `session_20260917_000001` |
| `configuration_id` | TEXT FK | → `configurations.id` |
| `app_version` | TEXT | binary version (`0.1.0`) |
| `experiment_id` | TEXT | `validation_lab` |
| `experiment_version` | TEXT | `0.12.1` for all new sessions, regardless of processor |
| `random_seed` | INTEGER | stored even if unused in M1 |
| `start_unix_ms` | INTEGER | wall clock (Unix ms) |
| `end_unix_ms` | INTEGER | wall clock, nullable until End |

---

### `raw_mouse_events`

Immutable raw input stream. Composite primary key per session.

| Column | Type | Unit / notes |
|--------|------|--------------|
| `session_id` | TEXT FK | → `sessions.id` |
| `sequence_number` | INTEGER | monotonic order per session |
| `timestamp_ns` | INTEGER | monotonic (QPC family) |
| `dx` | INTEGER | mouse counts |
| `dy` | INTEGER | mouse counts |
| `buttons` | INTEGER | button bitmask |

---

### `input_camera_samples`

**Input-derived** camera state immediately after applying one raw mouse sample. **Not** a render-frame pose.

| Column | Type | Unit / notes |
|--------|------|--------------|
| `session_id` | TEXT FK | → `sessions.id` |
| `sequence_number` | INTEGER | mouse sample sequence |
| `timestamp_ns` | INTEGER | same clock family as applied mouse sample |
| `yaw_deg` | REAL | degrees |
| `pitch_deg` | REAL | degrees (unchanged by mouse in M1) |

**Cadence:** one row per drained mouse sample while validation is running (same samples as `raw_mouse_events`).

Yaw is computed from **processed** horizontal counts; under `none`, processed equals raw so M1 behavior is preserved.

**Future:** `RenderCameraSample` (pose at render submit/present) will be a separate type/table — **not M1**.

---

### `processed_mouse_events` (M2)

Processor output paired with each raw sample. Composite primary key per session.

| Column | Type | Unit / notes |
|--------|------|--------------|
| `session_id` | TEXT FK | → `sessions.id` |
| `sequence_number` | INTEGER | pairs with `raw_mouse_events` |
| `timestamp_ns` | INTEGER | same as paired raw sample |
| `processed_dx` | REAL | after `InputProcessor::process` |
| `processed_dy` | REAL | after `InputProcessor::process` |
| `processor_id` | TEXT | e.g. `none` |
| `processor_version` | TEXT | e.g. `1.1.0` |
| `processor_config_json` | TEXT | reproducible config blob, e.g. `{}` |

**Cadence:** one row per drained mouse sample while validation is running (same sequence numbers as `raw_mouse_events`).

Processor identity is stored **per row** so the transformation remains reproducible even if session config is incomplete.

---

### `frame_samples`

Render-path timing; separate from input cadence. Recorded only during validation sessions.

| Column | Type | Unit / notes |
|--------|------|--------------|
| `id` | INTEGER PK | autoincrement |
| `session_id` | TEXT FK | → `sessions.id` |
| `timestamp_ns` | INTEGER | monotonic (QPC at frame record) |
| `frame_time_s` | REAL | seconds |
| `fps` | REAL | derived `1 / frame_time_s` |

Do not use frame index or FPS as a substitute for input timestamps.

---

### `validation_results`

Persisted outcome of End Validation, including math discrepancy and input-integrity report. One row per session.

| Column | Type | Unit / notes |
|--------|------|--------------|
| `session_id` | TEXT PK | → `sessions.id` |
| `expected_counts` | REAL | `counts_per_360(sensitivity)` |
| `observed_net_counts` | REAL | **signed** Σ `raw_dx` (net horizontal counts) |
| `observed_abs_path_counts` | REAL | Σ \|raw_dx\| — telemetry only, **not** used for validation math |
| `expected_degrees` | REAL | 360 |
| `observed_degrees` | REAL | Under `none`: raw-count-derived yaw; under acceleration: accumulated camera yaw delta |
| `count_difference` | REAL | Raw observed_net − raw expected counts (telemetry; not 360° proof under acceleration) |
| `error_percent` | REAL | Under `none`: count difference percentage; under acceleration: `(observed_degrees − 360) / 360 × 100` |
| `samples_received` | INTEGER | total raw samples in validation window |
| `sequence_gaps` | INTEGER | missing sequence numbers |
| `duplicate_sequences` | INTEGER | repeated sequence numbers |
| `out_of_order_samples` | INTEGER | sequence regressions |
| `timestamp_regressions` | INTEGER | `timestamp_ns` going backward |
| `pipeline_suspect` | INTEGER | 0/1 — **1 if any integrity counter > 0** |

If `pipeline_suspect = 1`, treat the run as **pipeline-suspect**: still show math discrepancy; do not auto-correct.

---

## Integrity → Pipeline-Suspect Rule

```
pipeline_suspect = (sequence_gaps > 0)
                OR (duplicate_sequences > 0)
                OR (out_of_order_samples > 0)
                OR (timestamp_regressions > 0)
```

Integrity counters distinguish input pipeline faults from sensitivity model mismatch.

---

## M3.y Aim Tables (five-table model)

Schema created by `TelemetryDb::migrate()` alongside M1/M2 tables. **Completed trials only** — aborted, cancelled, or in-progress runs are never inserted.

### Architecture

```
aim_trials                 ← immutable run snapshot (config + summary)
 ├── aim_target_events     ← target lifecycle (spawn/despawn/…)
 ├── aim_shots             ← player clicks (authoritative hit/miss)
 ├── aim_input_samples     ← raw + processed input stream
 └── aim_camera_samples    ← camera pose stream
```

All rows share one monotonic timing domain: `timestamp_ns` (QPC-backed). Reconstruct:

```
RAW INPUT → PROCESSOR → CAMERA → TARGET STATE → SHOT → HIT/MISS
```

**Lifecycle vs shots:** `aim_target_events` records what the target did (spawn, despawn, motion). `aim_shots` records what the player did (clicks). **`aim_shots.hit` owns click outcome** — do not store authoritative hit/miss on target events.

### `aim_trials`

One row per **completed** aim trial. Immutable experiment/run snapshot: everything needed to reproduce or interpret the trial lives on the row or in its JSON fields.

| Column | Type | Unit / notes |
|--------|------|--------------|
| `id` | TEXT PK | `aim_{utc_date}_{seq:06}` |
| `app_version` | TEXT | binary version |
| `experiment_id` | TEXT | `aim_lab` (distinct from `validation_lab`) |
| `experiment_version` | TEXT | **`0.12.1`** |
| `trial_type` | TEXT | `STATIC_CLICK`, `GRIDSHOT`, or `TRACKING` (task discriminator) |
| `status` | TEXT | always `completed` for inserted rows |
| `processor_id` | TEXT | snapshot at finish |
| `processor_version` | TEXT | snapshot at finish |
| `processor_config_json` | TEXT | full processor config (not merely id) |
| `dpi` | REAL | counts/inch |
| `sensitivity` | REAL | game sensitivity |
| `polling_rate_hz` | REAL | Hz |
| `fov_degrees_h` | REAL | horizontal FOV (degrees) |
| `pitch_model_id` | TEXT | e.g. `unverified_0.1` |
| `pitch_model_version` | TEXT | pitch model version |
| `pitch_config_json` | TEXT | yaw/pitch constants, sign, clamp |
| `resolution_width` | INTEGER | pixels at finish |
| `resolution_height` | INTEGER | pixels at finish |
| `aspect_ratio` | REAL | runtime width/height |
| `random_seed` | INTEGER | reproducibility handle (u64), assigned at **Start** |
| `task_version` | TEXT | bump when task/RNG semantics change (`"2"` = full-unit LCG for STATIC_CLICK/GRIDSHOT; TRACKING `"2"` = hold rapid-fire) |
| `hardware_config_json` | TEXT | slower-moving env (mouse, display, …) |
| `view_config_json` | TEXT | projection, VFOV, present mode, … |
| `task_config_json` | TEXT | type-specific knobs incl. `rng`, `rng_version` |
| `metrics_json` | TEXT | light extras (`{}` for STATIC_CLICK v1) |
| `start_unix_ms` / `end_unix_ms` | INTEGER | wall clock (Unix ms) |
| `start_timestamp_ns` / `end_timestamp_ns` | INTEGER | monotonic ns (score clock) |
| `duration_secs` | REAL | pause-excluded active seconds (matches `score_secs` at finish) |
| `hits` | INTEGER | successful hits (TRACKING v1: 0; TRACKING v2: rapid-fire hits) |
| `shots` | INTEGER | all clicks / virtual shots (TRACKING v1: 0; TRACKING v2: rapid-fire shots) |
| `misses` | INTEGER | `shots - hits` |
| `score_secs` | REAL | pause-excluded active time to finish (STATIC_CLICK/GRIDSHOT) or on-target seconds (TRACKING) |
| `accuracy` | REAL | `hits / shots` (STATIC_CLICK/GRIDSHOT/TRACKING v2; 0 if no shots). TRACKING v1 rows: `score_secs / active_duration_secs` |

#### Seed semantics

| Concept | Role |
|---------|------|
| `random_seed` | Persisted reproducibility handle, chosen at **Start** |
| `rng` + `rng_version` in `task_config_json` | RNG implementation identity |
| `task_version` | Bump when generation semantics change so same seed ≠ false equivalence |

```
random_seed + task_version + rng/rng_version + task_config
  → deterministic target sequence
```

Internal LCG state is runtime-only — **not** the persisted reproducibility mechanism.

Trial + all child rows insert in a **single transaction**; failure leaves no partial rows.

### `aim_target_events`

Target lifecycle stream, independent of player clicks.

| Column | Type | Unit / notes |
|--------|------|--------------|
| `trial_id` | TEXT FK | → `aim_trials.id` |
| `target_id` | TEXT | stable instance id: `target_001`, … |
| `event_index` | INTEGER | 0-based order in trial stream |
| `timestamp_ns` | INTEGER | monotonic |
| `event_type` | TEXT | `spawn`, `despawn`, `direction_change`, `appear`, `disappear` |
| `position_x/y/z` | REAL | world position |
| `yaw_deg` / `pitch_deg` | REAL | target angles from camera (nullable) |
| `velocity_x/y/z` | REAL | motion; `0` for STATIC_CLICK |
| `event_data_json` | TEXT | extensible (e.g. `radius`) |

Primary key: `(trial_id, event_index)`.

**STATIC_CLICK v1 pattern:** per target instance → `spawn` → (miss shots have no target-event) → hit shot + `despawn`.

**TRACKING v1 pattern:** one target for the run → `spawn` → repeated `direction_change` (RNG reverse or wall bounce) → `despawn`. No `aim_shots` rows; score accumulates on held+hit samples only.

**TRACKING v2 pattern:** same motion/events as v1, plus `aim_shots` at 20 Hz while LMB held (`scoring_rule: hold_rapid_fire`); `accuracy` = hits/shots; `score_secs` still on-target hold seconds.

### `aim_shots`

Player clicks; owns authoritative hit/miss.

| Column | Type | Unit / notes |
|--------|------|--------------|
| `trial_id` | TEXT FK | → `aim_trials.id` |
| `shot_index` | INTEGER | 0-based order within trial |
| `target_id` | TEXT | matches `aim_target_events.target_id` |
| `timestamp_ns` | INTEGER | monotonic shot time |
| `hit` | INTEGER | 0/1 — **authoritative** |
| `yaw_deg` / `pitch_deg` | REAL | look at shot |
| `target_x` / `target_y` / `target_z` | REAL | sphere center at click |
| `target_radius` | REAL | radius used for hit test |

Primary key: `(trial_id, shot_index)`.

### `aim_input_samples`

Lossless raw + processed input stream for the armed trial. Buffered in memory; flushed on completed write only.

| Column | Type | Unit / notes |
|--------|------|--------------|
| `trial_id` | TEXT FK | → `aim_trials.id` |
| `sequence_number` | INTEGER | per-trial monotonic |
| `timestamp_ns` | INTEGER | **actual QPC** timestamp of raw sample |
| `raw_dx` / `raw_dy` | INTEGER | WM_INPUT counts |
| `processed_dx` / `processed_dy` | REAL | after `InputProcessor::process` |
| `dt_ns` | INTEGER | **raw** QPC interval to previous sample (`timestamp_ns − prev`); first sample: `0` |
| `dt_used_ns` | INTEGER | processor **effective** interval after speed-path floor/clamp |
| `input_speed` | REAL | computed using **`dt_used_ns`** (nullable) |
| `acceleration_scale` | REAL | Linear scale when available; `1.0` / NULL for `none` |

Primary key: `(trial_id, sequence_number)`.

#### `dt_ns` vs `dt_used_ns` (locked)

```
timestamp_ns  = actual QPC timestamp of the raw sample
dt_ns         = raw QPC interval to previous raw sample
dt_used_ns    = processor effective interval after polling-rate floor / clamp
input_speed   = f(raw_dx, raw_dy, dt_used_ns)   // never use clamped value as dt_ns
```

**Do not** store the clamped interval in `dt_ns`. Raw timing stays permanent; processor behavior stays reproducible via `dt_used_ns` + frozen `processor_config_json` on `aim_trials`.

For `rawaccel_linear`, `dt_used_ns` follows `sense-accel::clamp_speed_dt_ms`: clamp `dt_ns` between `1000/polling_rate_hz` (or RA default min) and RA max. If clamp formula changes, bump `processor_version` so old rows remain interpretable.

Processor identity is **not** repeated per input row — frozen on `aim_trials`.

### `aim_camera_samples`

Camera pose after each input apply; separates processor output from camera mapping.

| Column | Type | Unit / notes |
|--------|------|--------------|
| `trial_id` | TEXT FK | → `aim_trials.id` |
| `timestamp_ns` | INTEGER | same monotonic domain |
| `yaw_deg` / `pitch_deg` | REAL | absolute pose after apply |
| `yaw_delta_deg` / `pitch_delta_deg` | REAL | delta from previous sample (`0` for first) |

Primary key: `(trial_id, timestamp_ns)`.

### Write rules (M3.y)

1. **Armed:** buffer input, camera, target events, shots; assign `random_seed` at Start; deterministic spawns from seed.
2. **Completed (Nth required hit):** single transaction inserting `aim_trials` + all child rows. All-or-nothing.
3. **Abort / Cancel / Start Validation / quit while Armed:** discard buffers; **no** DB rows.
4. Validation Lab session tables remain for 360° validation; aim runs use **aim_*** tables (no active validation session required).

**Inspect after a run:**

```bash
sqlite3 data/sense_maxer.db "SELECT id, hits, shots, score_secs, accuracy, random_seed, status FROM aim_trials ORDER BY end_unix_ms;"
sqlite3 data/sense_maxer.db "SELECT COUNT(*) FROM aim_target_events WHERE trial_id='aim_YYYYMMDD_000001';"
sqlite3 data/sense_maxer.db "SELECT COUNT(*) FROM aim_shots WHERE trial_id='aim_YYYYMMDD_000001';"
sqlite3 data/sense_maxer.db "SELECT COUNT(*) FROM aim_input_samples WHERE trial_id='aim_YYYYMMDD_000001';"
sqlite3 data/sense_maxer.db "SELECT COUNT(*) FROM aim_camera_samples WHERE trial_id='aim_YYYYMMDD_000001';"
sqlite3 data/sense_maxer.db "SELECT sequence_number, dt_ns, dt_used_ns FROM aim_input_samples WHERE trial_id='aim_YYYYMMDD_000001' LIMIT 5;"
```

Design: [2026-09-19-m3y-aim-telemetry-data-model-design.md](./superpowers/specs/2026-09-19-m3y-aim-telemetry-data-model-design.md). Predecessor: [2026-09-18-m3x-aim-trial-persistence-design.md](./superpowers/specs/2026-09-18-m3x-aim-trial-persistence-design.md).

---

## M3.x Aim Tables (superseded by M3.y)

M3.x introduced `aim_trials` + `aim_shots` only. M3.y extends the snapshot columns and adds three child streams. Existing `0.7.0`–`0.12.1` rows remain readable after migration; new completed trials write at **`0.12.2`** (STATIC_CLICK, GRIDSHOT, or TRACKING) with full child streams and pause-excluded duration fields. STATIC_CLICK/GRIDSHOT rows at `task_version` `"1"` used half-range LCG; `"2"` is full-unit. TRACKING v1 (`task_version` `"1"`) omits `aim_shots`; TRACKING v2 writes rapid-fire `aim_shots`.

---

## Legacy M3.x reference (pre-M3.y)

<details>
<summary>Original two-table M3.x documentation (historical)</summary>

### `aim_trials` (M3.x)

One row per **completed** aim trial. `trial_type` discriminates task kind (`STATIC_CLICK` now; future types reuse this table).

| Column | Type | Unit / notes |
|--------|------|--------------|
| `id` | TEXT PK | `aim_{utc_date}_{seq:06}` |
| `app_version` | TEXT | binary version |
| `experiment_id` | TEXT | `aim_lab` (distinct from `validation_lab`) |
| `experiment_version` | TEXT | `0.7.0` (legacy rows) |
| `trial_type` | TEXT | e.g. `STATIC_CLICK` |
| `status` | TEXT | always `completed` for inserted rows |
| `processor_id` | TEXT | snapshot at finish |
| `processor_version` | TEXT | snapshot at finish |
| `processor_config_json` | TEXT | snapshot at finish |
| `dpi` | REAL | counts/inch |
| `sensitivity` | REAL | game sensitivity |
| `polling_rate_hz` | REAL | Hz |
| `fov_degrees_h` | REAL | degrees |
| `task_config_json` | TEXT | type-specific knobs (see design spec) |
| `metrics_json` | TEXT | type-specific extras (`{}` for STATIC_CLICK v1) |
| `start_unix_ms` / `end_unix_ms` | INTEGER | wall clock (Unix ms) |
| `start_timestamp_ns` / `end_timestamp_ns` | INTEGER | monotonic ns (score clock) |
| `duration_secs` | REAL | pause-excluded active seconds (matches `score_secs` at finish) |
| `hits` | INTEGER | successful hits |
| `shots` | INTEGER | all clicks (hits + misses) |
| `misses` | INTEGER | `shots - hits` |
| `score_secs` | REAL | pause-excluded active time to finish (HUD score) |
| `accuracy` | REAL | `hits / shots` |

### `aim_shots` (M3.x)

Optional per-click rows for click-style tasks.

| Column | Type | Unit / notes |
|--------|------|--------------|
| `trial_id` | TEXT FK | → `aim_trials.id` |
| `shot_index` | INTEGER | 0-based order within trial |
| `timestamp_ns` | INTEGER | monotonic shot time |
| `hit` | INTEGER | 0/1 |
| `yaw_deg` / `pitch_deg` | REAL | look at shot |
| `target_x` / `target_y` / `target_z` | REAL | sphere center |
| `target_radius` | REAL | radius used for hit test |

</details>

---

## Future Tables (Post-M3.y)

Documented for later phases; **not implemented**:

| Table | Purpose |
|-------|---------|
| `movements` | Segmented movement episodes |
| `performance_metrics` | Aggregated performance stats (compute offline from M3.y streams) |
| `users` | Multi-user research |
| `devices` | Hardware profiles |
| `render_camera_samples` | Pose at render submit/present |

Derived ML metrics (overshoot, jitter, path efficiency, RMS error, …) are **deferred** — recompute offline from trajectories + target geometry. M3.y stores lossless raw/event streams only.

CSV/JSON export is also out of scope; data must be queryable via SQLite.

---

## Units Summary

| Quantity | Unit |
|----------|------|
| dx, dy | mouse counts |
| timestamp | nanoseconds (monotonic for samples) |
| yaw, pitch, FOV | degrees |
| DPI | counts/inch |
| cm/360 | centimeters |
| error_percent | percent |

---

## Related Documents

- [ARCHITECTURE.md](./ARCHITECTURE.md) — batching and input vs render streams
- [M2_PROCESSOR.md](./M2_PROCESSOR.md) — processor pipeline, `dt_s` rules, manual checklist
- [EXPERIMENT_MODEL.md](./EXPERIMENT_MODEL.md) — Validation Lab flow and signed net counts
- [VALORANT_INPUT_MODEL.md](./VALORANT_INPUT_MODEL.md) — formulas for derived validation fields

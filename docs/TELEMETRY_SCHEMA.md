# Telemetry Schema (M1 + M2 + M3.x)

**Date:** 2026-09-18  
**M2:** adds `processed_mouse_events`; validation sessions use `experiment_version` `0.7.0`  
**M3.x:** adds `aim_trials` / `aim_shots` for completed aim runs (`experiment_id` = `aim_lab`)
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
| `experiment_version` | TEXT | `0.7.0` for all new sessions, regardless of processor |
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

## M3.x Aim Tables

Schema created by `TelemetryDb::migrate()` alongside M1/M2 tables. **Completed trials only** — aborted or in-progress runs are never inserted.

### `aim_trials`

One row per **completed** aim trial. `trial_type` discriminates task kind (`STATIC_CLICK` now; future types reuse this table).

| Column | Type | Unit / notes |
|--------|------|--------------|
| `id` | TEXT PK | `aim_{utc_date}_{seq:06}` |
| `app_version` | TEXT | binary version |
| `experiment_id` | TEXT | `aim_lab` (distinct from `validation_lab`) |
| `experiment_version` | TEXT | `0.7.0` |
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
| `duration_secs` | REAL | seconds |
| `hits` | INTEGER | successful hits |
| `shots` | INTEGER | all clicks (hits + misses) |
| `misses` | INTEGER | `shots - hits` |
| `score_secs` | REAL | time to required hits (HUD score) |
| `accuracy` | REAL | `hits / shots` |

Trial + shots insert in a **single transaction**; failure leaves no partial rows.

### `aim_shots`

Optional per-click rows for click-style tasks. Skipped by trial types that do not use shots.

| Column | Type | Unit / notes |
|--------|------|--------------|
| `trial_id` | TEXT FK | → `aim_trials.id` |
| `shot_index` | INTEGER | 0-based order within trial |
| `timestamp_ns` | INTEGER | monotonic shot time |
| `hit` | INTEGER | 0/1 |
| `yaw_deg` / `pitch_deg` | REAL | look at shot |
| `target_x` / `target_y` / `target_z` | REAL | sphere center |
| `target_radius` | REAL | radius used for hit test |

Primary key: `(trial_id, shot_index)`.

**Inspect after a run:**

```bash
sqlite3 data/sense_maxer.db "SELECT id, hits, shots, score_secs, accuracy, status FROM aim_trials ORDER BY end_unix_ms;"
sqlite3 data/sense_maxer.db "SELECT COUNT(*) FROM aim_shots WHERE trial_id='aim_YYYYMMDD_000001';"
```

Design: [2026-09-18-m3x-aim-trial-persistence-design.md](./superpowers/specs/2026-09-18-m3x-aim-trial-persistence-design.md).

---

## Future Tables (Post-M3.x)

Documented for later phases; **not implemented**:

| Table | Purpose |
|-------|---------|
| `movements` | Segmented movement episodes |
| `task_events` | Task lifecycle markers |
| `performance_metrics` | Aggregated performance stats |
| `users` | Multi-user research |
| `devices` | Hardware profiles |
| `render_camera_samples` | Pose at render submit/present |

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

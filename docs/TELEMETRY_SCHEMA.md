# Telemetry Schema (M1)

**Date:** 2026-09-17  
**Storage:** SQLite under `data/` (gitignored)  
**Write pattern:** batched inserts inside transactions; never one transaction per mouse event.

---

## Raw vs Derived

| Category | Definition | Mutability | Examples |
|----------|------------|------------|----------|
| **Raw** | Direct from input hardware/OS path | Immutable after capture | `raw_mouse_events.dx`, `raw_mouse_events.dy`, timestamps, sequence numbers |
| **Derived** | Computed from raw + configuration | Stored separately from raw | eDPI, expected counts/360, validation error %, integrity counters |

Rules:

- Never mutate raw samples after capture.
- Derived metrics are written to separate columns/tables.
- When acceleration is added later, store both raw and processed counts; M1 processed equals raw under `NoAcceleration`.

---

## M1 Tables

### `configurations`

Snapshot of experimental settings. Referenced by sessions; do not rely on “current settings” to reconstruct history.

| Column | Type | Unit / notes |
|--------|------|--------------|
| `config_id` | TEXT PK | e.g. `config_NNNNNN` |
| `created_at` | TEXT | wall-clock ISO8601 |
| `app_version` | TEXT | binary version |
| `experiment_id` | TEXT | `validation_lab` |
| `experiment_version` | TEXT | e.g. `0.1.0` |
| `dpi` | INTEGER | counts/inch |
| `sensitivity` | REAL | game sensitivity |
| `edpi` | REAL | derived: DPI × sensitivity |
| `fov_axis` | TEXT | `Horizontal` in M1 |
| `fov_degrees` | REAL | degrees |
| `resolution_w` | INTEGER | pixels |
| `resolution_h` | INTEGER | pixels |
| `refresh_hz` | REAL | Hz, if detected |
| `accel_state` | TEXT | e.g. `NoAcceleration` |
| `polling_rate_hz` | INTEGER | optional, if known |
| `random_seed` | INTEGER | stored even if unused in M1 |
| `yaw_constant` | REAL | 0.07 (UNCERTAIN provenance) |

---

### `sessions`

One row per Validation Lab session (Start → End).

| Column | Type | Unit / notes |
|--------|------|--------------|
| `session_id` | TEXT PK | e.g. `session_YYYYMMDD_NNNNNN` |
| `config_id` | TEXT FK | → `configurations` |
| `started_at` | TEXT | wall clock |
| `ended_at` | TEXT | wall clock, nullable until End |
| `notes` | TEXT | optional |

---

### `raw_mouse_events`

Immutable raw input stream.

| Column | Type | Unit / notes |
|--------|------|--------------|
| `id` | INTEGER PK | autoincrement |
| `session_id` | TEXT FK | → `sessions` |
| `timestamp_ns` | INTEGER | monotonic (QPC family) |
| `sequence_number` | INTEGER | monotonic order per session |
| `dx` | INTEGER | mouse counts |
| `dy` | INTEGER | mouse counts |
| `buttons` | INTEGER | button bitmask |

Preserve additional platform fields if available in later schema revisions.

---

### `input_camera_samples`

**Input-derived** camera state immediately after applying one raw mouse sample. **Not** a render-frame pose.

| Column | Type | Unit / notes |
|--------|------|--------------|
| `id` | INTEGER PK | autoincrement |
| `session_id` | TEXT FK | → `sessions` |
| `timestamp_ns` | INTEGER | same clock family as applied mouse sample |
| `sequence_number` | INTEGER | mouse sample sequence (when applicable) |
| `yaw_deg` | REAL | degrees |
| `pitch_deg` | REAL | degrees (unchanged by mouse in M1) |
| `event_kind` | TEXT | e.g. `mouse_sample`, `camera_reset`, `counter_reset` |

**Cadence:** one row per drained mouse sample that updates yaw, plus rows on camera/counter reset.

**Future:** `RenderCameraSample` (pose at render submit/present) will be a separate type/table — **not M1**.

---

### `frame_samples`

Render-path timing; separate from input cadence.

| Column | Type | Unit / notes |
|--------|------|--------------|
| `id` | INTEGER PK | autoincrement |
| `session_id` | TEXT FK | → `sessions` |
| `timestamp_ns` | INTEGER | monotonic |
| `frame_index` | INTEGER | render frame counter |
| `delta_ms` | REAL | optional frame delta |

Do not use frame index or FPS as a substitute for input timestamps.

---

### `validation_results`

Persisted outcome of End Validation, including math discrepancy and input-integrity report.

| Column | Type | Unit / notes |
|--------|------|--------------|
| `id` | INTEGER PK | autoincrement |
| `session_id` | TEXT FK | → `sessions` unique per validation |
| `ended_at` | TEXT | wall clock |

**Math fields (derived via `sense-math`):**

| Column | Type | Unit / notes |
|--------|------|--------------|
| `expected_counts` | REAL | 360 / (sensitivity × 0.07) |
| `observed_counts` | REAL | **signed** Σ `raw_dx` (net horizontal counts) |
| `absolute_path_counts` | REAL | Σ \|raw_dx\| — telemetry only, **not** used for validation math |
| `expected_degrees` | REAL | 360 |
| `observed_degrees` | REAL | derived from signed net counts |
| `difference_degrees` | REAL | observed − expected |
| `error_percent` | REAL | relative error |

**Integrity fields (pipeline health):**

| Column | Type | Unit / notes |
|--------|------|--------------|
| `samples_received` | INTEGER | total raw samples in validation window |
| `sequence_gaps` | INTEGER | missing sequence numbers |
| `duplicate_sequences` | INTEGER | repeated sequence numbers |
| `out_of_order_samples` | INTEGER | sequence regressions |
| `timestamp_regressions` | INTEGER | timestamp_ns going backward |
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

## Future Tables (Non-M1)

Documented for later phases; **not implemented in M1**:

| Table | Purpose |
|-------|---------|
| `trials` | Discrete aim/task trials |
| `movements` | Segmented movement episodes |
| `task_events` | Task lifecycle markers |
| `performance_metrics` | Aggregated performance stats |
| `users` | Multi-user research |
| `devices` | Hardware profiles |
| `render_camera_samples` | Pose at render submit/present |

CSV/JSON export is also out of M1 scope; data must be queryable via SQLite.

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
- [EXPERIMENT_MODEL.md](./EXPERIMENT_MODEL.md) — Validation Lab flow and signed net counts
- [VALORANT_INPUT_MODEL.md](./VALORANT_INPUT_MODEL.md) — formulas for derived validation fields

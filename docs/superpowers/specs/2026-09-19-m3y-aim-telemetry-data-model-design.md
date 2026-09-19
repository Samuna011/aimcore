# M3.y — Aim Telemetry Data Model (Task Framework Foundation) — Design Spec

**Date:** 2026-09-19  
**Status:** Approved for implementation (user confirmed architecture + write depth **I**)  
**Extends:** M3.x aim persistence (`0.7.0` → this work bumps to **`0.8.0`**)  
**Predecessor:** `2026-09-18-m3x-aim-trial-persistence-design.md`

## Goal

Evolve the aim persistence layer into a **reconstructable, ML-ready telemetry model** for the future task framework **before** Gridshot / 1wall6 / tracking gameplay.

**Key principle:** Never record only the final score. Record the complete sequence of events and enough state to reconstruct what happened.

**Locked rule:** Raw telemetry is **lossless and permanent**. Derived metrics (overshoot, jitter, path efficiency, RMS error, …) are **disposable / recomputable** — not primary stored architecture.

## Non-goals

- New task types (Gridshot, 1wall6, tracking gameplay)
- Derived-metrics / ML feature tables
- Redesigning Validation Lab `sessions` / stuffing aim into those tables
- Authoritative hit/miss on target lifecycle events (clicks own that)
- Precomputing hundreds of analysis features at write time

## Architecture overview

Five persistable layers (plus deferred derived):

```
aim_trials                    ← immutable experiment/run snapshot
 ├── aim_target_events        ← what the target did
 ├── aim_shots                 ← what the player did (clicks)
 ├── aim_input_samples         ← raw + processed input stream
 └── aim_camera_samples       ← camera pose stream
```

All event/sample rows share one **monotonic** timing domain: `timestamp_ns` (QPC-backed). Reconstruct:

```
RAW INPUT → PROCESSOR → CAMERA → TARGET STATE → SHOT → HIT/MISS
```

Evolve existing tables; do not greenfield `experiments/runs` rename in this milestone.

---

## 1. Run-level configuration (`aim_trials`)

Treat each completed `aim_trials` row as an **immutable experiment/run snapshot**: everything needed to reproduce or interpret the trial lives on the row or in its structured JSON fields.

### Explicit columns (query / group constantly)

| Column | Notes |
|--------|--------|
| `processor_id` | Existing |
| `processor_version` | Existing |
| `processor_config_json` | Existing — **full** Linear (or none) config, not merely the id |
| `dpi` | Existing |
| `sensitivity` | Existing (`game_sensitivity`) |
| `polling_rate_hz` | Existing |
| `fov_degrees_h` | Existing (rename in docs to fov horizontal; column may stay `fov_degrees_h`) |
| `pitch_model_id` | **New** — e.g. `unverified_0.1` |
| `pitch_model_version` | **New** |
| `pitch_config_json` | **New** — e.g. shared 0.07, +dy look down, ±89 clamp, UNCERTAIN |
| `resolution_width` | **New** |
| `resolution_height` | **New** |
| `aspect_ratio` | **New** — **runtime** aspect (store even if mathematically derivable) |
| `random_seed` | **New** — reproducibility handle (u64) |
| `trial_type` | Existing discriminator (`STATIC_CLICK`; later `FLICK`, …) — alias “task_type” in docs |
| `task_version` | **New** — bump when task/RNG semantics change |
| `task_config_json` | Existing — include `rng`, `rng_version`, cone knobs, etc. |
| `hardware_config_json` | **New** — slower-moving env |
| `view_config_json` | **New** — projection, VFOV, present mode, etc. |

Keep existing result/timing columns: `status=completed` only, hits/shots/misses, score_secs, accuracy, unix + QPC timestamps, duration, app/experiment ids, `metrics_json` (still `{}` or light extras — not derived ML dumps).

### Example structured JSON

**`processor_config_json` (Linear):**
```json
{
  "acceleration": 0.007,
  "sensitivity_multiplier": 1.0,
  "gain": true,
  "cap_x": 2.0,
  "cap_y": 2.0,
  "polling_rate_hz": 1000
}
```
(Exact keys must match live `RawAccelLinear` `config_json()`; include any `dt` floor semantics already encoded via `polling_rate_hz`.)

**`hardware_config_json`:**
```json
{
  "mouse_model": "",
  "mouse_connection": "",
  "firmware": "",
  "display_refresh_hz": null
}
```
Empty/unknown allowed; fields evolve without migration.

**`view_config_json`:**
```json
{
  "projection": "perspective",
  "horizontal_fov_deg": 103.0,
  "vertical_fov_deg": null,
  "camera_mode": "yaw_pitch",
  "presentation_mode": "AutoNoVsync"
}
```

**`pitch_config_json`:**
```json
{
  "yaw_deg_per_count_at_sens_1": 0.07,
  "pitch_deg_per_count_at_sens_1": 0.07,
  "pitch_sign": "+dy_look_down",
  "pitch_clamp_deg": 89.0,
  "certainty": "UNCERTAIN"
}
```

**`task_config_json` (STATIC_CLICK):**
```json
{
  "hits_required": 5,
  "target_radius": 0.25,
  "aim_distance": 10.0,
  "yaw_half_deg": 25.0,
  "pitch_up_deg": 12.0,
  "pitch_down_deg": 5.0,
  "floor_clearance": 0.35,
  "rng": "lcg",
  "rng_version": "1"
}
```

### Seed semantics

| Concept | Role |
|---------|------|
| `random_seed` | Persisted reproducibility handle, chosen at **Start** |
| RNG algorithm + `rng_version` in `task_config_json` | Implementation identity |
| `task_version` | Bump when generation/semantics change so same seed ≠ false equivalence |
| Internal LCG state | Runtime only — **not** the persisted reproducibility mechanism |

```
random_seed + task_version + rng/rng_version + task_config
  → deterministic target sequence
```

At Start: assign `random_seed` (e.g. from QPC/entropy), initialize RNG from that seed, clear prior run buffers. Do **not** only XOR scramble an opaque LCG state without storing the seed.

---

## 2. Target lifecycle (`aim_target_events`)

Independent of player clicks.

| Column | Notes |
|--------|--------|
| `trial_id` | FK |
| `target_id` | Stable instance id within trial: `target_001`, `target_002`, … |
| `event_index` | 0-based order in the trial’s target-event stream |
| `timestamp_ns` | Same monotonic domain |
| `event_type` | See below |
| `position_x/y/z` | World position |
| `yaw_deg` / `pitch_deg` | Target angles from camera origin (nullable if N/A) |
| `velocity_x/y/z` | Motion; `0` for STATIC_CLICK |
| `event_data_json` | Extensible (e.g. `radius`, future flags) |

### Event types (v1 vocabulary)

| Type | Use |
|------|-----|
| `spawn` | Target instance created |
| `despawn` | Target instance removed |
| `direction_change` | Moving targets (unused in STATIC_CLICK v1) |
| `appear` | Become visible without new instance (optional later) |
| `disappear` | Hide without destroy (optional later) |

**Do not** use `hit` / `miss` as authoritative results on this table. **`aim_shots.hit` owns click outcome.**

### STATIC_CLICK v1 pattern

For each target instance: `spawn` (geometry + radius in `event_data_json`) → zero or more miss **shots** (no miss target-event) → on destroy: hit **shot** + `despawn`.

---

## 3. Player clicks (`aim_shots`)

Keep and extend. Owns click result.

| Column | Notes |
|--------|--------|
| `trial_id` | FK |
| `shot_index` | Existing |
| `target_id` | **New** — matches `aim_target_events.target_id` |
| `timestamp_ns` | Existing |
| `hit` | Existing — authoritative |
| `yaw_deg` / `pitch_deg` | Crosshair / camera at click |
| `target_x/y/z` | Target snapshot at click |
| `target_radius` | Snapshot at click |

```
target_events → what the target was doing
aim_shots     → what the player did
```

---

## 4. Continuous input (`aim_input_samples`)

Most important stream for future analysis. Buffer for the whole Armed trial; flush on completed write only.

| Column | Notes |
|--------|--------|
| `trial_id` | FK |
| `timestamp_ns` | Sample time |
| `sequence_number` | Per-trial monotonic |
| `raw_dx` / `raw_dy` | WM_INPUT counts |
| `processed_dx` / `processed_dy` | After processor |
| `dt_ns` | Interval used for this sample (from QPC deltas) |
| `input_speed` | Counts/ms (or agreed unit) after dt clamp when available; NULL if N/A |
| `acceleration_scale` | Linear scale factor when available; `1.0` / NULL for `none` |

Do **not** repeat full `processor_id` on every row (frozen on `aim_trials`).

Optional later (not required v1 columns): `effective_gain`, `cap_x_applied`, `cap_y_applied`, `dt_used` if not already recoverable from `dt_ns` + config — prefer extending only when Linear debug state already exposes them cheaply; otherwise keep regenerable from raw+config.

---

## 5. Continuous camera (`aim_camera_samples`)

Separates mouse/processor output from camera mapping (critical for sensitivity research).

| Column | Notes |
|--------|--------|
| `trial_id` | FK |
| `timestamp_ns` | Same domain |
| `yaw_deg` / `pitch_deg` | Absolute pose after apply |
| `yaw_delta_deg` / `pitch_delta_deg` | Delta from previous sample (0 for first) |

---

## 6. Derived metrics — deferred

**Do not** store as primary architecture fields:

underflick, overflick, overshoot, undershoot, path_efficiency, wiggle_score, correction_count, jitter_score, smoothness_score, movement_efficiency, reaction_time, acquisition_time, angular_error, RMS_error, high_frequency_jitter, …

Compute offline from trajectories + target geometry later.

Trial-level `score_secs` / `accuracy` / hits/shots remain as **lightweight run summaries** already in M3.x (not ML feature dumps).

---

## Write rules

1. **Armed:** buffer input samples, camera samples, target events, shots in memory; assign `random_seed` at Start; deterministic spawns from seed.
2. **Completed (Nth required hit):** single SQLite transaction inserting `aim_trials` (full snapshot) + all buffered child rows. All-or-nothing.
3. **Abort / Cancel / Start Validation / quit while Armed:** discard buffers; **no** DB rows; never present incomplete as completed.
4. **Trial id:** keep `aim_{utc_date}_{seq:06}` with **max suffix + 1** inside txn (not `COUNT(*)+1`).
5. **Wall clock:** refuse Start if `unix_time_ms` fails (existing M3.x guard).
6. Validation Lab raw/processed session tables remain for 360° validation; aim runs use **aim_*** sample tables (do not require an active validation session).

### Implementation depth (locked)

**I:** Migrate all five tables + STATIC_CLICK write path buffers/flushes config, target_events, shots, input_samples, and camera_samples on completed trial only.

---

## Types / API (sketch)

Extend `sense-types` with:

- Expanded `AimTrialRecord` fields listed above  
- `AimShotRecord.target_id: String`  
- `AimTargetEventRecord`  
- `AimInputSampleRecord`  
- `AimCameraSampleRecord`  

`TelemetryDb::insert_completed_aim_trial(...)` expands to accept the additional slices and insert them in the same txn (or a renamed `insert_completed_aim_run` wrapping the same behavior — prefer extending the existing API to avoid dual paths).

Migrate: `CREATE TABLE IF NOT EXISTS` / `ALTER TABLE` additive columns for existing `aim_trials` / `aim_shots` on upgrade (compatible with existing `0.7.0` DBs).

---

## Version / docs

- `EXPERIMENT_VERSION` → **`0.8.0`**
- Update `BASELINE.md`, `TELEMETRY_SCHEMA.md`, README M3.y / telemetry-model status
- Stop after this milestone: **no** new aim tasks until a task-specific spec

---

## Testing / validation

- DB: completed insert round-trips all five tables; duplicate/failure mid-txn rolls back all; abort path inserts nothing
- Unit: same `random_seed` + task_config → same first N spawn centers; seed persisted on completed record
- `cargo test` green (sense-maxer + sense-telemetry)
- Manual: complete a STATIC_CLICK run; confirm HUD summary matches trial row; child counts non-zero for events/shots/input/camera; cancel mid-run → no new completed trial

## Stop criteria

- Spec implemented for STATIC_CLICK with full snapshot + four child streams  
- Lossless raw/event principle documented and followed  
- Derived metrics not primary columns  
- Stop — next work is task designs (Gridshot/etc.) **on top of** this model, not before

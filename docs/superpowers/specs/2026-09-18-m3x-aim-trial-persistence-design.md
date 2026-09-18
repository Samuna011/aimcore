# M3.x — Aim Trial SQLite Persistence — Design Spec

**Date:** 2026-09-18  
**Status:** Approved for implementation (user confirmed schema + HUD/flow)  
**Extends:** M3 / M3.1 STATIC_CLICK (`0.6.x`)  
**Experiment version after ship:** `0.7.0`

## Goal

Persist completed aim trials to SQLite in a dedicated aim layer so HUD-reported results can be checked in the DB. Keep raw and processed mouse architecture unchanged. Stop after this milestone (no new trial types).

## Non-goals

- New trial types (FLICK / TRACK / etc.) — schema only prepared for them
- Stuffing aim fields into `sessions`, `raw_mouse_events`, `processed_mouse_events`, or `validation_results`
- Aim query / history UI beyond last-saved status on the HUD
- Persisting aborted or in-progress trials as completed results

## Architecture

**Shared aim layer, type-discriminated**

One persistence API and two tables serve all current and future aim tasks:

| Piece | Role |
|-------|------|
| `aim_trials` | One row per **completed** trial; `trial_type` discriminates task kind |
| `aim_shots` | Optional per-click rows for click-style tasks; skipped by types that do not use shots |
| `trial_type` | Stable string (`STATIC_CLICK` now; later `FLICK`, `TRACK`, …) |
| `task_config_json` | Full type-specific knobs (no per-type migration when possible) |
| `metrics_json` | Optional type-specific extras (unused / `{}` for STATIC_CLICK v1) |

Validation Lab session flush paths are untouched.

## Schema

### `aim_trials`

| Column | Notes |
|--------|--------|
| `id` | Stable padded id, e.g. `aim_{utc_date}_{seq:06}` |
| `app_version` | App constant |
| `experiment_id` | `aim_lab` (distinct from `validation_lab`) |
| `experiment_version` | `0.7.0` |
| `trial_type` | `STATIC_CLICK` |
| `status` | Always `completed` for inserted rows |
| `processor_id` | Snapshot at finish |
| `processor_version` | Snapshot at finish |
| `processor_config_json` | Snapshot at finish |
| `dpi` | From live `ExperimentSettings` |
| `sensitivity` | Same |
| `polling_rate_hz` | Same |
| `fov_degrees_h` | Same |
| `task_config_json` | See below |
| `metrics_json` | `{}` for STATIC_CLICK v1 |
| `start_unix_ms` / `end_unix_ms` | Wall clock |
| `start_timestamp_ns` / `end_timestamp_ns` | QPC / monotonic ns used for score |
| `duration_secs` | `(end_ns - start_ns) / 1e9` |
| `hits` | Successful hits |
| `shots` | All clicks (hits + misses) |
| `misses` | `shots - hits` |
| `score_secs` | Same meaning as current HUD (time to required hits) |
| `accuracy` | `hits / shots` (0 if shots == 0, should not occur on complete) |

### `task_config_json` (STATIC_CLICK)

```json
{
  "hits_required": 5,
  "target_radius": 0.25,
  "aim_distance": 10.0,
  "yaw_half_deg": 25.0,
  "pitch_up_deg": 12.0,
  "pitch_down_deg": 5.0,
  "floor_clearance": 0.35
}
```

Future trial types add their own keys; readers must branch on `trial_type`.

### `aim_shots`

| Column | Notes |
|--------|--------|
| `trial_id` | FK to `aim_trials.id` |
| `shot_index` | 0-based order within the trial |
| `timestamp_ns` | Shot time |
| `hit` | 0 or 1 |
| `yaw_deg` / `pitch_deg` | Look at shot |
| `target_x` / `target_y` / `target_z` | Sphere center |
| `target_radius` | Radius used for hit test |

Index: `(trial_id, shot_index)` unique.

## Write rules

1. While Armed, buffer shot records in memory on `AimTrial` (do not write DB).
2. On Nth required hit (STATIC_CLICK: 5): build `AimTrialRecord` + `Vec<AimShotRecord>` from buffers + live settings + wall clock.
3. Single SQLite transaction: insert trial, then all shots. Commit only if all succeed.
4. On success: phase → Idle; HUD shows score + `saved` + `trial_id`.
5. On txn failure: keep in-memory score; HUD shows `save failed: …`; **no** partial trial/shot rows left committed.
6. Cancel Aim, Start Validation while Armed, or process exit while Armed → **no insert**. Never promote incomplete/aborted runs to `status=completed`.

## Trial result model (in-app)

- Extend `AimTrial` (or a sibling resource) with: shot buffer, `misses`/`shots` counters, `last_persist: Option<AimPersistStatus>`.
- `AimPersistStatus`: `{ trial_id, score_secs, hits, shots, accuracy, saved_ok, error: Option<String> }` for last **attempted** completed save (success or fail). Abort clears “just completed” persist messaging so an unsaved abort is not shown as a DB result.
- Types: `AimTrialRecord`, `AimShotRecord` in `sense-types`; insert API on `TelemetryDb`.

## HUD

**Armed**

- State: Armed  
- Hits / required  
- Elapsed time (live from start ns)  
- Active processor id + short config summary  
- Last shot HIT/MISS (existing)

**Completed (in-memory after 5th hit)**

- State: Idle (completed)  
- Score, hits/required, duration, accuracy  
- Persist line: `saved <trial_id>` or `save failed: …`

**Abort**

- Return to Idle without treating the run as a completed DB result  
- Do not leave a false “saved” line for the aborted run

## Migration

`TelemetryDb::migrate` always `CREATE TABLE IF NOT EXISTS` for `aim_trials` and `aim_shots` (same pattern as `processed_mouse_events`), so existing `data/sense_maxer.db` files upgrade in place.

## Testing / validation

- Unit/DB: insert completed trial + shots in one txn; read back; assert columns match; abort path inserts nothing.
- Existing `cargo test -p sense-maxer` suite stays green (extend as needed; do not regress the prior 20).
- Manual: run a few STATIC_CLICK trials; confirm HUD score/hits/accuracy/duration match the new DB rows; cancel mid-run and confirm no completed row.

## Version / docs

- Bump aim/app experiment constant used for aim persistence to `0.7.0` (validation_lab may keep its own constant or share a crate-level bump — prefer **one** `EXPERIMENT_VERSION = "0.7.0"` if already shared, else document split).
- Update `docs/BASELINE.md`, `docs/TELEMETRY_SCHEMA.md` (replace vague `trials` placeholder with `aim_trials` / `aim_shots`).
- Brief README / ARCHITECTURE note: M3.x persistence shipped; stop.

## Stop criteria (M3.x done)

- Completed STATIC_CLICK trials persist transactionally with summary + per-shot rows  
- Aborts never appear as completed  
- HUD shows live + last-saved fields listed above  
- Raw/processed input path unchanged  
- Tests green + minimal manual DB check  
- **No** further M3 work (no new trial types) until a new spec

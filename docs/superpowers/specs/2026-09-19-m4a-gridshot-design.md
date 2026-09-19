# M4.a — GRIDSHOT v1 (Timed Multi-Target) — Design Spec

**Date:** 2026-09-19  
**Status:** Approved for implementation (user locked behavior + invariants)  
**Depends on:** M3.y aim telemetry data model (`0.8.0`)  
**Experiment version after ship:** `0.9.0`  
**trial_type:** `GRIDSHOT`

## Goal

Ship a KovaaK/Aim Lab–style **Gridshot** task on the existing five-layer aim telemetry: timed run, multiple exclusive-grid targets, hits + accuracy, completed-only persist. No new telemetry tables.

## Non-goals

- Tracking / moving targets / `direction_change`
- 1wall6 or other task types
- Artificial respawn delay
- Derived ML features as primary columns
- New camera/pitch model (reuse M3.x/M3.y)

## Architecture approach

**Shared aim runtime** (Armed buffers, Start config snapshot, look-unlock cancel, completed-only flush) + **task module** for Gridshot beside STATIC_CLICK, selected by `trial_type` / HUD.

---

## GRIDSHOT V1 INVARIANTS

- Grid dimensions: **3 × 3** (9 cells)
- Maximum live targets: **3**
- **3 live targets throughout the active trial, aside from the internal hit→respawn transition** (briefly 2 live between despawn and replacement spawn)
- Each cell contains at most one live target
- Each live target occupies exactly one cell
- A hit destroys exactly one target
- Every hit immediately produces exactly one replacement target in a **currently vacant** cell (chosen among the **6** vacant cells after removal)
- Misses never modify target state
- `target_id` values are unique within the trial and **never reused**
- Only `spawn` / `despawn` lifecycle events in v1 (no miss/hit target-events)
- `aim_shots.hit` owns click outcome
- Trial ends at **60.0 s** based on Start QPC (see timer boundary)
- Exclusive cells are an **invariant** at all times outside the atomic hit→respawn transition

---

## Run shape

| Knob | Value |
|------|--------|
| Duration | **60.0 s** |
| Concurrent live | **3** |
| Grid | **3 × 3** exclusive cells |
| Radius `R` | **0.25** |
| Score summary | `hits`, `shots`, `misses`, `accuracy = hits/shots`, `duration_secs` / `score_secs` = elapsed at end |
| End condition | Elapsed ≥ 60.0 s (QPC), not fixed hit count |

---

## Look reset (existing camera semantics)

At Start, use the **same** reset as STATIC_CLICK / M3.y:

- `yaw_deg = 0.0`
- `pitch_deg = 0.0`
- Camera origin remains `AIM_CAMERA_ORIGIN = (0, 1.6, 4)` (existing)
- Pitch/yaw mapping unchanged (`UnverifiedPitchModel` / shared 0.07)

Do **not** introduce a Gridshot-specific camera model.

---

## Geometry (front wall grid)

Grid lies on a plane in front of the camera at depth **D = 10** along identity forward (−Z), eye-height center.

| Param | Value |
|-------|--------|
| Plane center | `(0, 1.6, 4 − 10) = (0, 1.6, −6)` |
| Cell spacing | **1.0** world units (center-to-center) — ensures non-intersection (`2R = 0.5 < 1.0`) |
| Cell indices | `row ∈ {−1,0,1}`, `col ∈ {−1,0,1}` |
| Cell center | `(col * spacing, 1.6 + row * spacing, −6)` |

Occupied set: up to 3 of the 9 cells. Replacement spawn picks uniformly (seeded RNG) among **vacant** cells only.

`task_config_json` must record: `grid_rows`, `grid_cols`, `concurrent_targets`, `duration_secs`, `spacing`, `depth`, `radius`, `rng`, `rng_version`, plane/origin notes as needed.

---

## Timer boundary (precise)

```
start_qpc_ns = QPC time at which Gridshot becomes Armed
elapsed      = (now_qpc_ns - start_qpc_ns) / 1e9
end_qpc_ns   = first QPC timestamp where elapsed >= 60.0
```

- When `elapsed >= 60.0` on a sample/update, the trial **ends** at that timestamp.
- Input/camera/shot processing **after** that boundary must **not** create post-trial gameplay events (no further hits, spawns, or Armed buffering for that run).
- Avoid “60 s + one frame” ambiguity: the first timestamp that crosses the threshold is the end.

---

## Spawn / hit / miss flow

### Start (deterministic)

```
random_seed
  → RNG(task_version + rng_version in task_config)
  → choose 3 distinct cells
  → spawn target_001, target_002, target_003
```

Each `target_id` is stable for that instance’s lifetime.

### Hit (same logical update; no respawn delay)

Ordering within the click that hits:

1. `aim_shots` row with `hit = true`, `target_id` = destroyed instance  
2. `despawn` target event (same `timestamp_ns` as the shot, or documented equal logical time)  
3. Choose one vacant cell among the **6** empty cells  
4. Allocate **new** `target_id` (never reuse), e.g. `target_004`  
5. `spawn` target event + place sphere  

If shot and lifecycle events share one QPC sample time, use that timestamp for all of them and document “same logical update.”

### Miss

- `aim_shots` with `hit = false`
- Target set unchanged (no lifecycle events)

### Ray test

- Test against all live spheres; **closest** intersection along the ray wins (safety net; exclusive cells normally prevent multi-hit).

---

## Telemetry (reuse M3.y)

| Layer | Gridshot usage |
|-------|----------------|
| `aim_trials` | `trial_type = GRIDSHOT`, full Start snapshot, `random_seed`, summary hits/shots/accuracy/duration |
| `aim_target_events` | spawn/despawn only; `event_data_json` may include `cell_row`, `cell_col`, `radius` |
| `aim_shots` | every click + `target_id` |
| `aim_input_samples` / `aim_camera_samples` | Armed buffering until end boundary |

Completed-only atomic flush; abort/cancel/look-unlock → no insert (existing rules).

---

## HUD

- Start **Gridshot** (blocked if validation Running or another aim Armed)
- While Armed: time remaining, hits, shots, accuracy, live count (3)
- On complete: final hits / accuracy / duration + SAVED / SAVE FAILED
- Settings locked while Armed (existing snapshot immutability)

STATIC_CLICK remains available (task picker or separate Start button).

---

## Version / docs

- Bump `EXPERIMENT_VERSION` → **`0.9.0`**
- `task_version` for Gridshot = `"1"`; `rng` = `lcg`, `rng_version` = `"1"` (or shared helper)
- Update BASELINE / TELEMETRY_SCHEMA / README: GRIDSHOT v1 shipped; stop before next task type

---

## Testing

- Invariants: never two live in one cell; after hit→respawn always 3 live; IDs never reused
- Same `random_seed` → same initial 3 cells (and same replacement sequence given same hit order / cell choices)
- Timer: synthetic timestamps end at first `elapsed >= 60`; no post-end shots
- Miss does not change occupancy
- DB: completed Gridshot row + child streams; cancel mid-run inserts nothing
- Existing STATIC_CLICK + M3.y tests remain green

## Stop criteria

- Gridshot v1 playable + persisted under M3.y model  
- Invariants + timer boundary held  
- **Stop** — do not add tracking / 1wall6 until a new task spec

# ONE_WALL_SIX v1 — 1 Wall 6 Targets

**Date:** 2026-09-21  
**Status:** Implemented (exp 0.13.0)  
**Depends on:** Aim History + five-layer telemetry; GRIDSHOT multi-target path  
**Experiment version after ship:** `0.13.0` (with FLICK_LADDER)  
**trial_type:** `ONE_WALL_SIX`  
**task_version:** `"1"`

## Goal

**Precise, wide-angle flicks:** six small targets spread over a large region of the view so acquisition spans large yaw/pitch without GRIDSHOT’s dense mid-grid.

Closest in spirit to classic KovaaK **1wall6targets**.

## Non-goals

- Sequential-only (one lit target)
- Wave clear-all-then-refill
- Tracking / hold-fire
- Changing Raw Accel or existing tasks’ knobs

## Gameplay

| Knob | v1 value |
|------|----------|
| Duration | 60 s active |
| Concurrent live | **6** |
| Radius | **0.12** (smaller than GRIDSHOT 0.25) |
| Wall depth | z = **−8** (farther than GRIDSHOT −6 → wider angular span) |
| Placement | Uniform LCG in wall rectangle sized for ~**±40° yaw** and ~**±22° pitch** at that depth (exact world AABB frozen in `task_config_json`) |
| Min separation | Centers must stay ≥ **2.5 × radius** apart (reject/resample) |
| Hit | Closest ray∩sphere among hit spheres → despawn → respawn at new valid wall position |
| Miss | Nearest live target stamped (GRIDSHOT pattern) |

**Render:** Sphere entity pool must be ≥ **6** (today’s GRIDSHOT pool is 3).

## Scoring

| Field | Meaning |
|-------|---------|
| `hits` / `shots` / `misses` | LMB |
| `accuracy` | hits/shots |
| `score_secs` / `duration_secs` | ~60 active |

## Telemetry

- Per-target `spawn` / `despawn` with world position
- `aim_shots` + input + camera streams
- `task_config_json`: duration, radius, z, wall x/y bounds, concurrent=6, min_separation, rng/lcg

## Analysis / M4.2

- Click task; GRIDSHOT-like `metric_scope`
- Distinct `trial_type` — not FairMatch vs GRIDSHOT / FLICK_LADDER

## UI

Lobby radio **1 Wall 6**. HUD: hits/shots/%, time left.

## Acceptance

- 6 live targets visible; hit refreshes that slot on-wall
- Deterministic initial + respawn sequence for same seed
- History replay shows all six
- Pool ≥6 without breaking GRIDSHOT (3 concurrent)

## Ship with

[FLICK_LADDER](./2026-09-21-flick-ladder-design.md) at experiment `0.13.0`.

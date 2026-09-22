# FLICK_LADDER v1 — Commanded Yaw Flicks

**Date:** 2026-09-21  
**Status:** Implemented (exp 0.13.0)  
**Depends on:** Aim History + five-layer telemetry; GRIDSHOT click path  
**Experiment version after ship:** `0.13.0` (with ONE_WALL_SIX)  
**trial_type:** `FLICK_LADDER`  
**task_version:** `"1"`

## Goal

Force **large, situation-tagged yaw acquisitions** so sensitivity / acceleration can be compared by commanded angle (e.g. 45° vs 90°), not only mid-cone GRIDSHOT behavior.

## Non-goals

- Pitch-heavy ladder (v1 yaw-only)
- Hold-fire / TRACKING scoring
- Changing Raw Accel or GRIDSHOT/TRACKING
- Recommendations

## Gameplay

| Knob | v1 value |
|------|----------|
| Duration | 60 s active (pause-excluded) |
| Live targets | 1 |
| Radius | 0.25 |
| Pitch | ~0° (floor-clamped center) |
| Distance | STATIC_CLICK-style (~10 from camera origin) |
| Commanded yaw set | `{−90, −60, −45, +45, +60, +90}` degrees |
| Sequence | Seeded LCG shuffle / cycle of the set (no immediate repeat of same yaw preferred) |
| Hit | Ray∩sphere; despawn → next commanded spawn |
| Miss | Keep same target; stamp shot as miss |

Camera resets to yaw/pitch 0 at **Start** (same as other tasks). Each spawn is relative to **world** axes at that commanded yaw from forward (−Z), not relative to current look — so the player must turn to the absolute offset.

## Scoring

| Field | Meaning |
|-------|---------|
| `hits` / `shots` / `misses` | LMB clicks |
| `accuracy` | hits/shots (0 if no shots) |
| `score_secs` / `duration_secs` | ~60 active seconds |

## Telemetry

- `aim_target_events`: `spawn` / `despawn` with **commanded yaw/pitch** in existing yaw/pitch event fields (STATIC_CLICK pattern)
- `aim_shots`, `aim_input_samples`, `aim_camera_samples` as GRIDSHOT
- `task_config_json`: duration, radius, distance, yaw_set, scoring_rule `click`, rng/lcg + version

## Analysis / M4.2

- Allowed in `sense-analysis` as click task; `metric_scope` = GRIDSHOT profile
- Fairness: distinct `trial_type` + `task_config_json` — do not compare to GRIDSHOT as FairMatch

## UI

Lobby radio **Flick Ladder**. Playing HUD: hits/shots/%, time left (GRIDSHOT-like).

## Acceptance

- Playable from Lobby; persists; History replays spawn/despawn
- Seeded sequence deterministic for same `random_seed` + `task_version`
- Unit tests for yaw set, no-end-before-60s active, hit advances command

## Ship with

[ONE_WALL_SIX](./2026-09-21-one-wall-six-design.md) at experiment `0.13.0`.

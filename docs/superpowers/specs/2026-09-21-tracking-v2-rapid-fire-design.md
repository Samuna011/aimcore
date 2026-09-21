# TRACKING v2 — Hold Rapid-Fire Shots — Design Spec

**Date:** 2026-09-21  
**Status:** Implemented (exp 0.12.2)  
**Depends on:** TRACKING v1 motion/pause/History  
**Experiment version after ship:** `0.12.2`  
**trial_type:** `TRACKING`  
**task_version:** `"2"`

## Goal

While LMB is held in TRACKING, emit a deterministic **20 Hz** virtual shot stream (hit if ray∩sphere). Primary metrics are `hits` / `shots` / `accuracy`. Keep `score_secs` = on-target hold time as a **secondary** diagnostic. Enables History shot flashes and M4.1 analysis on TRACKING.

## Non-goals

- Changing strafe motion / reverse schedule / duration (30 s)
- Per-frame or 60 Hz fire
- Removing hold requirement
- Schema migrations

## Scoring

| Field | Meaning (v2) |
|-------|----------------|
| `shots` / `hits` / `misses` | Virtual rapid-fire while held |
| `accuracy` | `hits / shots` (0 if no shots) |
| `score_secs` | On-target hold seconds (secondary; same accumulation as v1) |

**Fire rule:** While Armed, not paused, LMB held: emit shots on a 50 ms active-time grid (`fire_rate_hz = 20`). First shot on press (or first held tick). Pause freezes the schedule. Release clears the schedule so the next press fires immediately.

Each shot writes a normal `aim_shots` row (pose + live target center/id + `hit`).

`task_config_json`: `scoring_rule: "hold_rapid_fire"`, `fire_rate_hz: 20`, plus existing motion knobs. `rng_version` stays `"1"` for motion RNG.

## Compat

- Old DB rows `task_version "1"`: time-based accuracy; no/empty shots
- New rows `"2"`: shot-based accuracy + secondary `score_secs`

## Stop

Ship `0.12.2` + task_version `"2"`; enable `sense-analysis` for TRACKING; stop.

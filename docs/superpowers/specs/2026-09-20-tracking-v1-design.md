# TRACKING v1 — Horizontal Strafe (Hold-to-Score) — Design Spec

**Date:** 2026-09-20  
**Status:** Approved for implementation (user locked run shape + scoring + internals)  
**Depends on:** Lab UI shell (`0.10.0`), Aim History replay (`0.11.0`), M3.y five-layer telemetry  
**Experiment version after ship:** `0.12.0`  
**trial_type:** `TRACKING`  
**task_version:** `"1"`

## Goal

Ship a first **tracking** aim task: one sphere strafes horizontally with **random sudden reversals**; score is **time on target while LMB is held**; look/input/camera telemetry still records the full run. Playable from Lobby, pause-safe, History-replayable via target events.

## Non-goals

- Vertical / 2D paths, multi-target tracking
- Click-to-destroy or flick hybrids
- 1wall6
- HUD difficulty sliders (knobs frozen in `task_config_json`)
- Schema changes
- Completeness charts

---

## Architecture approach

**Discrete sample scoring (Approach 1):** On each Armed input sample (active / non-paused time), if LMB is **held** and the aim ray intersects the live sphere, add that sample’s interval to `time_on_target_ns`. Motion advances on active elapsed; each reverse or wall bounce emits `direction_change` so History replay stays event-driven (no re-roll).

Shared aim runtime: Start snapshot, pause accounting, completed-only persist, Lobby task picker, History list/replay.

---

## TRACKING V1 INVARIANTS

- Exactly **one** live target for the whole Armed run (aside from Start spawn / End despawn)
- Target motion is **horizontal only** (y fixed, z fixed on front wall)
- Score time accumulates **only** when `LMB held ∧ ray–sphere hit`
- Look/input/camera samples are recorded regardless of hold/hit
- Every scheduled reverse **and** every wall-bound bounce is logged as `direction_change`
- No `aim_shots` rows in v1 (scoring is not click-outcome based)
- Trial ends at first **active** elapsed ≥ **30.0 s**
- Pause freezes motion, reverse schedule, and score accumulation

---

## Run shape

| Knob | Value |
|------|--------|
| Duration | **30.0 s** active |
| Concurrent live | **1** |
| Radius `R` | **0.25** |
| Plane | z = **−6**, y = **1.6** (same depth/height family as Gridshot) |
| Strafe bounds | x ∈ **[−1.5, 1.5]** |
| Speed \|vx\| | **1.2** world units / second (crosses full 3.0-wide span in 2.5 s) |
| Reverse schedule | After each reverse, next delay ~ **Uniform(1.5, 3.5) s** from seeded RNG; also reverse immediately on hitting ±x bound |
| Start pose | Camera yaw/pitch 0; target at x=0 (or seeded start x inside bounds) with initial ±vx from seed |
| Score | `score_secs` = time on target (seconds); `accuracy` = `score_secs / active_duration_secs` |
| `hits` / `shots` / `misses` | **0** (unused; schema compat) |
| End | Active elapsed ≥ 30 s → despawn, Idle, persist |

`task_config_json` must include: `rng`, `rng_version`, duration, R, x_min/x_max, speed, reverse_delay_min/max, scoring rule id (`hold_and_ray`).

---

## Scoring (sample-discrete)

For each Armed sample with timestamp `t` while **not paused**:

1. Advance target position by `dt` using current velocity (active time only).
2. Possibly schedule/fire reverse or wall bounce → append `direction_change`.
3. If LMB held **and** `ray_sphere_hit` from current camera → `time_on_target_ns += dt_ns` (use raw sample interval consistent with input log; document choice: prefer same `dt_ns` as `aim_input_samples` for that sample, or 0 for first sample).

At finish:

- `score_secs = time_on_target_ns / 1e9`
- `accuracy = score_secs / active_duration_secs` (clamp shots/hits unused)
- Optional `metrics_json`: `{ "time_on_target_secs": ..., "hold_on_target_fraction": ... }`

**LMB level:** maintain held flag from raw button down/up across samples (not falling-edge only).

---

## Target events

| event_type | When |
|------------|------|
| `spawn` | Start — position + initial velocity |
| `direction_change` | Each RNG reverse **and** each wall bounce — new `velocity_x` (y/z 0), updated position |
| `despawn` | Finish |

History replay integrates x between events using logged velocities; must not call RNG during replay.

---

## UI / shell

- Lobby: add **Tracking** to task selection; Start arms TRACKING + look on.
- Playing compact HUD: mode · on-target time · hold hint · time remaining · running accuracy.
- V overlay: snapshot fields (unchanged rules).
- Pause / Resume / Restart / Change trial: existing Lab UI semantics; motion+score freeze while paused.
- Esc never aborts; cancel only via Pause menu.

---

## History

- List shows TRACKING rows (score_secs = on-target time).
- Replay: camera from samples; sphere from spawn + `direction_change` integration; no shot flashes.

---

## Version / docs

- Bump `EXPERIMENT_VERSION` / `AIM_EXPERIMENT_VERSION` → **`0.12.0`**
- Update BASELINE / README / TELEMETRY: TRACKING v1; hold∧ray scoring; no schema change
- Do not bump STATIC_CLICK / GRIDSHOT `task_version`

---

## Testing

- Same seed → same initial vx and reverse times (given same active timeline)
- Score increases only on held+hit samples
- Pause excludes motion advance and score
- Wall bounce and RNG reverse both emit `direction_change`
- Ends at 30s active; no post-end scoring
- History replay position matches event integration for a fixture bundle
- Existing STATIC_CLICK / GRIDSHOT / Lab UI / History tests remain green

## Stop criteria

- TRACKING playable from Lobby, persisted, History-replayable  
- Hold∧ray sample scoring + pause-safe motion  
- Docs + `0.12.0`  
- **Stop** — no 1wall6 / vertical tracking / shot-based scoring in this milestone

## Architectural summary

**One sphere strafes horizontally with logged sudden reversals. Full look telemetry always; score only while the player holds LMB on the target. Reconstruct from spawn + direction_change + camera samples.**

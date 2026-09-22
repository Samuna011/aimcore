# FLICK_LADDER + ONE_WALL_SIX Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or executing-plans. Checkbox steps for tracking.

**Goal:** Ship `FLICK_LADDER` and `ONE_WALL_SIX` at experiment `0.13.0` per approved specs.

**Architecture:** Two modules mirroring GRIDSHOT (timed click tasks). Bump aim-target render pool to 6. Extend `AimTaskKind`, lobby, camera drain, persist, analysis gate + metric_scope.

**Tech Stack:** Bevy app modules, existing LCG / ray-sphere / five-table telemetry, `sense-analysis` string gates.

**Specs:** `docs/superpowers/specs/2026-09-21-flick-ladder-design.md`, `docs/superpowers/specs/2026-09-21-one-wall-six-design.md`

## Global Constraints

- experiment_version `0.13.0`; each new task `task_version` `"1"`
- No schema migration; new `trial_type` strings only
- Do not retune GRIDSHOT/TRACKING/STATIC_CLICK knobs
- Do not change Raw Accel
- Sphere pool ≥ 6; GRIDSHOT still uses 3 concurrent
- Analysis: click-task `metric_scope` (GRIDSHOT profile)

## File map

| File | Role |
|------|------|
| `src/aim_flick_ladder.rs` | FLICK_LADDER pure + runtime |
| `src/aim_one_wall_six.rs` | ONE_WALL_SIX pure + runtime |
| `src/aim_trial.rs` | enum, pool size, sync, persist match |
| `src/validation_lab.rs` / `lab_ui.rs` / `camera_ctrl.rs` / `main.rs` / `session.rs` | UI + wiring + exp bump |
| `crates/sense-analysis/src/{lib,scope}.rs` | allow trial types + scope |
| docs | BASELINE / README / TELEMETRY |

### Task 1: Pool size + AimTaskKind variants
### Task 2: FLICK_LADDER module + tests
### Task 3: ONE_WALL_SIX module + tests
### Task 4: Lobby / camera / HUD / persist / exp 0.13.0
### Task 5: sense-analysis gate + docs

Commits only if user requests.

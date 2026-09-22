# Locked Experimental Baseline (post-M1)

**Status:** LOCKED as of 2026-09-17 after M1 Runs 1–3  
**M1:** COMPLETE — STOPPED. Do not reopen M1 architecture unless a measured regression appears.

This is the default experimental condition for all subsequent milestones (M2+) unless an experiment explicitly snapshots a different configuration.

## Rendering

| Setting | Value |
|---------|--------|
| VSync | **OFF** (`PresentMode::AutoNoVsync`) |
| FPS | **Uncapped** via present mode (no separate application FPS limiter) |
| Frame telemetry | Records **actual** frame time / FPS |

**History:** Post-M1 lasting default was VSync ON. M1 Run 3 used uncapped for integrity only. **2026-09-17 exp `0.5.2`:** experimenter A/B’d uncapped for perceived input lag — **adopted** (feels better). VSync ON remains available if an experiment explicitly snapshots it.

VSync / refresh capping does **not** couple WM_INPUT sampling to render FPS. Input remains event-driven and queue-drained independently.

M1 does **not** claim measured mouse-to-photon latency for any present mode.

## Input

| Setting | Value |
|---------|--------|
| Raw input | Windows **`WM_INPUT`** (relative only; absolute packets skipped) |
| Window bridge | **`SetWindowSubclass`** on Bevy winit HWND |
| Timestamp | **QPC** → nanoseconds (monotonic) |
| Queue | Timestamped lossless queue; drain **all** samples; never coalesce/discard for FPS |
| Acceleration | **`NoAcceleration`** (identity `InputProcessor`) until M2+ replaces/extends processors |
| Pitch | **Enabled** — UnverifiedPitchModel (same 0.07; +dy look down; ±89°; **UNCERTAIN**) |

## Angular / display model

| Setting | Value |
|---------|--------|
| Yaw constant | **0.07** deg/count at sens 1.0 — **UNCERTAIN / community-derived** |
| Yaw formula | `yaw_delta_deg = raw_dx × sensitivity × 0.07` |
| Horizontal FOV | **103°** (`fov_axis = Horizontal`) |
| DPI | **Configurable per experiment** (declared; does not enter yaw math) |
| Sensitivity | **Configurable per experiment** |

Do **not** silently compensate the yaw constant from human 360° residuals.

## Current operator defaults (editable; not part of the lock above)

| Setting | Current default |
|---------|-----------------|
| DPI | 3200 (Logitech hardware declaration) |
| Sensitivity | 0.09 |
| eDPI | 288 |

Changing DPI/sens for an experiment is expected. Changing VSync/present mode, raw-input path, QPC, NoAcceleration, pitch model, `0.07`, or HFOV=103 requires an explicit experiment version bump and documentation.

**Experiment version:** `0.14.0` (FLICK_DEMAND + M4.3 movement-demand analysis; FLICK_LADDER / ONE_WALL_SIX remain from `0.13.0`).

## Research progression (locked order)

```
M1  Raw input validation          ← COMPLETE / STOPPED
 ↓
M2  InputProcessor framework      ← COMPLETE
 ↓
M2.x Reproduce Raw Accel mathematics ← Phase 1.1 + feel parity
 ↓
M3  Controlled aim task           ← STATIC_CLICK v1 (exp 0.6.x)
 ↓
M3.x Aim trial persistence        ← COMPLETE / STOPPED (exp 0.7.0)
 ↓
M3.y Aim telemetry data model     ← COMPLETE / STOPPED (exp 0.8.0)
 ↓
M4.a GRIDSHOT v1                  ← COMPLETE / STOPPED (exp 0.9.0)
 ↓
Lab UI shell (Lobby/Playing/Pause) ← COMPLETE / STOPPED (exp 0.10.0)
 ↓
Aim History replay (list + 3D reconstruct) ← COMPLETE / STOPPED (exp 0.11.0)
 ↓
TRACKING v1 (horizontal strafe hold-to-score) ← COMPLETE / STOPPED (exp 0.12.0)
 ↓
LCG full-unit fix (STATIC_CLICK/GRIDSHOT task_version 2) ← COMPLETE / STOPPED (exp 0.12.1)
 ↓
TRACKING v2 (hold rapid-fire 20 Hz shots) ← COMPLETE / STOPPED (exp 0.12.2)
 ↓
FLICK_LADDER + ONE_WALL_SIX ← COMPLETE / STOPPED (exp 0.13.0)
 ↓
M4.3 Movement demand × config (FLICK_DEMAND; comparison_version `"2"`) ← COMPLETE / STOPPED (exp 0.14.0)
 ↓
M4.1 sense-analysis freeze (`metric_scope`; analysis knobs) ← COMPLETE
 ↓
M4.2 Condition Comparison (`compare_trials`) ← COMPLETE
 ↓
M4.2.1 Exposure semantics (physical vs processor speeds; analysis_version `"2"`) ← COMPLETE / STOPPED
 ↓
Fitted interaction models / recommender (future)
 ↓
Longitudinal experiment
```

Do not skip ahead. **Stop** after M4.3 (`0.14.0`) — collect the sens × accel × demand matrix manually; no fitted models, recommender, or Raw Accel retune from tables.

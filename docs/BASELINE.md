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

**Experiment version:** `0.11.0` (Aim History: list completed trials + read-only 3D arena replay from stored telemetry; Esc pauses replay; no schema change; GRIDSHOT + STATIC_CLICK on five-table aim telemetry with Lab UI shell pause semantics).

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
M4  Compare conditions
 ↓
Longitudinal experiment
```

Do not skip ahead. **Stop** after Aim History replay — no completeness charts, validation replay, or delete until a new task spec; do not add tracking / 1wall6 until a new task spec.

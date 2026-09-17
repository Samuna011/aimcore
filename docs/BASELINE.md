# Locked Experimental Baseline (post-M1)

**Status:** LOCKED as of 2026-09-17 after M1 Runs 1–3  
**M1:** COMPLETE — STOPPED. Do not reopen M1 architecture unless a measured regression appears.

This is the default experimental condition for all subsequent milestones (M2+) unless an experiment explicitly snapshots a different configuration.

## Rendering

| Setting | Value |
|---------|--------|
| VSync | **OFF** (`PresentMode::AutoNoVsync`) |
| FPS | **Uncapped** (no application FPS cap; no 240 FPS limit) |
| Frame telemetry | Records **actual** frame time / FPS |

Uncapped / VSync-off does **not** constitute a measured mouse-to-photon latency claim.

## Input

| Setting | Value |
|---------|--------|
| Raw input | Windows **`WM_INPUT`** (relative only; absolute packets skipped) |
| Window bridge | **`SetWindowSubclass`** on Bevy winit HWND |
| Timestamp | **QPC** → nanoseconds (monotonic) |
| Queue | Timestamped lossless queue; drain **all** samples; never coalesce/discard for FPS |
| Acceleration | **`NoAcceleration`** (identity `InputProcessor`) until M2+ replaces/extends processors |
| Pitch | **Disabled** (`PITCH MODEL: UNVERIFIED`) |

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

Changing DPI/sens for an experiment is expected. Changing VSync/FPS/raw-input/QPC/NoAcceleration/pitch-disabled/0.07/HFOV=103 requires an explicit experiment version bump and documentation.

## Research progression (locked order)

```
M1  Raw input validation          ← COMPLETE / STOPPED
 ↓
M2  InputProcessor framework
 ↓
M2.x Reproduce Raw Accel mathematics
 ↓
M3  Controlled aim task
 ↓
M4  Compare conditions
 ↓
Longitudinal experiment
```

Do not skip ahead.

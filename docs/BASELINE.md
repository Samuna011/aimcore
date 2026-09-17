# Locked Experimental Baseline (post-M1)

**Status:** LOCKED as of 2026-09-17 after M1 Runs 1–3  
**M1:** COMPLETE — STOPPED. Do not reopen M1 architecture unless a measured regression appears.

This is the default experimental condition for all subsequent milestones (M2+) unless an experiment explicitly snapshots a different configuration.

## Rendering

| Setting | Value |
|---------|--------|
| VSync | **ON** (`PresentMode::AutoVsync`) |
| FPS | **Capped to display refresh** via VSync (no separate application FPS limiter) |
| Frame telemetry | Records **actual** frame time / FPS |

**History:** Run 3 temporarily used `AutoNoVsync` / uncapped FPS only to verify that presentation mode does not break the raw-input pipeline. That was **not** adopted as the lasting baseline. Do not switch back to uncapped / VSync-off unless the experimenter explicitly requests it.

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

**Experiment version:** `0.5.0` (UnverifiedPitchModel enabled).

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

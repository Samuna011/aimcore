# M2 Input Processor Framework

**Date:** 2026-09-17  
**Experiment version:** `0.4.0` for all new sessions, regardless of processor  
**Status:** M2 COMPLETE; M2.x Phase 1.1 (`rawaccel_linear` v1.1.0) COMPLETE / STOPPED

---

## Purpose

M2 establishes `InputProcessor` as the session-scoped boundary between immutable raw mouse samples and yaw/camera application. Raw telemetry is unchanged; processed samples are stored in a separate table with per-row processor identity.

```
WM_INPUT → Raw sample → InputProcessor → Processed dx/dy → Yaw → Camera
                ↓                              ↓
         raw_mouse_events            processed_mouse_events
```

Under `none` / `NoAcceleration`, processed counts equal raw counts and validation behavior matches M1. M2.x Phase 1.1 adds `rawaccel_linear` (Gain + caps, 1:1 classic port) — see [M2X_RAWACCEL_LINEAR.md](./M2X_RAWACCEL_LINEAR.md).

---

## Pipeline

1. **Idle:** HUD shows selected `processor_id` (`"none"` or `"rawaccel_linear"`). DPI/sensitivity editable. Processor selection and full `rawaccel_linear` config (Gain, offset, cap mode, cap X/Y, acceleration, multiplier) editable only while Idle.
2. **Start Validation:** `create_processor(selected_id, &settings.rawaccel_linear_config())` from session settings (config ignored for `"none"`); snapshot id/version/config into `ConfigurationRecord`; install `ActiveInputProcessor`; reset timing and buffers.
3. **Running:** Drain all queued raw samples. For each sample:
   - Compute `dt_s` from consecutive raw QPC timestamps (see below).
   - Call `process(dx, dy, dt_s)` → `(processed_dx, processed_dy)`.
   - Apply yaw from **processed** horizontal counts.
   - Record raw + processed + input-camera samples in memory buffers.
   - Do **not** pass render-frame delta into the processor.
4. **End Validation:** Atomic flush of raw, processed, input-camera, frame, and validation result rows.
5. **Reset Counters:** Clears processed buffer with other buffers; integrity reset unchanged.

Processor id cannot change mid-session.

---

## `dt_s` Rules

Inter-sample interval for velocity-dependent processors (unused under `none`, but API-ready):

```
sample N-1 timestamp_ns = T1
sample N   timestamp_ns = T2

dt_s = (T2 - T1) / 1_000_000_000
```

| Rule | Detail |
|------|--------|
| Source | Consecutive **raw mouse** QPC timestamps only |
| Forbidden | `Time::delta_secs()`, frame duration, FPS, VSync / present timing |
| First sample | No previous timestamp → `dt_s = 0.0` (documented convention) |
| Reset | `ProcessorTimingState.last_raw_timestamp_ns` cleared on Start Validation and Reset Counters |

Implementation: `sense_accel::dt_s_from_timestamps` in `crates/sense-accel`.

---

## Processors

| id | version | config | transform |
|----|---------|--------|-----------|
| `none` | `1.0.0` | `{}` | Identity: `(dx, dy)` unchanged |
| `rawaccel_linear` | `1.1.0` | `RawAccelLinearConfig` (acceleration, sensitivity_multiplier, gain, input_offset, cap_mode, cap_x, cap_y) | Raw Accel Linear (Gain or Legacy/Sensitivity; caps per config) — see [M2X_RAWACCEL_LINEAR.md](./M2X_RAWACCEL_LINEAR.md) |

Factory: `sense_accel::create_processor(id, &RawAccelLinearConfig)` — config ignored for `"none"`; unknown ids fail at session start.

Crate layout: `crates/sense-accel` (trait + registry); types in `sense-types`; wiring in `src/camera_ctrl.rs` and `src/session.rs`.

---

## Telemetry

New table: `processed_mouse_events` (see [TELEMETRY_SCHEMA.md](./TELEMETRY_SCHEMA.md)).

Each row pairs with `raw_mouse_events` on `(session_id, sequence_number)` and stores:

- `processed_dx`, `processed_dy` (REAL)
- `processor_id`, `processor_version`, `processor_config_json`

Configuration snapshot also records processor fields under `AccelerationConfig`.

---

## Manual Verification Checklist

1. Build and run Validation Lab (`cargo run --release`).
2. Confirm HUD: `PROCESSOR: none`, `PROCESSOR VERSION: 1.0.0`, note that processed equals raw.
3. **Start Validation** → look (Esc) → one horizontal 360° → **End Validation**.
4. Inspect SQLite:

```bash
sqlite3 data/sense_maxer.db "SELECT COUNT(*) FROM processed_mouse_events WHERE session_id = (SELECT id FROM sessions ORDER BY start_unix_ms DESC LIMIT 1);"
sqlite3 data/sense_maxer.db "SELECT processor_id, processor_version, processed_dx, processed_dy FROM processed_mouse_events LIMIT 5;"
```

5. Confirm `processor_id = none`, processed dx/dy match paired raw dx/dy, and `experiment_version = 0.4.0` on the session row. Repeat with `rawaccel_linear` (Gain + Output 2 defaults); confirm `processor_version = 1.1.0` and full config JSON in processed rows. New sessions use `0.4.0` regardless of processor.

---

## Explicitly Out of Scope (M2 + M2.x Phase 1.1)

- Additional Raw Accel modes (Natural, Power, Jump, Synchronous, LUT, …)
- Anisotropy, rotation, By Component, EMA, DPI normalization
- Driver-side comparison against real Raw Accel
- Mid-session processor hot-swap
- Pitch enablement
- Aim tasks (STATIC_CLICK, etc.)
- CSV/JSON export
- Changing VSync back to uncapped
- Silent yaw-constant compensation

**STOP:** M2.x Phase 1.1 (`rawaccel_linear` v1.1.0) is complete. Do not implement Natural, anisotropy, other styles, LUT infrastructure, or driver comparison without a new design + approval cycle. See [M2X_RAWACCEL_LINEAR.md](./M2X_RAWACCEL_LINEAR.md).

---

## Related Documents

- [M2X_RAWACCEL_LINEAR.md](./M2X_RAWACCEL_LINEAR.md) — M2.x Phase 1.1 Raw Accel Linear provenance and stop gate
- [2026-09-17-m2-input-processor-design.md](./superpowers/specs/2026-09-17-m2-input-processor-design.md) — full M2 design spec
- [2026-09-17-m2x-rawaccel-linear-design.md](./superpowers/specs/2026-09-17-m2x-rawaccel-linear-design.md) — M2.x Phase 1 design spec
- [2026-09-17-m2x-rawaccel-linear-gain-caps-design.md](./superpowers/specs/2026-09-17-m2x-rawaccel-linear-gain-caps-design.md) — M2.x Phase 1.1 design spec
- [ARCHITECTURE.md](./ARCHITECTURE.md) — Bevy wiring and crate layout
- [TELEMETRY_SCHEMA.md](./TELEMETRY_SCHEMA.md) — `processed_mouse_events` schema
- [BASELINE.md](./BASELINE.md) — locked VSync / WM_INPUT / pitch settings

# Architecture — VALORANT Input Validation Lab (M1)

**Date:** 2026-09-17  
**Scope:** Milestone 1 only — Validation Lab; no aim tasks.  
**Stack:** Bevy `0.19.1`, `bevy_egui` `0.42.0`, Windows only.

---

## Purpose

sense-maxer is a controlled experimental instrument for long-term mouse-input / sensitivity research. M1 is **not** a commercial aim trainer. It proves this pipeline end-to-end:

```
RAW MOUSE INPUT → SENSITIVITY MODEL → CAMERA YAW → NATIVE 3D RENDER → TELEMETRY → SQLITE
```

Primary subject: the developer. Priorities: input accuracy, low/predictable input-to-camera latency, mathematical VALORANT yaw equivalence, raw telemetry integrity, reproducibility.

---

## Bevy Process Diagram

```
┌─────────────────────────────────────────────────────────┐
│  Bevy App (single native process, Windows)              │
│                                                         │
│  ┌──────────────┐   ┌─────────────┐   ┌──────────────┐  │
│  │ WM_INPUT     │──▶│ MouseQueue  │──▶│ InputDrain   │  │
│  │ (Win32)      │   │ timestamped │   │ every sample │  │
│  └──────────────┘   └─────────────┘   └──────┬───────┘  │
│                                              │          │
│         ┌────────────────────────────────────┼───────┐  │
│         ▼                                    ▼       │  │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────┐ │  │
│  │ Telemetry   │  │ ValorantYaw  │  │ Camera      │ │  │
│  │ raw + cam   │  │ sense-math   │  │ yaw only    │ │  │
│  └──────┬──────┘  └──────────────┘  └──────┬──────┘ │  │
│         │                                   │        │  │
│         ▼                                   ▼        │  │
│  ┌─────────────┐                    ┌─────────────┐ │  │
│  │ SQLite      │                    │ Bevy 3D     │ │  │
│  │ (batched)   │                    │ + egui HUD  │ │  │
│  └─────────────┘                    └─────────────┘ │  │
└─────────────────────────────────────────────────────────┘
```

### Hard rules

- No DOM / browser / canvas-based game mechanics on the real-time path.
- Every raw sample is queued with monotonic `timestamp_ns` + `sequence_number`.
- Drain consumes **all** queued samples in order; never silently accumulate-and-discard.
- Raw samples are immutable; derived metrics are stored separately.
- Never silently compensate for discrepancies vs VALORANT; report them.
- Never tune until it “feels right”; mathematical equivalence is the target.

---

## Crate Layout

```
sense-maxer/
  crates/
    sense-math/          # degrees/count, eDPI, cm/360, inches/360 (no Bevy)
    sense-types/         # MouseSample, InputCameraSample, Session, Configuration, IDs
    sense-telemetry/     # in-memory buffers + batched SQLite writer
    sense-input-win/     # WM_INPUT → timestamped queue
  src/                   # Bevy binary: scene, yaw camera, egui Validation Lab
  docs/
    ARCHITECTURE.md
    VALORANT_INPUT_MODEL.md
    TELEMETRY_SCHEMA.md
    EXPERIMENT_MODEL.md
```

Sensitivity formulas live only in `sense-math`. Bevy, UI, and database code never reimplement them.

---

## Why `sense-math` Is Pure

`sense-math` is a dependency-free Rust crate with no Bevy, Win32, or SQLite imports.

**Rationale:**

1. **Single source of truth** — All sensitivity calculations (`degrees_per_count`, `eDPI`, `counts_per_360`, `cm_per_360`, validation expected counts) use the same functions in the app, HUD, and tests.
2. **Testability** — Unit tests run without GPU, windowing, or database setup.
3. **Reproducibility** — Math can be verified independently of rendering or input timing.
4. **Separation of concerns** — Platform and rendering layers consume math; they do not define it.

---

## Horizontal FOV Projection

The experiment configuration stores horizontal FOV, while Bevy's
`PerspectiveProjection::fov` expects vertical radians. The app recomputes the
vertical FOV from the current window aspect ratio (`width / height`):

```text
vfov = 2 × atan(tan(hfov / 2) / aspect)
```

This keeps the configured 103° horizontal FOV constant when the window aspect
ratio changes.

---

## Why `WM_INPUT`, Not Cursor Position

Mouse input is captured via Windows `WM_INPUT` raw relative movement.

**Do not** use cursor position deltas (`GetCursorPos`-style) as the experimental stream.

**Rationale:**

- `WM_INPUT` delivers hardware-reported relative counts independent of cursor warping, pointer acceleration settings on the OS path, and screen edge behavior.
- Cursor position deltas conflate physical movement with pointer repositioning and are unsuitable for sensitivity validation research.
- Raw `dx`/`dy` counts are the same unit family VALORANT yaw math expects.

---

## Bevy 0.19 HWND Hook (`SetWindowSubclass`)

After winit creates the primary window, a Bevy `Startup` exclusive system (`install_raw_input_hook` in `src/input_plugin.rs`) reads the Win32 `HWND` from the window entity's `RawHandleWrapper`. It:

1. Calls `sense_input_win::register_raw_mouse` to register for raw mouse `WM_INPUT`.
2. Installs a `SetWindowSubclass` callback (subclass ID `0x5345_4E53_455F_5241`) on the same window thread.
3. Stores `HookState` (shared `Arc<MouseQueue>` + `Arc<Mutex<IntegrityTracker>>`) as subclass reference data.

The subclass proc handles each `WM_INPUT` by calling `sense_input_win::handle_wm_input`, then **always** forwards to `DefSubclassProc` so winit's normal processing remains intact. Hook state is released on `WM_NCDESTROY`. The Bevy `Update` drain consumes every queued sample in order; no Bevy cursor or `MouseMotion` event participates in the experimental stream.

---

## Queue vs Render FPS

Input collection and rendering run on independent cadences:

```
RAW INPUT → timestamped queue → process every sample → camera yaw + telemetry → render
```

| Stream | Cadence | Purpose |
|--------|---------|---------|
| Mouse queue | Event-driven (hardware/OS) | Raw input samples with monotonic timestamps |
| Input drain | Every queued sample, before/during render | Apply yaw, record telemetry |
| Render / `FrameSample` | Display refresh rate | 3D scene + egui HUD |

**Rules:**

- Input collection continues even if rendering stalls.
- Frame timing is a separate stream (`FrameSample`); do not use FPS as a proxy for input timestamps.
- Movement intervals use monotonic clock (`QueryPerformanceCounter` → ns). Session metadata may use wall clock.
- M1 does **not** claim end-to-end display latency measurement; architecture reserves hooks for future instrumentation.

---

## `InputCameraSample` vs Future `RenderCameraSample`

| Type | When recorded | M1 status |
|------|---------------|-----------|
| `InputCameraSample` | Immediately after applying one raw mouse sample | **Implemented (M1)** |
| `RenderCameraSample` | Camera pose at render submit/present | **Future — not M1** |

`InputCameraSample` is **input-derived state**: `yaw_deg`, `pitch_deg`, and `sequence_number` tied to the mouse sample that produced the update. It is **not** the camera pose at frame submit.

A future `RenderCameraSample` must be a separate type and table. Do not alias or conflate render-frame pose with input-derived pose.

**M1 cadence:** one `InputCameraSample` row per drained mouse sample while `ValidationState::Running`. Samples are buffered in memory during the session and flushed to SQLite on End Validation. Camera yaw still updates live outside a session; only telemetry persistence and net counters are gated on validation state.

---

## Pitch Disabled in M1

- `raw_dy` is recorded in telemetry unchanged.
- Camera pitch is **not** updated from mouse input.
- egui HUD shows explicitly:
  - `PITCH MODEL: UNVERIFIED`
  - `PITCH ROTATION: DISABLED`

When pitch is enabled later, it must sit behind a named abstraction (e.g. `UnverifiedPitchModel`). M1 does not pretend VALORANT vertical behavior is verified.

---

## SQLite Batching

Telemetry persists to SQLite at `data/sense_maxer.db` (directory `data/` is gitignored).

**Performance rules:**

- Prepared statements for repeated inserts.
- Batched inserts inside transactions.
- **Never** one transaction per mouse event.

The `sense-telemetry` crate owns in-memory buffers and a batched writer. Raw events and derived samples flush in batches; the real-time input path does not block on disk I/O per sample.

---

## Acceleration (M1 Stub)

M1 implements `NoAcceleration` (identity transform) behind an `InputProcessor` trait. Future curve types (Linear, Classic, Natural, Power, Custom/LUT) must not require changes to camera or telemetry core.

When acceleration is eventually enabled, telemetry keeps both raw and processed `dx`/`dy`. M1 stores raw; processed equals raw under `NoAcceleration`.

---

## Explicitly Out of Scope (M1)

- STATIC_CLICK / flick / tracking / target switching
- Movement segmentation and flick phase classifier
- Acceleration curve implementations beyond identity stub
- ML / Bayesian optimization / auto sensitivity search
- Separate research UI process (Vue/Tauri/etc.)
- CSV/JSON export tooling
- Cross-platform input
- Pitch camera control
- Claiming display latency measurement
- `RenderCameraSample` and render-frame camera tables
- Future telemetry tables: trials, movements, task_events, performance_metrics, users, devices (documented in `TELEMETRY_SCHEMA.md` only)

---

## Related Documents

- [VALORANT_INPUT_MODEL.md](./VALORANT_INPUT_MODEL.md) — formulas, units, provenance
- [TELEMETRY_SCHEMA.md](./TELEMETRY_SCHEMA.md) — tables, raw vs derived
- [EXPERIMENT_MODEL.md](./EXPERIMENT_MODEL.md) — Validation Lab flow, session/config IDs

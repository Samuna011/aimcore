# sense-maxer

Windows-native Bevy VALORANT Input Validation Lab (M1 + M2).

Pipeline:

```
RAW MOUSE INPUT → INPUT PROCESSOR → SENSITIVITY MODEL → CAMERA YAW → NATIVE 3D RENDER → TELEMETRY → SQLITE
```

See `docs/` for architecture, input model, telemetry schema, experiment design, the **locked baseline** (`docs/BASELINE.md`), M1 completion report, **M2 processor framework** (`docs/M2_PROCESSOR.md`), and **M2.x Raw Accel Linear** (`docs/M2X_RAWACCEL_LINEAR.md`).

**M1 status:** COMPLETE / STOPPED. Experimental baseline locked (VSync **ON** / FPS capped to refresh, WM_INPUT, QPC, yaw 0.07 uncertain, HFOV 103°, DPI/sens configurable). Uncapped VSync-off was a temporary M1 verification only.

**M2 status:** COMPLETE. InputProcessor framework with dual telemetry (raw + `processed_mouse_events`).

**M2.x status:** Phase 1.1 COMPLETE / STOPPED. Processors: `none` and `rawaccel_linear` (v1.1.0, Gain + caps). Documents mathematical behavior 1:1 with official source — not actual-driver comparison. **Stopped** before Natural/anisotropy/LUT/driver comparison.

**M3 status:** UnverifiedPitchModel enabled. Look mode applies yaw **and** pitch (same 0.07; +dy look down; ±89°; **UNCERTAIN**). All new sessions use `experiment_version` `0.5.0`. See [design spec](docs/superpowers/specs/2026-09-17-unverified-pitch-model-design.md) and [research findings](docs/superpowers/specs/2026-09-17-valorant-pitch-research-findings.md).

## Requirements

- **Windows 10/11** (raw `WM_INPUT` capture is Windows-only in M1)
- [Rust toolchain](https://rustup.rs/) (2021 edition)
- GPU/driver compatible with Bevy/wgpu

## Build

```bash
cargo build --release
```

Debug build (faster compile):

```bash
cargo build
```

## Run

```bash
cargo run --release
```

Opens the Validation Lab window (1280×720 default).

**Cursor / UI control**

- Starts in **UI mode** (cursor free) so you can click Start / Reset / End.
- Press **Esc** to enter **look mode** (cursor locked) for yaw/pitch look and horizontal 360° validation.
- Press **Esc** again to unlock the cursor and use the Validation Lab buttons.
- While unlocked, mouse movement is not applied to the camera and is not counted toward validation.

### Validation Lab workflow

1. (UI mode) **Start Validation** — creates session/config in `data/sense_maxer.db`, resets counters and buffers.
2. Press **Esc** to lock cursor (look mode).
3. **Reset Camera** if needed — unlock with Esc first if you are in look mode.
4. Perform **one continuous horizontal 360°** in a single direction without reversing.
5. Press **Esc** to unlock cursor, then **End Validation**.
6. **Reset Counters** — retry within the same session (clears live counters and in-memory buffers).

Default settings: DPI **3200**, sensitivity **0.09**, horizontal FOV 103° → **eDPI = 288**.

DPI does not change yaw math; it only affects eDPI / cm/360 metadata. Sensitivity alone sets counts/360.

## Test

```bash
cargo test --workspace
```

Runs unit tests for `sense-math`, `sense-types`, `sense-input-win`, `sense-telemetry`, and app modules (no GPU required for most tests).

## Database

SQLite file: `data/sense_maxer.db` (created on first validation; `data/` is gitignored).

Inspect after a run:

```bash
sqlite3 data/sense_maxer.db ".tables"
sqlite3 data/sense_maxer.db "SELECT session_id, error_percent, pipeline_suspect FROM validation_results;"
sqlite3 data/sense_maxer.db "SELECT processor_id, COUNT(*) FROM processed_mouse_events GROUP BY processor_id;"
```

## Status

M1 Validation Lab complete (`docs/M1_COMPLETION_REPORT.md`). M2 processor framework complete (`docs/M2_PROCESSOR.md`). M2.x Phase 1.1 (`rawaccel_linear` v1.1.0, Gain + caps) complete (`docs/M2X_RAWACCEL_LINEAR.md`). **Stopped** before Natural/anisotropy/LUT/driver comparison and STATIC_CLICK.

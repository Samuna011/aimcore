# sense-maxer

Windows-native Bevy VALORANT Input Validation Lab (M1).

M1 proves the pipeline end-to-end:

```
RAW MOUSE INPUT → SENSITIVITY MODEL → CAMERA YAW → NATIVE 3D RENDER → TELEMETRY → SQLITE
```

See `docs/` for architecture, input model, telemetry schema, experiment design, the **locked baseline** (`docs/BASELINE.md`), and the M1 completion report.

**M1 status:** COMPLETE / STOPPED. Experimental baseline locked (VSync **ON** / FPS capped to refresh, WM_INPUT, QPC, NoAcceleration, pitch disabled, yaw 0.07 uncertain, HFOV 103°, DPI/sens configurable). Uncapped VSync-off was a temporary M1 verification only.

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
- Press **Esc** to enter **look mode** (cursor locked) for yaw / 360° movement.
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
```

## Status

M1 Validation Lab complete. See `docs/M1_COMPLETION_REPORT.md`. **Stopped** before STATIC_CLICK and later phases.

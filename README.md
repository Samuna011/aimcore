# sense-maxer

Windows-native Bevy VALORANT Input Validation Lab (M1).

M1 proves the pipeline end-to-end:

```
RAW MOUSE INPUT → SENSITIVITY MODEL → CAMERA YAW → NATIVE 3D RENDER → TELEMETRY → SQLITE
```

See `docs/` for architecture, input model, telemetry schema, experiment design, and the M1 completion report.

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

Opens the Validation Lab window (1280×720 default). Focus the window to lock the cursor.

### Validation Lab workflow

1. **Start Validation** — creates session/config in `data/sense_maxer.db`, resets counters and buffers.
2. **Reset Camera** — sets yaw to 0° (optional).
3. Perform **one continuous horizontal 360°** in a single direction without reversing.
4. **Reset Counters** — retry within the same session (clears live counters and in-memory buffers).
5. **End Validation** — flushes telemetry, shows expected vs observed counts/degrees, integrity counters, and pipeline-suspect flag.

Default settings: DPI 1600, sensitivity 0.175, horizontal FOV 103°.

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

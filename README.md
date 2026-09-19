# sense-maxer

Windows-native Bevy VALORANT Input Validation Lab (M1 + M2).

Pipeline:

```
RAW MOUSE INPUT → INPUT PROCESSOR → SENSITIVITY MODEL → CAMERA YAW → NATIVE 3D RENDER → TELEMETRY → SQLITE
```

See `docs/` for architecture, input model, telemetry schema, experiment design, the **locked baseline** (`docs/BASELINE.md`), M1 completion report, **M2 processor framework** (`docs/M2_PROCESSOR.md`), and **M2.x Raw Accel Linear** (`docs/M2X_RAWACCEL_LINEAR.md`).

**M1 status:** COMPLETE / STOPPED. Experimental baseline: VSync **OFF** / uncapped present (adopted exp `0.5.2` after latency feel A/B), WM_INPUT, QPC, yaw 0.07 uncertain, HFOV 103°, DPI/sens configurable.

**M2 status:** COMPLETE. InputProcessor framework with dual telemetry (raw + `processed_mouse_events`).

**M2.x status:** Phase 1.1 COMPLETE / STOPPED. Processors: `none` and `rawaccel_linear` (v1.1.0, Gain + caps). Documents mathematical behavior 1:1 with official source — not actual-driver comparison. **Stopped** before Natural/anisotropy/LUT/driver comparison.

**M3 status:** UnverifiedPitchModel enabled. Look mode applies yaw **and** pitch (same 0.07; +dy look down; ±89°; **UNCERTAIN**). STATIC_CLICK v1 (5-hit front-cone timed run). See [pitch design spec](docs/superpowers/specs/2026-09-17-unverified-pitch-model-design.md) and [research findings](docs/superpowers/specs/2026-09-17-valorant-pitch-research-findings.md).

**M3.x status:** COMPLETE / STOPPED. Completed STATIC_CLICK trials persist to SQLite (`aim_trials` + `aim_shots`); HUD shows score and `saved <trial_id>`. Aborts never insert. See [persistence design spec](docs/superpowers/specs/2026-09-18-m3x-aim-trial-persistence-design.md).

**M3.y status:** COMPLETE / STOPPED. Full reconstructable aim telemetry on completed STATIC_CLICK runs: five SQLite tables (`aim_trials`, `aim_target_events`, `aim_shots`, `aim_input_samples`, `aim_camera_samples`); deterministic `random_seed` at Start; raw `dt_ns` vs processor `dt_used_ns` on input samples. See [telemetry data model spec](docs/superpowers/specs/2026-09-19-m3y-aim-telemetry-data-model-design.md).

**M4.a status:** COMPLETE / STOPPED. GRIDSHOT v1 (60s, 3 concurrent exclusive 3×3 cells, vacant-cell respawn) on the M3.y five-layer model; HUD Start Gridshot + time/hits/accuracy/live; multi-sphere render sync. See [GRIDSHOT design spec](docs/superpowers/specs/2026-09-19-m4a-gridshot-design.md).

**Lab UI shell status:** COMPLETE / STOPPED. Lobby / Playing / Paused screen machine; Esc enters pause (never aborts Armed trials); active trial duration excludes pause time; Settings and Lab tools from Lobby or Pause Home; V detail overlay. `experiment_version` **`0.10.0`**. See [Lab UI shell design spec](docs/superpowers/specs/2026-09-19-lab-ui-shell-design.md).

**Aim History replay status:** COMPLETE / STOPPED. History list of completed aim trials; read-only 3D arena replay reconstructs camera, targets, and hit flashes from stored telemetry (no schema change); Esc pauses replay transport. `experiment_version` **`0.11.0`**. See [History replay design spec](docs/superpowers/specs/2026-09-19-aim-history-replay-design.md). **Stop** — no completeness charts, validation replay, or delete; do not add tracking / 1wall6 until a new task spec.

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
- After **Start Validation**, the app enters **Validating**: press **Esc** to toggle **look mode** (cursor locked) for yaw/pitch look and horizontal 360° sampling; press **Esc** again to unlock. This never pauses an aim trial.
- In **aim tasks** (Playing): **Esc** pauses the trial (clock frozen, no shots); **Esc** again resumes. Esc never cancels an Armed trial — use Restart / Change trial / Exit from Pause Home.
- While the cursor is unlocked, mouse movement is not applied to the camera and is not counted toward validation or aim gameplay.

### Validation Lab workflow

1. (UI mode) **Start Validation** — creates session/config in `data/sense_maxer.db`, resets counters and buffers.
2. Press **Esc** to lock the cursor (Validating look mode).
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

M1 Validation Lab complete (`docs/M1_COMPLETION_REPORT.md`). M2 processor framework complete (`docs/M2_PROCESSOR.md`). M2.x Phase 1.1 (`rawaccel_linear` v1.1.0, Gain + caps) complete (`docs/M2X_RAWACCEL_LINEAR.md`). M3 STATIC_CLICK + M3.x/M3.y aim telemetry + M4.a GRIDSHOT v1 + **Lab UI shell** + **Aim History replay** complete — **STOPPED** at exp `0.11.0`. Next: new task spec (tracking / 1wall6) or M4 compare conditions.

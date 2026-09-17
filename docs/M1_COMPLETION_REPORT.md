# M1 Completion Report — VALORANT Input Validation Lab

**Date:** 2026-09-17  
**App version:** 0.1.0  
**Experiment:** `validation_lab` / `0.1.0`  
**Status:** M1 complete — **STOPPED** before STATIC_CLICK or later phases.

---

## 1. Final Architecture

Single Windows-native Bevy process with thin pure-Rust crates:

```
RAW MOUSE INPUT (WM_INPUT) → MouseQueue → drain every sample → sense-math yaw → 3D camera + egui HUD
                                                          ↓
                                              in-memory buffers (while Running)
                                                          ↓
                                              SQLite flush on End Validation
```

| Crate / module | Role |
|----------------|------|
| `sense-math` | VALORANT yaw formulas (no Bevy) |
| `sense-types` | `MouseSample`, `InputCameraSample`, session/config/validation types |
| `sense-input-win` | QPC timestamps, `WM_INPUT` parsing, queue, integrity tracker |
| `sense-telemetry` | In-memory buffers + batched SQLite writer |
| `src/` (binary) | Bevy app: scene, camera, egui Validation Lab, session lifecycle |

Stack: **Bevy 0.19.1**, **bevy_egui 0.42.0**, **windows 0.62.2**, **raw-window-handle 0.6.2**.

---

## 2. Renderer Chosen and Why

**Bevy 0.19.1** with native wgpu rendering.

- Single-process native 3D FPS view without browser/DOM on the real-time path.
- Integrates with winit for Win32 HWND access required by raw input.
- egui overlay via `bevy_egui 0.42.0` in the same window for Validation Lab HUD.

---

## 3. Native Input API and Why

**Windows `WM_INPUT`** raw relative mouse movement via **`SetWindowSubclass`** on the Bevy winit HWND.

- Delivers hardware-reported relative counts independent of cursor warping and pointer acceleration on the experimental path.
- Subclass proc calls `handle_wm_input`, then forwards to `DefSubclassProc` so winit remains functional.
- Cursor position deltas (`GetCursorPos`-style) and Bevy `MouseMotion` are **not** used.

Hook implementation: `src/input_plugin.rs` (`install_raw_input_hook`, subclass ID `0x5345_4E53_455F_5241`).

---

## 4. Input Timestamp Precision

**QueryPerformanceCounter (QPC)** converted to nanoseconds (`sense-input-win/src/clock.rs`).

- Monotonic; suitable for per-sample ordering and integrity checks.
- Session start/end metadata uses Unix wall-clock milliseconds.
- FPS / frame time is a separate stream (`FrameSample`); not used as input timestamps.

---

## 5. Sensitivity Implementation

All formulas in `crates/sense-math/src/lib.rs`:

```
degrees_per_count = sensitivity × 0.07
yaw_delta_deg     = raw_dx × sensitivity × 0.07
eDPI              = DPI × sensitivity
counts_per_360    = 360 / (sensitivity × 0.07)
cm_per_360        = counts_per_360 / DPI × 2.54
```

Default config: DPI 1600, sensitivity 0.175, eDPI 280. Yaw constant `0.07` marked **UNCERTAIN** (see `docs/VALORANT_INPUT_MODEL.md`). M1 does not auto-compensate for discrepancy.

Acceleration: `NoAcceleration` identity stub behind `InputProcessor` trait.

---

## 6. FOV Implementation

Horizontal FOV 103° stored in `ExperimentSettings`. Vertical FOV derived at runtime:

```
vfov = 2 × atan(tan(hfov / 2) / aspect)
```

Implemented in `src/fov.rs`; updated on window resize via `maintain_horizontal_fov`. Angular sensitivity model is independent of FOV/resolution changes.

---

## 7. Camera Implementation

- Component `YawPitch { yaw_deg, pitch_deg }` on `Camera3d`.
- Yaw updated per drained sample: `yaw_deg += yaw_delta_deg(processed_dx, sensitivity)`.
- Reset Camera records an `InputCameraSample` while validation is running; sequence `0` is reserved for this synthetic reset sample because raw mouse sequences start at `1`.
- Pitch frozen at default (0°); vertical mouse input recorded but does not rotate camera.
- Transform applied via `Quat::from_rotation_y(-yaw_deg)`.
- Simple scene: floor plane, placeholder cube, egui crosshair overlay.
- Cursor: starts unlocked (UI mode); **Esc** locks for look / 360°; **Esc** again unlocks for menu buttons. While unlocked, queued samples are drained and discarded (not applied to camera or validation counters).

---

## 8. Telemetry Pipeline

1. While `ValidationState::Running`: each drained sample pushes to in-memory `SessionBuffers` (mouse + input camera); each frame pushes `FrameSample`.
2. Net counters (`net_dx`, `abs_dx`, etc.) accumulate only during running state.
3. On **End Validation**: `TelemetryDb::complete_validation` uses one transaction to flush all buffers, insert `validation_results`, and update the session end time.
4. On **Reset Counters**: zero live counters, clear in-memory buffers, and reset `IntegrityTracker` (no SQLite delete).
5. Raw-input read failures are baselined at Start Validation. The session delta is added to `sequence_gaps` at End Validation because each failed `WM_INPUT` read represents a missing input sample; any delta therefore marks the pipeline suspect without changing the schema.

Database path: **`data/sense_maxer.db`**.

---

## 9. Database Schema

Six M1 tables (see `docs/TELEMETRY_SCHEMA.md`):

| Table | Purpose |
|-------|---------|
| `configurations` | Settings snapshot + JSON |
| `sessions` | Session metadata |
| `raw_mouse_events` | Immutable raw input |
| `input_camera_samples` | Post-sample yaw/pitch state |
| `frame_samples` | Render timing |
| `validation_results` | Math + integrity report |

Prepared statements; batched inserts in one transaction per End Validation.

---

## 10. Validation Tests

Automated (`cargo test --workspace`):

| Area | Tests |
|------|-------|
| `sense-math` | deg/count, eDPI, counts/cm per 360, same-eDPI equivalence, FOV independence |
| `sense-types` | serde round-trip, integrity pipeline-suspect |
| `sense-input-win` | queue ordering, integrity gaps/duplicates/OOO/regressions |
| `sense-telemetry` | DB migrate + round-trip flush |
| `src/` | session IDs, validation math, UTC date, yaw/pitch, frame sample, FOV |

Manual (operator): 360° horizontal rotation in Validation Lab; confirm HUD results and SQLite rows.

---

## 11. Results of 360° Validation

**Date recorded:** 2026-09-17  
**Operator:** developer (first subject)  
**Method:** human-performed continuous horizontal 360° (single direction)

| Field | Value |
|-------|-------|
| DPI / Sensitivity / eDPI | 1600 / 0.175 / 280 (default) |
| Expected counts | 29387.755102 |
| Observed net counts | +29380.000000 |
| Observed abs path counts | 29422.000000 |
| Expected degrees | 360.000000 |
| Observed degrees | +359.905000 |
| Count difference | −7.755102 |
| Error % | −0.026389% |
| Samples received | 4836 |
| Sequence gaps | 0 |
| Duplicate sequences | 0 |
| Out of order | 0 |
| Timestamp regressions | 0 |
| Pipeline suspect | **false** |

### Interpretation

- **Pipeline health:** Integrity counters are all zero and `pipeline_suspect` is false. This run does **not** indicate an input-pipeline fault (no gaps, duplicates, OOO, or timestamp regressions).
- **Angular model:** Observed ≈ 359.905° vs expected 360°. Net shortfall ≈ **7.8 counts** (~0.026%). That is consistent with **human aiming / stopping error** on a freehand 360°, not with a broken `0.07` yaw formula (a wrong yaw constant would typically produce a much larger systematic bias).
- **Abs path vs net:** Abs path (29422) > net (29380) by 42 counts implies a small amount of reverse motion or micro-corrections during the sweep. Validation correctly uses **signed net** for the discrepancy; abs path is telemetry only.
- **Do not “fix” the model from this:** Per project rules, do not silently compensate the formula to erase a human 360° miss. Treat this as evidence that the instrument is measuring and reporting, not as a calibration target to zero out.

**Residual uncertainty:** A human 360° cannot prove bit-exact VALORANT equivalence. Stronger confirmation would need a mechanical/reference rotation or a side-by-side count comparison against VALORANT under identical DPI/sens — still separate from this healthy first instrument run.

---

## 12. Known Uncertainties

| Item | Status |
|------|--------|
| Yaw constant `0.07` | **UNCERTAIN** — community-derived; not official Riot documentation |
| VALORANT pitch model | **UNVERIFIED** — pitch rotation disabled in M1 |
| Refresh rate detection | Passed as `None` at session start; stored as 0 Hz if undetected |
| End-to-end display latency | Not measured in M1 |

---

## 13. Known Limitations

- Windows only.
- No CSV/JSON export.
- No aim tasks (STATIC_CLICK, flick, tracking, etc.).
- No acceleration curves beyond identity stub.
- No pitch camera control.
- No multi-user/device tables.
- No `RenderCameraSample` (render-frame pose).
- Camera yaw updates live outside validation sessions, but counters and telemetry buffer only during Running.
- Reset Counters clears in-memory buffers only; prior flushed rows remain in SQLite.

---

## 14. Files Created / Modified (M1)

### Workspace root

- `Cargo.toml` — Bevy 0.19.1, bevy_egui 0.42.0, workspace members
- `README.md`

### `src/` (Bevy binary)

- `main.rs`, `app.rs`, `config.rs`, `session.rs`, `validation_lab.rs`
- `camera_ctrl.rs`, `frame_telemetry.rs`, `input_plugin.rs`, `scene.rs`, `fov.rs`

### `crates/`

- `sense-math/` — formulas + unit tests
- `sense-types/` — shared types + tests
- `sense-input-win/` — WM_INPUT queue, QPC clock, integrity + tests
- `sense-telemetry/` — buffers, SQLite schema, flush + round-trip test

### `docs/`

- `ARCHITECTURE.md`, `VALORANT_INPUT_MODEL.md`, `TELEMETRY_SCHEMA.md`, `EXPERIMENT_MODEL.md`, `M1_COMPLETION_REPORT.md`

---

## Stop Checkpoint

M1 Validation Lab is functionally validated on hardware with a clean integrity report and ~0.026% human 360° error. **Do not proceed** to STATIC_CLICK, movement segmentation, ML optimization, or export tooling until you explicitly approve the next phase.

The `0.07` yaw constant remains **UNCERTAIN** pending stronger VALORANT-side confirmation; this run does not justify changing it.

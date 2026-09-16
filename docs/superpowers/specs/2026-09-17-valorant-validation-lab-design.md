# VALORANT Input Validation Lab — Design Spec (M1)

**Date:** 2026-09-17  
**Status:** Draft for user review  
**Scope:** Milestone 1 only — stop after Validation Lab; no aim tasks

---

## 1. Purpose

Build a controlled experimental instrument for long-term mouse-input / sensitivity research. The first deliverable is **not** a commercial aim trainer.

M1 proves this pipeline end-to-end:

```
RAW MOUSE INPUT → SENSITIVITY MODEL → CAMERA YAW → NATIVE 3D RENDER → TELEMETRY → SQLITE
```

Primary subject: the developer. Priorities: input accuracy, low/predictable input-to-camera latency, mathematical VALORANT yaw equivalence, raw telemetry integrity, reproducibility.

---

## 2. Decisions Locked In

| Topic | Decision |
|-------|----------|
| Platform (M1) | Windows only |
| Renderer | Bevy + native GPU |
| Validation HUD | egui in the same native window |
| Mouse input | Windows `WM_INPUT` (raw relative) |
| Pitch (M1) | Telemetry only; camera pitch rotation **disabled** |
| Architecture style | Bevy app + thin pure-Rust crates for math/types/telemetry/input |
| Acceleration (M1) | `NoAcceleration` stub only; modular trait reserved |
| Research web UI | Out of scope for M1 |

---

## 3. System Architecture

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

## 4. Crate / Module Layout

```
sense-maxer/
  crates/
    sense-math/          # degrees/count, eDPI, cm/360, inches/360 (no Bevy)
    sense-types/         # MouseSample, InputCameraSample, Session, Configuration, IDs
    sense-telemetry/     # in-memory buffers + batched SQLite writer
    sense-input-win/     # WM_INPUT → timestamped queue
  src/                   # Bevy binary: scene, yaw camera, egui Validation Lab
  docs/
    ARCHITECTURE.md      # written in implementation (mirrors this spec)
    VALORANT_INPUT_MODEL.md
    TELEMETRY_SCHEMA.md
    EXPERIMENT_MODEL.md
```

Sensitivity formulas live only in `sense-math`. Bevy/UI/database never reimplement them.

---

## 5. VALORANT Yaw Model (M1)

```
degrees_per_count = sensitivity × 0.07
yaw_delta_deg     = raw_dx × degrees_per_count
```

Examples (internal, unrounded):

| Sensitivity | deg/count | 1000 counts |
|-------------|-----------|-------------|
| 0.15 | 0.0105 | 10.5° |
| 0.175 | 0.01225 | 12.25° |

Derived:

```
eDPI            = DPI × sensitivity
counts_per_360  = 360 / (sensitivity × 0.07)
inches_per_360  = counts_per_360 / DPI
cm_per_360      = inches_per_360 × 2.54
```

**DPI does not enter camera math.** DPI only affects how many counts the hardware emits for a physical distance.

Same eDPI configurations must share the same theoretical **cm/360**, but they do **not** necessarily share the same **counts/360**. Counts/360 depends on sensitivity; cm/360 depends on sensitivity and DPI.

| DPI | Sens | eDPI | counts/360 | cm/360 |
|-----|------|------|------------|--------|
| 800 | 0.35 | 280 | ≈14693.88 | ≈46.65 cm |
| 1600 | 0.175 | 280 | ≈29387.76 | ≈46.65 cm |
| 3200 | 0.0875 | 280 | ≈58775.51 | ≈46.65 cm |

(Internal math is unrounded; table values are illustrative.)

Default experimental config (not a recommendation):

- DPI 1600, sensitivity 0.175, eDPI 280  
- yaw constant 0.07°/count at sens 1.0  
- horizontal FOV 103°  
- acceleration OFF  
- resolution / refresh: detect current display when possible  

Provenance of `0.07` and confidence labels (CONFIRMED / DERIVED / EMPIRICALLY TESTED / UNCERTAIN) go in `docs/VALORANT_INPUT_MODEL.md`. Do not present assumptions as facts.

---

## 6. Pitch (M1)

- `raw_dy` is recorded in telemetry unchanged.
- Camera pitch is **not** updated from mouse input.
- egui HUD must show explicitly:

  - `PITCH MODEL: UNVERIFIED`
  - `PITCH ROTATION: DISABLED`

When pitch is enabled later, it must sit behind a named abstraction (e.g. `UnverifiedPitchModel`); M1 does not pretend VALORANT vertical behavior is verified.

---

## 7. FOV and Resolution

Profile fields:

- `fov_axis = Horizontal`
- `fov_degrees = 103`

Vertical FOV is derived from horizontal FOV and viewport aspect ratio. Do not hand-tune vertical FOV.

**Angular sensitivity model independence:**

Changing resolution or FOV must **not** change the angular sensitivity model (`degrees_per_count`, `counts_per_360`, `cm_per_360`).

FOV/resolution **may** change projection and screen-space representation (how many pixels a given angular movement subtends). That is expected and must not be “fixed” by altering yaw math.

Unit/resolution tests assert model independence; they do not assert screen-space invariance.

---

## 8. Input Path

### Capture

- Windows `WM_INPUT` raw relative mouse movement.
- Do not use cursor position deltas (`GetCursorPos`-style) as the experimental stream.

### Sample type (`MouseSample`)

| Field | Unit / notes |
|-------|----------------|
| `timestamp_ns` | monotonic high-res (QPC → ns) |
| `dx`, `dy` | mouse counts (raw) |
| `buttons` | button state |
| `sequence_number` | order |

Preserve additional platform fields if available. Do not mutate raw samples.

### InputCameraSample (not a render-frame sample)

| Field | Unit / notes |
|-------|----------------|
| `timestamp_ns` | same clock family as the mouse sample just applied |
| `yaw_deg` | degrees after applying that sample |
| `pitch_deg` | degrees (unchanged by mouse in M1) |
| `sequence_number` | mouse sample sequence that produced this state (when applicable) |

This is **input-derived state**, recorded immediately after applying one raw mouse sample. It is **not** the camera pose at render submit/present. A future `RenderCameraSample` must be a separate type/table.

### Queue discipline

```
RAW INPUT → timestamped queue → process every sample → camera yaw + telemetry → render
```

Input collection must continue even if rendering stalls. Frame timing is a separate stream (`FrameSample`).

### Clocks

- Movement intervals: monotonic (`QueryPerformanceCounter`).
- Session start/end metadata: wall clock allowed.
- Do not use FPS as a proxy for input timestamps.

### Latency claims (M1)

Architect for future multi-stage latency instrumentation. **Do not claim** end-to-end display latency measurement in M1.

---

## 9. Camera

True 3D FPS camera state: `position`, `yaw_deg`, `pitch_deg`.

M1:

- Yaw updated per raw sample: `yaw_delta = dx × sensitivity × 0.07`
- Pitch held constant (disabled from mouse)
- Angular representation is source of truth (not pixel→angle inference)

Simple scene: flat floor, simple background, crosshair, optional inert placeholder mesh(es). No visual polish.

---

## 10. Acceleration Abstraction (stub only)

```rust
trait InputProcessor {
    fn process(&mut self, dx: f64, dy: f64, dt_s: f64) -> (f64, f64);
}
```

M1 implements `NoAcceleration` (identity). Future: Linear, Classic, Natural, Power, Custom/LUT without changing camera or telemetry core.

When acceleration is eventually enabled, telemetry must keep both raw and processed dx/dy. M1 still stores raw; processed = raw under `NoAcceleration`.

---

## 11. Validation Lab (sole M1 experiment)

### HUD (egui)

Live display:

- DPI, Sensitivity, eDPI, Yaw (°/count), HFOV, Resolution, Refresh, cm/360, counts/360  
- raw dx, raw dy, total horizontal counts, total vertical counts  
- current yaw, current pitch (frozen), total degrees rotated (yaw)  
- pitch status banners (UNVERIFIED / DISABLED)

Controls:

- Reset Camera  
- Reset Counters  
- Start Validation  
- End Validation  

### Flow

1. **Start Validation** → allocate `session_id`, snapshot configuration + versions into SQLite  
2. **Reset Camera** → known forward yaw (e.g. 0)  
3. User performs **one continuous horizontal rotation in a single direction without reversing**  
4. Apply yaw sample-by-sample; accumulate signed net horizontal counts  
5. **End Validation** → using **the same** `sense-math` functions as the app:

   - Expected counts = `360 / (sensitivity × 0.07)`  
   - Observed counts = **signed** Σ `raw_dx` (net counts)  
   - Absolute path counts (Σ `|raw_dx|`) may be recorded separately as telemetry but **must not** replace signed net counts for the validation calculation  
   - Expected / observed degrees, difference, error %  

6. Persist raw events + derived validation result + **input-integrity report**  
7. **Do not** auto-compensate for discrepancy  

### Input-integrity reporting (360° / validation session)

Report (HUD + persisted with validation result) so math discrepancies can be distinguished from pipeline faults:

- Samples received  
- Sequence gaps  
- Duplicate sequences  
- Out-of-order samples  
- Timestamp regressions  

If integrity counters are non-zero, treat the run as **pipeline-suspect** in the report (still show math discrepancy; do not auto-correct).

Example at 1600 DPI / 0.175 (from `sense-math`, unrounded display may round for UI only):

- eDPI = 280  
- yaw/count = 0.01225°  
- counts/360 ≈ 29387.755…  
- cm/360 ≈ 46.65 cm (display rounding only)

---

## 12. Session & Configuration Model

- Session IDs e.g. `session_YYYYMMDD_NNNNNN`
- Config IDs e.g. `config_NNNNNN`
- Snapshot at session start: app version, experiment version (`validation_lab` / version), DPI, sensitivity, eDPI, FOV, resolution, refresh, accel state, polling rate if known, random seed (stored even if unused in M1)

Do not rely on “current settings” to reconstruct history.

---

## 13. SQLite (M1)

Tables (conceptual):

- `configurations`
- `sessions`
- `raw_mouse_events`
- `input_camera_samples`
- `frame_samples`
- `validation_results`

Performance: prepared statements, batched inserts, transactions. Never one transaction per mouse event.

**InputCameraSample (M1):** camera state **immediately after applying one raw mouse sample** (input-derived), not a render-frame camera pose. Cadence: one row per drained mouse sample that updates yaw, plus on camera/counter reset. Do not conflate with `FrameSample` (render path only). A future `RenderCameraSample` (pose at frame submit/present) is out of scope for M1 and must not be silently aliased to this table.

**Export (CSV/JSON):** out of scope for M1. Data must be queryable via SQLite; export lands in a later phase.

Out of M1 tables (documented as future in `TELEMETRY_SCHEMA.md` only): trials, movements, task_events, performance_metrics, users, devices.

**M1 requirement:** configurations, sessions, raw mouse, input camera samples, frame samples, validation results are implemented and used.

---

## 14. Units

| Quantity | Unit |
|----------|------|
| dx, dy | mouse counts |
| timestamp | nanoseconds (monotonic for samples) |
| yaw, pitch, FOV | degrees |
| DPI | counts/inch |
| velocity (future) | counts/second |
| cm/360 | centimeters |
| inches/360 | inches |

Never mix pixels, counts, and degrees without an explicit conversion.

---

## 15. Testing

`sense-math` unit tests (no GPU required):

- `degrees_per_count`, `eDPI`, `counts_per_360`, `cm_per_360`, `inches_per_360`
- Equivalence: 800×0.35, 1600×0.175, 3200×0.0875 → same eDPI and same cm/360; counts/360 must differ according to sensitivity
- Sensitivities 0.15, 0.175, 0.20
- No intermediate rounding
- FOV/resolution independence of the **angular sensitivity model** (not screen-space)

Manual: 360° validation run; report expected vs observed without compensation.

---

## 16. Explicitly Out of Scope (M1)

- STATIC_CLICK / flick / tracking / target switching  
- Movement segmentation & flick phase classifier  
- Acceleration curve implementations beyond identity stub  
- ML / Bayesian optimization / auto sensitivity search  
- Separate research UI process (Vue/Tauri/etc.)  
- CSV/JSON export tooling  
- Cross-platform input  
- Pitch camera control  
- Claiming display latency measurement  

---

## 17. Documentation Deliverables (implementation phase)

Create/update:

1. `docs/ARCHITECTURE.md` — mirrors this architecture; why each component exists  
2. `docs/VALORANT_INPUT_MODEL.md` — formulas + provenance + confidence  
3. `docs/TELEMETRY_SCHEMA.md` — tables, units, raw vs derived  
4. `docs/EXPERIMENT_MODEL.md` — session/config/versioning; Validation Lab as first experiment  

---

## 18. M1 Completion Report (required before next phase)

After Validation Lab works, stop and report:

1. Final architecture  
2. Renderer chosen and why  
3. Native input API and why  
4. Input timestamp precision  
5. Sensitivity implementation  
6. FOV implementation  
7. Camera implementation  
8. Telemetry pipeline  
9. Database schema  
10. Validation tests  
11. Results of 360° validation  
12. Known uncertainties  
13. Known limitations  
14. Files created/modified  

---

## 19. Success Criteria

- Bevy window renders simple 3D FPS view with egui HUD  
- `WM_INPUT` samples appear live (dx/dy) with independent timing from FPS  
- Yaw follows `dx × sens × 0.07` exactly in math module + camera  
- Pitch banners present; look up/down does not rotate camera  
- Unit tests for sensitivity math pass  
- Start/End Validation persists session + raw samples + discrepancy report + input-integrity counters  
- Changing FOV/resolution in tests does not change angular model outputs (projection/screen-space may change)  
- Same-eDPI unit tests assert shared cm/360 and **different** counts/360 by sensitivity

---

## Revision Notes

- **2026-09-17:** FOV/resolution wording clarified: angular sensitivity model is independent of FOV/resolution; projection and screen-space representation may change.
- **2026-09-17:** Export deferred from M1; input-derived camera sample cadence specified (per drained mouse sample).
- **2026-09-17:** Same-eDPI clarification: cm/360 matches across DPI/sens pairs; counts/360 does not (depends on sensitivity only). 360° validation uses signed net Σ dx; absolute path counts separate. Input-integrity counters added.
- **2026-09-17:** Renamed/documented `InputCameraSample` (post-mouse-sample camera state) vs future render-frame camera sampling.

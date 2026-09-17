# M2.x Phase 1 Raw Accel Linear Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `RawAccelLinear` (Linear + Legacy/Sensitivity) as an `InputProcessor` that demonstrates reproduction of the documented mathematical behavior from the Raw Accel Guide/source, with unit tests first and live wiring second.

**Architecture:** Extend `sense-accel` with pure math helpers, a debug eval result, and `RawAccelLinear`. Extend `create_processor` to take acceleration + sensitivity_multiplier (ignored for `none`). Wire Idle HUD + session snapshot; keep QPC `dt_s`, dual telemetry, and VSync ON. No LUT framework, Gain, caps, offsets, or driver comparison.

**Tech Stack:** Existing workspace; `sense-accel`; Bevy 0.19 + bevy_egui; SQLite unchanged schema for Phase 1.

**Spec:** `docs/superpowers/specs/2026-09-17-m2x-rawaccel-linear-design.md`

## Global Constraints

- Mode: **Linear + Legacy / Sensitivity** (not Gain); naming must never say “Classic² / Linear”
- Claim: demonstrates **documented mathematical behavior**; not actual-driver parity
- Speed: `v = sqrt((dx * dx) + (dy * dy)) / dt_ms` when `dt_ms > 0`
- Trainer boundary: `dt_ms <= 0` → identity; `input_speed=0`; `acceleration_scale=1`; `bypassed_nonpositive_dt=true`; multiplier **not** applied
- Trainer defaults: `acceleration=0.01` (trainer-only, not official RA default); `sensitivity_multiplier=1.0`
- `dt_s` from consecutive raw QPC only; `dt_ms = dt_s * 1000`
- VSync ON; no mid-session processor swap; raw table immutable
- No smoothing, anisotropy, caps, offsets, DPI norm, LUT, Classic/Gain modes, driver dependency
- Unit tests green before trusting live trainer; validation run is confirmation only
- Stop after Phase 1

---

## File Structure

```
crates/sense-accel/src/lib.rs          # helpers, LinearEval, RawAccelLinear, factory
crates/sense-accel/tests/processor_tests.rs   # extend with Linear suite
src/config.rs                          # + acceleration, sensitivity_multiplier
src/session.rs                         # create_processor with config; experiment_version bump
src/validation_lab.rs                  # combo + param editors; HUD note
src/camera_ctrl.rs                     # create_processor("none", …) call sites if signature changes
docs/M2X_RAWACCEL_LINEAR.md            # provenance + confidence
docs/M2_PROCESSOR.md                   # mention rawaccel_linear exists
docs/ARCHITECTURE.md                   # one-line processor list update
```

---

### Task 1: Pure Linear math + debug eval + mandatory unit suite

**Files:**
- Modify: `crates/sense-accel/src/lib.rs`
- Modify: `crates/sense-accel/tests/processor_tests.rs`

**Interfaces:**
- Produces:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct LinearEval {
    pub raw_dx: f64,
    pub raw_dy: f64,
    pub dt_ms: f64,
    pub input_speed: f64,
    pub acceleration_scale: f64,
    pub processed_dx: f64,
    pub processed_dy: f64,
    pub bypassed_nonpositive_dt: bool,
}

pub fn vector_speed_counts_per_ms(dx: f64, dy: f64, dt_ms: f64) -> f64;
pub fn linear_acceleration_scale(v: f64, acceleration: f64) -> f64;
pub fn apply_whole(dx: f64, dy: f64, acceleration_scale: f64, sensitivity_multiplier: f64) -> (f64, f64);
pub fn eval_rawaccel_linear(dx: f64, dy: f64, dt_s: f64, acceleration: f64, sensitivity_multiplier: f64) -> LinearEval;
```

Semantics for `eval_rawaccel_linear`:
- `dt_ms = dt_s * 1000.0`
- if `dt_ms <= 0.0`: return identity processed = raw; `input_speed=0`; `acceleration_scale=1`; `bypassed_nonpositive_dt=true`
- else: `input_speed = sqrt((dx*dx)+(dy*dy)) / dt_ms`; `acceleration_scale = 1.0 + acceleration * input_speed`; processed = apply_whole(...)

- [ ] **Step 1: Write failing tests** in `crates/sense-accel/tests/processor_tests.rs` (keep existing `none` / `dt_s` tests)

```rust
#[test]
fn guide_vector_30_40() {
    let e = eval_rawaccel_linear(30.0, 40.0, 0.001, 0.01, 0.5);
    assert!(!e.bypassed_nonpositive_dt);
    assert!((e.dt_ms - 1.0).abs() < 1e-12);
    assert!((e.input_speed - 50.0).abs() < 1e-9);
    assert!((e.acceleration_scale - 1.5).abs() < 1e-12);
    assert!((e.processed_dx - 22.5).abs() < 1e-9);
    assert!((e.processed_dy - 30.0).abs() < 1e-9);
}

#[test]
fn zero_vector_stable() {
    let e = eval_rawaccel_linear(0.0, 0.0, 0.001, 0.01, 0.5);
    assert_eq!((e.processed_dx, e.processed_dy), (0.0, 0.0));
}

#[test]
fn nonpositive_dt_trainer_bypass() {
    let e = eval_rawaccel_linear(30.0, 40.0, 0.0, 0.01, 0.5);
    assert!(e.bypassed_nonpositive_dt);
    assert_eq!(e.input_speed, 0.0);
    assert_eq!(e.acceleration_scale, 1.0);
    assert_eq!((e.processed_dx, e.processed_dy), (30.0, 40.0));
}

#[test]
fn axis_aligned_whole_not_by_component() {
    let e = eval_rawaccel_linear(50.0, 0.0, 0.001, 0.01, 0.5);
    assert!((e.processed_dx - 37.5).abs() < 1e-9);
    assert_eq!(e.processed_dy, 0.0);
}

#[test]
fn zero_acceleration_keeps_multiplier() {
    let e = eval_rawaccel_linear(30.0, 40.0, 0.001, 0.0, 0.5);
    assert!((e.acceleration_scale - 1.0).abs() < 1e-12);
    assert!((e.processed_dx - 15.0).abs() < 1e-9);
    assert!((e.processed_dy - 20.0).abs() < 1e-9);
}

#[test]
fn negative_vector_preserves_direction() {
    let e = eval_rawaccel_linear(-30.0, -40.0, 0.001, 0.01, 0.5);
    assert!((e.processed_dx - (-22.5)).abs() < 1e-9);
    assert!((e.processed_dy - (-30.0)).abs() < 1e-9);
}
```

- [ ] **Step 2: Run tests — expect FAIL**

```bash
cargo test -p sense-accel --test processor_tests
```

Expected: compile/link failure or missing `eval_rawaccel_linear`.

- [ ] **Step 3: Implement helpers + `LinearEval` + `eval_rawaccel_linear` in `lib.rs`**

Document in comments (near the Linear entry point):

```rust
/// Raw Accel Linear (Legacy / Sensitivity), reproduced according to the official
/// implementation; mathematically equivalent to Classic with exponent 2 where the
/// documented equivalence applies.
/// Demonstrates documented mathematical behavior — not actual-driver parity.
```

Speed must use exactly:

```rust
(dx * dx + dy * dy).sqrt() / dt_ms
```

- [ ] **Step 4: `cargo test -p sense-accel` — expect PASS**

- [ ] **Step 5: Commit**

```bash
git add crates/sense-accel/src/lib.rs crates/sense-accel/tests/processor_tests.rs
git commit -m "feat(sense-accel): Raw Accel Linear math and Guide unit suite"
```

---

### Task 2: `RawAccelLinear` processor + factory with config params

**Files:**
- Modify: `crates/sense-accel/src/lib.rs`
- Modify: `crates/sense-accel/tests/processor_tests.rs`
- Modify: `src/session.rs` (call-site compile fix if done in this task; otherwise Task 3)
- Modify: `src/camera_ctrl.rs` (default `create_processor("none", …)`)
- Modify: `src/validation_lab.rs` (Idle version probe uses new signature)

**Interfaces:**
- Produces:

```rust
pub struct RawAccelLinear {
    pub acceleration: f64,
    pub sensitivity_multiplier: f64,
}

impl InputProcessor for RawAccelLinear {
    fn id(&self) -> &'static str { "rawaccel_linear" }
    fn version(&self) -> &'static str { "1.0.0" }
    fn config_json(&self) -> String { /* {"acceleration":...,"sensitivity_multiplier":...} */ }
    fn process(&mut self, dx: f64, dy: f64, dt_s: f64) -> (f64, f64) {
        let e = eval_rawaccel_linear(dx, dy, dt_s, self.acceleration, self.sensitivity_multiplier);
        (e.processed_dx, e.processed_dy)
    }
}

pub fn create_processor(
    id: &str,
    acceleration: f64,
    sensitivity_multiplier: f64,
) -> Result<Box<dyn InputProcessor>, String> {
    match id {
        "none" => Ok(Box::new(NoAcceleration)),
        "rawaccel_linear" => Ok(Box::new(RawAccelLinear {
            acceleration,
            sensitivity_multiplier,
        })),
        _ => Err(format!("unknown processor id: {id}")),
    }
}
```

`config_json` must be stable/reproducible (serde_json object with both fields, or `format!` with full precision). Prefer `serde_json` if already a dependency of `sense-accel`; otherwise add it or use deterministic `format!("{{\"acceleration\":{},\"sensitivity_multiplier\":{}}}", …)`.

- [ ] **Step 1: Write failing factory / process tests**

```rust
#[test]
fn factory_rawaccel_linear_guide_process() {
    let mut p = create_processor("rawaccel_linear", 0.01, 0.5).unwrap();
    assert_eq!(p.id(), "rawaccel_linear");
    assert_eq!(p.version(), "1.0.0");
    let (ox, oy) = p.process(30.0, 40.0, 0.001);
    assert!((ox - 22.5).abs() < 1e-9);
    assert!((oy - 30.0).abs() < 1e-9);
}

#[test]
fn factory_none_ignores_accel_params() {
    let mut p = create_processor("none", 0.01, 0.5).unwrap();
    assert_eq!(p.process(3.0, -2.0, 0.001), (3.0, -2.0));
}

#[test]
fn factory_still_rejects_unknown() {
    assert!(create_processor("raw_accel", 0.0, 1.0).is_err());
}
```

Update existing `factory_accepts_none_rejects_unknown` to the new signature.

- [ ] **Step 2: Run `cargo test -p sense-accel` — expect FAIL**

- [ ] **Step 3: Implement `RawAccelLinear` + new `create_processor` signature**

- [ ] **Step 4: Fix all in-repo call sites so workspace compiles**

Replace:
- `create_processor("none")` → `create_processor("none", 0.0, 1.0)` (or settings fields)
- `create_processor(&settings.processor_id)` → pass `settings.acceleration`, `settings.sensitivity_multiplier`

If `ExperimentSettings` does not yet have those fields, use temporary literals `0.01` / `1.0` only in `camera_ctrl` default resource; Session/HUD wiring lands in Task 3. Prefer adding the fields in Task 3 and using `0.0, 1.0` placeholders only where settings are unavailable.

Minimal compile-fix pattern for this task:

```rust
create_processor(id, 0.01, 1.0)
```

Task 3 replaces placeholders with real settings.

- [ ] **Step 5: `cargo test -p sense-accel` and `cargo test --workspace` PASS**

- [ ] **Step 6: Commit**

```bash
git add crates/sense-accel src
git commit -m "feat(sense-accel): RawAccelLinear processor and factory config"
```

---

### Task 3: Experiment settings + session + HUD wiring

**Files:**
- Modify: `src/config.rs`
- Modify: `src/session.rs`
- Modify: `src/validation_lab.rs`
- Modify: `src/camera_ctrl.rs` (use settings if available at construction; else keep `none` default)

**Interfaces:**
- Extend `ExperimentSettings`:

```rust
pub struct ExperimentSettings {
    pub dpi: f64,
    pub sensitivity: f64,
    pub fov_degrees_h: f64,
    pub processor_id: String,
    pub acceleration: f64,              // trainer default 0.01
    pub sensitivity_multiplier: f64,    // trainer default 1.0
}
```

- `start_validation` calls:

```rust
let processor = sense_accel::create_processor(
    &settings.processor_id,
    settings.acceleration,
    settings.sensitivity_multiplier,
)?;
```

- Bump `EXPERIMENT_VERSION` to `0.3.0` in `session.rs`.
- When `accel.enabled`: set `enabled: settings.processor_id != "none"` (or `true` for `rawaccel_linear`).

- [ ] **Step 1: Update `ExperimentSettings` defaults + unit test**

```rust
acceleration: 0.01,              // trainer default only — not official Raw Accel default
sensitivity_multiplier: 1.0,
```

Add test asserting those defaults.

- [ ] **Step 2: Wire `session.rs` + replace temporary factory args**

- [ ] **Step 3: HUD**

- Combo: `none` and `rawaccel_linear` (Idle only)
- When `rawaccel_linear` selected (Idle): DragValues for `acceleration` and `sensitivity_multiplier`
- Note: `Linear + Legacy/Sensitivity`; under `none`, processed == raw
- Running: show snapshotted active processor id/version; lock editors

Example combo addition:

```rust
ui.selectable_value(&mut settings.processor_id, "none".into(), "none");
ui.selectable_value(
    &mut settings.processor_id,
    "rawaccel_linear".into(),
    "rawaccel_linear",
);
```

Idle version probe:

```rust
sense_accel::create_processor(
    &settings.processor_id,
    settings.acceleration,
    settings.sensitivity_multiplier,
)
```

- [ ] **Step 4: `cargo test --workspace` PASS**

- [ ] **Step 5: Manual smoke (confirmation only)** — Start Validation with `rawaccel_linear`, move mouse, End; confirm processed rows exist with `processor_id=rawaccel_linear` and config_json containing both fields. Not a math proof.

- [ ] **Step 6: Commit**

```bash
git add src/config.rs src/session.rs src/validation_lab.rs src/camera_ctrl.rs
git commit -m "feat(app): wire rawaccel_linear settings, session, and HUD"
```

---

### Task 4: Docs + stop gate

**Files:**
- Create: `docs/M2X_RAWACCEL_LINEAR.md`
- Modify: `docs/M2_PROCESSOR.md`
- Modify: `docs/ARCHITECTURE.md` (processor list only)
- Modify: `README.md` only if it lists processors

**Content for `docs/M2X_RAWACCEL_LINEAR.md` (required):**
- Mode: Linear + Legacy/Sensitivity
- Provenance table from spec (CONFIRMED / DERIVED / EXPLICIT)
- Formula: `v = sqrt((dx * dx) + (dy * dy)) / dt_ms`
- Trainer `dt_ms <= 0` boundary
- Trainer defaults vs official RA defaults
- “Documents mathematical behavior; not actual-driver comparison”
- Out of scope list (Gain, caps, LUT, …)
- Stop after Phase 1

- [ ] **Step 1: Write `docs/M2X_RAWACCEL_LINEAR.md`**

- [ ] **Step 2: Patch `M2_PROCESSOR.md`** — note M2.x added `rawaccel_linear`; link the new doc; keep M2 `none` description.

- [ ] **Step 3: Patch `ARCHITECTURE.md`** processor mention.

- [ ] **Step 4: Commit**

```bash
git add docs/M2X_RAWACCEL_LINEAR.md docs/M2_PROCESSOR.md docs/ARCHITECTURE.md README.md
git commit -m "docs: M2.x Raw Accel Linear provenance and stop gate"
```

- [ ] **Step 5: STOP** — do not start Natural/Classic/Gain/LUT/driver comparison without a new spec.

---

## Spec Coverage Checklist

| Spec requirement | Task |
|------------------|------|
| Linear + Legacy/Sensitivity naming | 1, 2, 4 |
| Unambiguous speed formula | 1 |
| Trainer `dt_ms<=0` bypass + debug fields | 1 |
| Guide + 5 other mandatory tests | 1 |
| `RawAccelLinear` + factory id/version/config | 2 |
| Trainer defaults a=0.01 / m=1.0 | 3 |
| HUD select + param edit Idle-only | 3 |
| Session snapshot / dual telemetry | 3 (existing path) |
| Provenance docs + confidence | 4 |
| No LUT/Gain/caps/driver | all (out of scope) |
| Unit tests before live trust | Task 1 before 3 |

## Placeholder / consistency self-check

- Factory signature is consistently three-arg across Tasks 2–3.
- Bypass does **not** apply `sensitivity_multiplier`.
- No DB columns for `input_speed` / `acceleration_scale` in this plan.

# M2.x Phase 1.1 Raw Accel Linear Gain + Caps Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend `rawaccel_linear` so Gain and cap modes (`out` / `in` / `io`) match official Raw Accel classic Linear math 1:1, with Phase 1 Sensitivity/no-cap kept as a configurable verification path.

**Architecture:** Add `classic_linear.rs` as a faithful port of `accel-classic.hpp` (LEGACY + GAIN, exponent fixed at 2). `RawAccelLinear` takes a `RawAccelLinearConfig`, precomputes classic state in `new`, and evaluates scale then × multiplier. App settings/HUD expose Gain, offset, cap mode, and cap x/y; factory accepts the config struct.

**Tech Stack:** Existing workspace; `sense-accel`; Bevy 0.19 + bevy_egui; no new dependencies required.

**Spec:** `docs/superpowers/specs/2026-09-17-m2x-rawaccel-linear-gain-caps-design.md`

## Global Constraints

- Math must be **1:1** with `accel-classic.hpp` (no simplified reinterpretation)
- Linear ≡ Classic exponent **2** where documented; do not brand “Classic² / Linear”
- Primary defaults: Gain **on**, `cap_mode=out`, `cap_y=2`, `input_offset=0`, `acceleration=0.007`, `sensitivity_multiplier=1.0` — **trainer defaults**, not official RA defaults
- Phase 1 Guide path must remain selectable: `gain=false` + inactive out cap (`cap_y <= 0`)
- `dt_ms <= 0` → EXPLICIT trainer identity bypass (no Gain/cap/`m`); not RA-attributed
- Speed: `v = sqrt((dx * dx) + (dy * dy)) / dt_ms`
- Processor id stays `rawaccel_linear`; version **`1.1.0`**; experiment version **`0.4.0`**
- Documented math claim only — not actual-driver parity
- No anisotropy, rotation, EMA, DPI norm, By Component, other styles, LUT framework
- Unit tests green before trusting live feel; stop after 1.1

---

## File Structure

```
crates/sense-accel/src/classic_linear.rs   # NEW: CapMode, ClassicLinearArgs, ClassicLinearState, scale()
crates/sense-accel/src/lib.rs              # RawAccelLinearConfig, wire eval/process/factory
crates/sense-accel/tests/processor_tests.rs
crates/sense-accel/tests/classic_linear_tests.rs  # NEW: Gain/cap suite
src/config.rs
src/session.rs
src/validation_lab.rs
src/camera_ctrl.rs                         # create_processor("none", …) via config helper
docs/M2X_RAWACCEL_LINEAR.md
docs/M2_PROCESSOR.md
docs/ARCHITECTURE.md
README.md
```

---

### Task 1: `classic_linear` module — 1:1 classic scale math + unit tests

**Files:**
- Create: `crates/sense-accel/src/classic_linear.rs`
- Modify: `crates/sense-accel/src/lib.rs` — `mod classic_linear; pub use classic_linear::*;`
- Create: `crates/sense-accel/tests/classic_linear_tests.rs`

**Interfaces:**
- Produces:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapMode { Out, In, Io }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassicLinearArgs {
    pub acceleration: f64,
    pub input_offset: f64,
    pub gain: bool,
    pub cap_mode: CapMode,
    pub cap_x: f64,
    pub cap_y: f64,
}

/// Precomputed state mirroring C++ classic ctor fields.
#[derive(Debug, Clone)]
pub struct ClassicLinearState { /* private fields */ }

impl ClassicLinearState {
    pub fn new(args: ClassicLinearArgs) -> Self;
    /// Official sensitivity scale for input speed x (counts/ms). Does not apply sens multiplier.
    pub fn scale(&self, x: f64) -> f64;
}
```

Port rules (from `accel-classic.hpp`):
- `EXPONENT: f64 = 2.0`
- `base_fn(x) = accel_raised * (x - offset).powf(2.0) / x` when used as in source
- Legacy: `sign * base_fn.min(cap) + 1` after offset check
- Gain: piecewise `base_fn` vs `constant/x + cap.y`, then `sign * output + 1`
- Helpers `gain`, `gain_inverse`, `gain_accel`, `base_accel` — same formulas as source
- Cap init branches for `out` / `in` / `io` — same as source (including negative cap → flip sign)
- Inactive out cap when `cap_y` not `> 0`: Legacy `cap = f64::MAX`; Gain leaves uncapped path per source (`cap.x` stays max / skip knee)

- [ ] **Step 1: Write failing tests** in `crates/sense-accel/tests/classic_linear_tests.rs`

```rust
use sense_accel::{CapMode, ClassicLinearArgs, ClassicLinearState};

fn approx(a: f64, b: f64, eps: f64) {
    assert!((a - b).abs() < eps, "{a} != {b}");
}

#[test]
fn legacy_uncapped_matches_phase1_linear() {
    let s = ClassicLinearState::new(ClassicLinearArgs {
        acceleration: 0.01,
        input_offset: 0.0,
        gain: false,
        cap_mode: CapMode::Out,
        cap_x: 0.0,
        cap_y: 0.0, // inactive
    });
    approx(s.scale(50.0), 1.5, 1e-12);
}

#[test]
fn legacy_output_cap_2_clamps_addend() {
    let s = ClassicLinearState::new(ClassicLinearArgs {
        acceleration: 0.01,
        input_offset: 0.0,
        gain: false,
        cap_mode: CapMode::Out,
        cap_x: 0.0,
        cap_y: 2.0,
    });
    // base_fn(50)=0.5 → min(0.5,1)+1 = 1.5
    approx(s.scale(50.0), 1.5, 1e-12);
    // base_fn(200)=2.0 → min(2,1)+1 = 2.0
    approx(s.scale(200.0), 2.0, 1e-12);
}

#[test]
fn gain_output_cap_2_knee() {
    let accel = 0.007;
    let s = ClassicLinearState::new(ClassicLinearArgs {
        acceleration: accel,
        input_offset: 0.0,
        gain: true,
        cap_mode: CapMode::Out,
        cap_x: 0.0,
        cap_y: 2.0,
    });
    // cap.y addend = 1; cap.x = gain_inverse(1, 0.007, 2, 0) = 0.5/0.007
    let cap_x = 0.5 / accel;
    approx(cap_x, 71.42857142857143, 1e-9);
    // below knee: scale = 1 + accel * x
    approx(s.scale(50.0), 1.0 + accel * 50.0, 1e-12);
    // at/above knee: output = constant/x + 1; scale = output + 1
    // constant = (accel*cap_x - 1.0) * cap_x = (0.5 - 1.0) * cap_x = -0.5 * cap_x
    let constant = (accel * cap_x - 1.0) * cap_x;
    let x = 100.0;
    let expected = constant / x + 1.0 + 1.0; // sign*output + 1 with output=constant/x+cap.y
    approx(s.scale(x), expected, 1e-9);
}

#[test]
fn offset_returns_one_at_or_below_offset() {
    let s = ClassicLinearState::new(ClassicLinearArgs {
        acceleration: 0.01,
        input_offset: 10.0,
        gain: false,
        cap_mode: CapMode::Out,
        cap_x: 0.0,
        cap_y: 0.0,
    });
    approx(s.scale(10.0), 1.0, 1e-12);
    approx(s.scale(5.0), 1.0, 1e-12);
}

#[test]
fn gain_cap_in_and_io_construct_and_evaluate() {
    let in_state = ClassicLinearState::new(ClassicLinearArgs {
        acceleration: 0.01,
        input_offset: 0.0,
        gain: true,
        cap_mode: CapMode::In,
        cap_x: 40.0,
        cap_y: 0.0,
    });
    assert!(in_state.scale(20.0).is_finite());
    assert!(in_state.scale(80.0).is_finite());

    let io_state = ClassicLinearState::new(ClassicLinearArgs {
        acceleration: 0.01, // overwritten by io init from cap point
        input_offset: 0.0,
        gain: true,
        cap_mode: CapMode::Io,
        cap_x: 40.0,
        cap_y: 2.0,
    });
    assert!(io_state.scale(20.0).is_finite());
    assert!(io_state.scale(80.0).is_finite());
}
```

Also add one Legacy `in` / `io` smoke construct+finite test (same idea, `gain: false`).

- [ ] **Step 2: Run — expect FAIL**

```bash
cargo test -p sense-accel --test classic_linear_tests
```

- [ ] **Step 3: Implement `classic_linear.rs`** by translating C++ classic LEGACY and GAIN templates with exponent fixed at 2. Prefer `f64::MAX` where C++ uses `DBL_MAX`. Use `minsd`-equivalent: `a.min(b)`. Document source file in module docs.

- [ ] **Step 4: `cargo test -p sense-accel --test classic_linear_tests` PASS**

- [ ] **Step 5: Commit**

```bash
git add crates/sense-accel/src/classic_linear.rs crates/sense-accel/src/lib.rs crates/sense-accel/tests/classic_linear_tests.rs
git commit -m "feat(sense-accel): 1:1 classic Linear Gain/Legacy scale math"
```

---

### Task 2: `RawAccelLinearConfig` + eval/process + factory

**Files:**
- Modify: `crates/sense-accel/src/lib.rs`
- Modify: `crates/sense-accel/tests/processor_tests.rs`
- Modify: `src/session.rs`, `src/validation_lab.rs`, `src/camera_ctrl.rs` (compile fixes — temporary `RawAccelLinearConfig::phase1(...)` or defaults until Task 3)

**Interfaces:**
- Produces:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct RawAccelLinearConfig {
    pub acceleration: f64,
    pub sensitivity_multiplier: f64,
    pub gain: bool,
    pub input_offset: f64,
    pub cap_mode: CapMode,
    pub cap_x: f64,
    pub cap_y: f64,
}

impl RawAccelLinearConfig {
    /// Phase 1 Guide / verification: Sensitivity, inactive out cap.
    pub fn phase1_sensitivity(acceleration: f64, sensitivity_multiplier: f64) -> Self { … }
    /// Trainer play-oriented defaults (Gain + Output 2, a=0.007, m=1).
    pub fn trainer_default() -> Self { … }
}

pub struct RawAccelLinear {
    config: RawAccelLinearConfig,
    classic: ClassicLinearState,
}

impl RawAccelLinear {
    pub fn new(config: RawAccelLinearConfig) -> Self;
}

pub fn eval_rawaccel_linear_config(dx, dy, dt_s, config: &RawAccelLinearConfig) -> LinearEval;
// Keep thin wrapper or replace old eval_rawaccel_linear(a,m) to call phase1_sensitivity(a,m)
// so existing Guide tests keep compiling.

pub fn create_processor(id: &str, config: &RawAccelLinearConfig) -> Result<Box<dyn InputProcessor>, String>;
// "none" ignores config; "rawaccel_linear" uses RawAccelLinear::new(config.clone())
```

`RawAccelLinear` version string: `"1.1.0"`.  
`config_json`: stable JSON including all config fields (`cap_mode` as `"out"|"in"|"io"`).

Eval path:
1. `dt_ms` bypass unchanged (identity, no multiplier)
2. else `v = speed`, `acceleration_scale = classic.scale(v)`, apply_whole × multiplier

- [ ] **Step 1: Update/add tests**

Keep Guide test via `phase1_sensitivity(0.01, 0.5)`.  
Add:

```rust
#[test]
fn factory_rawaccel_linear_v110_gain_default_process_finite() {
    let mut p = create_processor("rawaccel_linear", &RawAccelLinearConfig::trainer_default()).unwrap();
    assert_eq!(p.version(), "1.1.0");
    let (ox, oy) = p.process(30.0, 40.0, 0.001);
    assert!(ox.is_finite() && oy.is_finite());
}
```

Update every `create_processor(id, a, m)` call site to `create_processor(id, &config)`.

- [ ] **Step 2: FAIL then implement**

- [ ] **Step 3: `cargo test -p sense-accel` and `cargo test --workspace` PASS**

- [ ] **Step 4: Commit**

```bash
git add crates/sense-accel src
git commit -m "feat(sense-accel): RawAccelLinearConfig and v1.1.0 factory"
```

---

### Task 3: Experiment settings + session + HUD

**Files:**
- Modify: `src/config.rs`
- Modify: `src/session.rs`
- Modify: `src/validation_lab.rs`
- Modify: `src/camera_ctrl.rs` if needed

**Interfaces:**
Extend `ExperimentSettings`:

```rust
pub struct ExperimentSettings {
    pub dpi: f64,
    pub sensitivity: f64,
    pub fov_degrees_h: f64,
    pub processor_id: String,
    pub acceleration: f64,
    pub sensitivity_multiplier: f64,
    pub gain: bool,
    pub input_offset: f64,
    pub cap_mode: CapMode, // re-export from sense_accel or mirror enum in config
    pub cap_x: f64,
    pub cap_y: f64,
}
```

Defaults = `RawAccelLinearConfig::trainer_default()` field values; update unit tests (old assert `acceleration == 0.01` → `0.007`, plus gain/cap asserts).

Helper on settings:

```rust
fn rawaccel_linear_config(&self) -> RawAccelLinearConfig { … }
```

`start_validation` / HUD version probe:

```rust
create_processor(&settings.processor_id, &settings.rawaccel_linear_config())
```

`EXPERIMENT_VERSION = "0.4.0"`.

**HUD (Idle only):**
- Gain checkbox
- Cap mode combo: out / in / io
- Cap X, Cap Y DragValues (always visible; tooltips note which modes use which)
- Input offset
- Acceleration, Sensitivity multiplier (existing)
- Note: Gain vs Legacy/Sensitivity; caps 1:1 with official classic; trainer defaults

- [ ] **Step 1: Settings + tests**
- [ ] **Step 2: Session + HUD wiring**
- [ ] **Step 3: `cargo test --workspace` PASS**
- [ ] **Step 4: Commit**

```bash
git add src/config.rs src/session.rs src/validation_lab.rs src/camera_ctrl.rs
git commit -m "feat(app): wire Linear Gain/cap settings and HUD"
```

---

### Task 4: Docs + stop gate

**Files:**
- Modify: `docs/M2X_RAWACCEL_LINEAR.md` (Gain/caps, version 1.1.0 / experiment 0.4.0, EXPLICIT dt rule, trainer defaults, provenance)
- Modify: `docs/M2_PROCESSOR.md`, `docs/ARCHITECTURE.md`, `README.md` as needed
- Optional: short note linking Phase 1.1 spec

- [ ] **Step 1: Update docs** — claim 1:1 classic math; not driver parity; smoke operator-pending
- [ ] **Step 2: Commit**

```bash
git add docs README.md
git commit -m "docs: Phase 1.1 Linear Gain and caps provenance"
```

- [ ] **Step 3: STOP** — no Natural/anisotropy/driver comparison without new spec

---

## Spec Coverage Checklist

| Spec requirement | Task |
|------------------|------|
| 1:1 classic LEGACY + GAIN | 1 |
| Cap out / in / io | 1 |
| `dt_ms<=0` EXPLICIT bypass | 2 (preserved) |
| Phase 1 Guide regression | 2 |
| Config + version 1.1.0 | 2 |
| Trainer defaults Gain/Output2/0.007 | 2–3 |
| HUD Gain/caps/offset | 3 |
| Experiment 0.4.0 | 3 |
| Docs / stop | 4 |

## Placeholder / consistency self-check

- Factory uses `&RawAccelLinearConfig` everywhere after Task 2
- Gain Output-cap expected values derived from `gain_inverse` / `constant` as in tests above
- No DB schema changes required

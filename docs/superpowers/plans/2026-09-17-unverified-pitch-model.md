# M3 UnverifiedPitchModel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable hipfire pitch look using locked research assumptions (same 0.07 as yaw, +dy look down, ±89° clamp) behind UnverifiedPitchModel labels — no pitch validation experiment.

**Architecture:** Add pitch helpers in `sense-math`; wire `processed_dy` into `apply_sample` and a yaw×pitch camera transform; update HUD/docs and bump experiment version to `0.5.0`.

**Tech Stack:** Existing workspace; `sense-math`; Bevy 0.19 camera transform.

**Spec:** `docs/superpowers/specs/2026-09-17-unverified-pitch-model-design.md`  
**Research:** `docs/superpowers/specs/2026-09-17-valorant-pitch-research-findings.md`

## Global Constraints

- `pitch_delta_deg = -(processed_dy × sensitivity × 0.07)` when `+pitch_deg` = look up (positive ΔY → look down)
- Clamp pitch to `[-89.0, 89.0]`
- Same numeric constant `0.07` as yaw; provenance **UNCERTAIN** / UnverifiedPitchModel
- Use **processed** `dy` from InputProcessor
- FOV/resolution must not change deg/count
- No Invert-Y toggle, no pitch validation experiment, no ADS, no yaw-constant change, no silent compensation
- Validation Lab 360° remains yaw-centric
- Experiment version **`0.5.0`**
- Stop after this cycle

---

## File Structure

```
crates/sense-math/src/lib.rs
crates/sense-math/tests/math_tests.rs
src/camera_ctrl.rs
src/validation_lab.rs
src/session.rs                    # EXPERIMENT_VERSION 0.5.0
docs/VALORANT_INPUT_MODEL.md
docs/BASELINE.md
docs/ARCHITECTURE.md
README.md
```

---

### Task 1: `sense-math` pitch helpers + unit tests

**Files:**
- Modify: `crates/sense-math/src/lib.rs`
- Modify: `crates/sense-math/tests/math_tests.rs`

**Interfaces:**
- Produces:

```rust
/// UnverifiedPitchModel: hipfire pitch uses same 0.07 as yaw (UNCERTAIN).
/// Positive dy → look down when +pitch_deg means look up.
pub fn pitch_delta_deg(processed_dy: f64, sensitivity: f64) -> f64;

pub const PITCH_LIMIT_DEG: f64 = 89.0;

pub fn clamp_pitch_deg(pitch_deg: f64) -> f64;

pub fn apply_pitch_delta(pitch_deg: f64, processed_dy: f64, sensitivity: f64) -> f64 {
    clamp_pitch_deg(pitch_deg + pitch_delta_deg(processed_dy, sensitivity))
}
```

`pitch_delta_deg` must equal `-(processed_dy * sensitivity * VALORANT_YAW_DEG_PER_COUNT_AT_SENS_1)` (reuse existing 0.07 constant).

- [ ] **Step 1: Write failing tests** in `math_tests.rs`

```rust
#[test]
fn pitch_positive_dy_looks_down() {
    // +pitch = look up ⇒ +dy yields negative delta
    let d = pitch_delta_deg(1000.0, 0.175);
    assert!((d - (-12.25)).abs() < 1e-12);
}

#[test]
fn pitch_and_yaw_same_magnitude() {
    let sens = 0.09;
    let yaw = yaw_delta_deg(500.0, sens).abs();
    let pitch = pitch_delta_deg(500.0, sens).abs();
    assert!((yaw - pitch).abs() < 1e-12);
}

#[test]
fn pitch_clamps_at_pm_89() {
    assert_eq!(clamp_pitch_deg(90.0), 89.0);
    assert_eq!(clamp_pitch_deg(-90.0), -89.0);
    let p = apply_pitch_delta(88.0, 1_000_000.0, 1.0);
    assert_eq!(p, -89.0); // large +dy drives look-down to floor
}
```

Adjust the third test if `apply_pitch_delta(88, huge +, 1)` overshoots: expect exactly `-89.0` after clamp.

- [ ] **Step 2: `cargo test -p sense-math` — expect FAIL**

- [ ] **Step 3: Implement helpers + UNCERTAIN/UnverifiedPitchModel docs on the functions**

- [ ] **Step 4: `cargo test -p sense-math` PASS**

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(sense-math): UnverifiedPitchModel pitch delta and clamp"
```

---

### Task 2: Camera apply + transform + reset

**Files:**
- Modify: `src/camera_ctrl.rs`
- Modify tests in `src/camera_ctrl.rs`

**Interfaces:**
- `apply_sample` takes `processed_dy` (already has processed_dx path — pass both from drain)
- After yaw: `pose.pitch_deg = sense_math::apply_pitch_delta(pose.pitch_deg, processed_dy, sensitivity)`
- Replace `apply_yaw_transform` body:

```rust
let yaw = Quat::from_rotation_y(-(pose.yaw_deg.to_radians() as f32));
let pitch = Quat::from_rotation_x(pose.pitch_deg.to_radians() as f32); // +pitch look up
transform.rotation = yaw * pitch;
```

(If look is inverted vs expectation in manual check, fix only after tests; do not flip research sign without documenting.)

- `reset_camera`: `pose.pitch_deg = 0.0` as well as yaw
- Update test `yaw_updates_and_pitch_remains_frozen` → pitch **updates** with dy; rename accordingly

- [ ] **Step 1: Update/add camera unit tests** (pitch changes with processed dy; reset clears pitch)

- [ ] **Step 2: Implement apply_sample + transform + reset**

- [ ] **Step 3: Ensure drain passes `processed_dy` into `apply_sample`**

- [ ] **Step 4: `cargo test -p sense-maxer` PASS**

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(app): apply UnverifiedPitchModel to camera look"
```

---

### Task 3: HUD + experiment version + docs

**Files:**
- Modify: `src/validation_lab.rs`
- Modify: `src/session.rs` (`EXPERIMENT_VERSION = "0.5.0"` + test if present)
- Modify: `docs/VALORANT_INPUT_MODEL.md`, `docs/BASELINE.md`, `docs/ARCHITECTURE.md`, `README.md`

**HUD:**
- `PITCH: …` (not FROZEN)
- Small note: UnverifiedPitchModel; same 0.07; +dy look down; ±89°; UNCERTAIN

**Docs:** pitch enabled per spec; link research + design specs; BASELINE pitch row updated.

- [ ] **Step 1: Wire HUD + version**
- [ ] **Step 2: Docs**
- [ ] **Step 3: `cargo test --workspace` PASS**
- [ ] **Step 4: Commit**

```bash
git commit -m "docs: enable UnverifiedPitchModel baseline and HUD"
```

- [ ] **Step 5: STOP**

---

## Spec Coverage

| Spec item | Task |
|-----------|------|
| pitch_delta + clamp math | 1 |
| Camera apply + quat | 2 |
| Reset both axes | 2 |
| HUD + 0.5.0 + docs | 3 |
| No pitch validation / invert / ADS | all |

## Placeholder self-check

- Sign convention fixed: `pitch_delta = -(dy × sens × 0.07)`
- Constant reused from existing yaw constant symbol

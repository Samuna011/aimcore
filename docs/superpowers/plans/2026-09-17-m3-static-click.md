# M3 STATIC_CLICK Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship STATIC_CLICK: fixed sphere target, raw LMB-down, ray–sphere hit/miss, look reset on Start, HUD result — no aim DB yet.

**Architecture:** Pure `ray_sphere_hit` in `sense-math` with unit tests; app `aim_trial` module owns state machine, target spawn/visibility, click edge from `MouseSample.buttons`, and HUD wiring. Separate from 360° validation; blocked while validation Running.

**Tech Stack:** Bevy 0.19, existing `WM_INPUT` queue / `YawPitch`, egui HUD, `sense-math`.

**Spec:** `docs/superpowers/specs/2026-09-17-m3-static-click-design.md`

## Global Constraints

- Hit = analytic ray–sphere only (no screen pick)
- Click = `RI_MOUSE_LEFT_BUTTON_DOWN` (0x0001) on raw sample; look mode + Armed only
- Fixed target; one-shot trial; Start resets yaw/pitch to 0
- Camera at `(0, 1.6, 4)` looks −Z at identity pose → target center `(0, 1.6, -6)` (distance 10), `R = 0.25`
- No SQLite aim table in v1; `EXPERIMENT_VERSION` → `0.6.0`
- Block Start Aim while `ValidationState::Running`

---

### Task 1: Ray–sphere math + tests

**Files:**
- Modify: `crates/sense-math/src/lib.rs`
- Modify: `crates/sense-math/tests/math_tests.rs`

**Interfaces:**
- `pub fn ray_sphere_hit(origin: [f64; 3], dir: [f64; 3], center: [f64; 3], radius: f64) -> bool`
- `dir` need not be pre-normalized; function normalizes (or returns false if dir ~ 0)
- Hit iff closest non-negative intersection exists

- [ ] **Step 1: Failing tests** — miss beside sphere; hit through center; ray opposite direction (behind) = miss; zero dir = miss

- [ ] **Step 2: Implement `ray_sphere_hit`**

- [ ] **Step 3: `cargo test -p sense-math` PASS**

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(sense-math): ray-sphere hit test for STATIC_CLICK"
```

---

### Task 2: Aim trial state + click + target

**Files:**
- Create: `src/aim_trial.rs`
- Modify: `src/camera_ctrl.rs` (export reset helper if needed; or call pose clear from aim)
- Modify: `src/app.rs` (register systems/resources)
- Modify: `src/lib.rs` or `main` module tree (`mod aim_trial`)
- Modify: `src/scene.rs` optional — remove conflicting red cuboid or leave; aim target is separate entity with `AimTarget` component

**Interfaces:**
- `AimTrialState`: Idle | Armed
- `AimTrialResult { hit, yaw_deg, pitch_deg, timestamp_ns }`
- `pub const RI_MOUSE_LEFT_BUTTON_DOWN: u32 = 0x0001;`
- System: after/with drain — inspect drained samples’ buttons while Armed + look enabled; first left-down fires test using camera `Transform` translation + forward (−Z in local), then set result and Idle
- Start: reset yaw/pitch; spawn/show sphere at `(0.0, 1.6, -6.0)` radius 0.25; Armed
- Visibility: hide mesh when Idle

- [ ] **Step 1: Resources + Start/Cancel API used by HUD**

- [ ] **Step 2: Spawn AimTarget sphere; sync visibility**

- [ ] **Step 3: Process raw left-down → ray_sphere_hit → result**

- [ ] **Step 4: Unit test helpers if any (button flag); `cargo test -p sense-maxer`**

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(app): STATIC_CLICK aim trial state and ray hit"
```

---

### Task 3: HUD + crosshair + version/docs

**Files:**
- Modify: `src/validation_lab.rs`
- Modify: `src/session.rs` (`EXPERIMENT_VERSION = "0.6.0"`)
- Modify: `docs/BASELINE.md`, `docs/EXPERIMENT_MODEL.md`, `docs/ARCHITECTURE.md`, `README.md` (brief)

**HUD:**
- Start Aim Trial / Cancel Aim
- `AIM: Idle|Armed`, `LAST AIM: HIT|MISS` (+ pose optional)
- Crosshair center while look mode (egui painter or label at center)
- Block Start Aim when validation Running
- Note: STATIC_CLICK; ray–sphere; raw LMB; fixed target

- [ ] **Step 1: Wire HUD + crosshair**

- [ ] **Step 2: Docs + experiment 0.6.0**

- [ ] **Step 3: `cargo test --workspace` PASS**

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(app): STATIC_CLICK HUD and experiment 0.6.0"
```

- [ ] **Step 5: STOP** — manual: Start Aim → look at sphere → LMB HIT; look away MISS

---

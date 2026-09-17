# M3 — STATIC_CLICK Controlled Aim Task — Design Spec

**Date:** 2026-09-17  
**Status:** Approved  
**Depends on:** Baseline exp `0.5.2+` (UnverifiedPitchModel, uncapped present, `rawaccel_linear` optional)  
**Goal:** First controlled aim task: one fixed target, one raw click, ray–sphere hit/miss — prove the loop before random/flick/tracking.

---

## 1. Locked decisions (brainstorm)

| Topic | Choice |
|-------|--------|
| Task type | **STATIC_CLICK** |
| Hit test | **Ray–sphere** (camera origin along look ∩ target sphere) |
| Click source | **`WM_INPUT` primary button-down** (same stream as look) |
| Target placement | **Fixed** world position |
| Trial outcome | **One-shot:** click → hit or miss → trial ends |
| On Start | **Reset yaw and pitch to 0** |
| Architecture | **Approach 1:** app module + pure ray–sphere helper (no full `sense-aim` crate / DB in v1) |

---

## 2. Purpose

Deliver a minimal, reproducible aim trial so later M3/M4 work can compare processors (`none` vs `rawaccel_linear`) under a real click task—not only free look.

Success for v1 = correct **Armed → click → HIT/MISS → Idle** loop with HUD feedback and unit-tested hit math. Feel polish and rich telemetry are secondary.

---

## 3. Coordinate / camera model

- Camera pose: existing `YawPitch` (`yaw_deg`, `pitch_deg`) with transform `yaw * pitch` as today.
- **Look ray origin:** camera world position (scene spawn; typically near origin facing −Z or +Z as already set up — **match existing scene forward**).
- **Look ray direction:** camera forward after yaw/pitch (same as crosshair center).
- **Crosshair:** screen-center marker (egui or simple overlay); purely visual — hit test uses camera forward, not cursor pixels.

**CONFIRMED** with current look path: no screen-space picking in v1.

---

## 4. Target

| Parameter | Trainer default | Notes |
|-----------|-----------------|-------|
| Position | Fixed, on look-reset forward axis at distance `D` | e.g. if camera looks down −Z at identity, target at `(0, 0, −D)` — **implementer must match `setup_scene` forward** |
| `D` | `10.0` world units | Idle-editable optional in v1; constant OK if HUD shows it |
| Radius `R` | `0.25` world units | Visible sphere mesh; hit uses same `R` |
| Appearance | Simple colored sphere (Bevy mesh) | Shown only while trial `Armed` (or through Result brief flash) |

No random placement, grid, or movement in v1.

---

## 5. Hit / miss mathematics

Pure function (prefer `sense-math` or a tiny app-local module with unit tests):

```text
ray_sphere_intersect(origin, dir_unit, center, radius) -> Option<t>
hit = intersect is Some and t >= 0
```

- `dir` must be normalized.
- On primary **button-down** while `Armed` and look mode enabled: evaluate once with current camera pose.
- **HIT** if intersection exists with `t ≥ 0`.
- **MISS** otherwise (including looking away).
- No partial credit / distance grades in v1 (optional later: closest approach distance logged only).

**Performance:** one analytic test per click — negligible.

---

## 6. Click input

- Samples already carry `buttons: u32` from raw `ulButtons`.
- Detect **left button down** edge: raw flag `RI_MOUSE_LEFT_BUTTON_DOWN` (`0x0001`) on the sample (not Bevy/`egui`).
- Process only when:
  - Aim trial state is `Armed`
  - `LookCapture.enabled` (look mode)
- Ignore aim clicks in UI mode (cursor free).
- Do **not** use egui clicks for scoring.
- Button-down only (ignore up); one shot ends the trial so repeats in the same packet burst after transition are ignored.

---

## 7. Trial state machine

```text
Idle ──Start Aim Trial──► Armed ──primary down──► Result ──(immediate or Next)──► Idle
                              │
                              └── Cancel / ESC UI? stay rules: Cancel button → Idle, despawn target
```

| State | Behavior |
|-------|----------|
| `Idle` | No aim target (or hidden); 360° validation HUD unchanged |
| `Armed` | Target visible; look reset already applied at Start; waiting for one primary down |
| `Result` | Show last HIT/MISS on HUD; target may hide; waiting for user to Start again (v1: return to Idle right after recording is OK if HUD keeps `last_result`) |

**Start Aim Trial** (Idle only):

1. Reset `yaw_deg = 0`, `pitch_deg = 0` (same helper spirit as camera reset).
2. Ensure target at fixed pose; show mesh.
3. Enter `Armed`.
4. Does **not** require Start Validation / DB session in v1.

**Separation from 360° validation:** Aim trial is a **separate** control path. Running Validation may either (a) block Start Aim Trial, or (b) allow both — **v1: block aim trial while Validation `Running`** to avoid mixed semantics.

---

## 8. HUD

- Button: **Start Aim Trial** (Idle, not Validating).
- Optional: **Cancel Aim** while Armed.
- Status: `AIM: Idle | Armed | …`
- After click: `LAST AIM: HIT` or `MISS`
- Show `D`, `R`, and optionally look yaw/pitch at click.
- Small note: STATIC_CLICK; ray–sphere; raw LMB down; fixed target.

Crosshair: visible at least while Armed (and ideally always in look mode).

---

## 9. Telemetry (v1)

**In-memory / HUD only** for first slice:

| Field | Purpose |
|-------|---------|
| `hit: bool` | Outcome |
| `timestamp_ns` | Click sample time |
| `yaw_deg`, `pitch_deg` | Pose at click |
| `processor_id` | Active processor |
| `target_center`, `radius` | Config snapshot |

SQLite aim-trial table: **out of scope for v1** (follow-up once loop is proven). Existing raw/processed mouse tables unchanged; aim click need not write DB rows yet.

---

## 10. Experiment version

- Bump `EXPERIMENT_VERSION` to `0.6.0` when aim trial ships (M3 STATIC_CLICK).
- Document in `BASELINE.md` / `EXPERIMENT_MODEL.md`: STATIC_CLICK available; still no flick/tracking.

---

## 11. Out of scope (v1)

- Random / grid / moving targets  
- Flick, tracking, target switching  
- Multi-shot / free-fire modes  
- Screen-space hit tests  
- Bevy/`egui` click scoring  
- Aim trial SQLite schema  
- Sound/VFX beyond basic mesh + HUD  
- Claiming Valorant hitreg parity  

---

## 12. Acceptance

- Unit tests: ray misses sphere; ray grazes/hits; `t < 0` behind camera = miss.  
- Manual: Start Aim Trial → look at sphere → LMB → HIT; look away → MISS; look reset on Start.  
- Validation Lab 360° path still works; aim blocked while validation Running.  
- Workspace tests green.  

---

## 13. Open implementer notes (non-blocking)

- Confirm scene forward axis when placing `(0,0,±D)`.  
- Decode left-down from `buttons` consistently with Windows raw-input docs.  
- If multiple samples in one drain contain left-down, only the first while `Armed` consumes the trial.

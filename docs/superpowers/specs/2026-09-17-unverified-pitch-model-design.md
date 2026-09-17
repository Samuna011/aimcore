# M3 — UnverifiedPitchModel (Hipfire Pitch Enable) — Design Spec

**Date:** 2026-09-17  
**Status:** Draft for user review  
**Depends on:** `docs/superpowers/specs/2026-09-17-valorant-pitch-research-findings.md`; M2/M2.x processor path  
**Scope:** Enable hipfire pitch look using approved research assumptions; Approach 1 (minimal enable). **No** pitch validation experiment, invert toggle, or ADS.

---

## 1. Purpose

Enable vertical look in the Validation Lab camera so hipfire pitch matches the locked research findings as closely as evidence allows, behind an explicitly **unverified** model.

Does **not** claim Riot-primary CONFIRMED pitch math. Does **not** change yaw `0.07` provenance or Validation Lab 360° (yaw-centric) semantics.

---

## 2. Locked assumptions (from research)

| Item | Value | Confidence |
|------|--------|------------|
| Pitch scalar | Same `0.07` deg/count @ sens 1 as yaw | **UNCERTAIN** (approved assumption) |
| Formula | `pitch` uses `processed_dy × sensitivity × 0.07` with sign below | **UNCERTAIN** |
| Direction | Positive mouse `ΔY` → look **down** | **UNCERTAIN** (approved) |
| Clamp | `[-89°, +89°]` | **UNCERTAIN** (approved) |
| FOV/res | Do not change deg/count | **DERIVED** / approved |
| Input | **Processed** `dy` (InputProcessor) | **DERIVED** (M2) |

Convention in this codebase: **`+pitch_deg` means look up**. Therefore:

```
pitch_delta_deg = -(processed_dy × sensitivity × 0.07)
pitch_deg = clamp(pitch_deg + pitch_delta_deg, -89.0, 89.0)
```

Yaw unchanged:

```
yaw_delta_deg = processed_dx × sensitivity × 0.07
```

---

## 3. Architecture

### 3.1 `sense-math`

Add pitch helpers (names illustrative):

- `pitch_delta_deg(processed_dy, sensitivity) -> f64` — applies the negative sign for look-down
- `clamp_pitch_deg(pitch_deg) -> f64` — clamp to ±89
- Document module/constants as **UnverifiedPitchModel** / UNCERTAIN
- Keep existing yaw helpers; shared numeric constant remains `0.07` (optionally rename comment to “hipfire deg/count @ sens 1” without claiming pitch is CONFIRMED)

### 3.2 App camera (`src/camera_ctrl.rs`)

- In `apply_sample`: after yaw update, apply pitch from `processed_dy` via sense-math; store clamped `pose.pitch_deg`
- Replace yaw-only `apply_yaw_transform` with look transform:

```
rotation = yaw_quat * pitch_quat
```

  preserving existing yaw sign (`from_rotation_y(-yaw)`); pitch about local X so +pitch looks up; no roll
- `reset_camera`: zero **both** yaw and pitch
- Existing `InputCameraSample.pitch_deg` will now vary (already in schema)

### 3.3 HUD / session

- Show live `PITCH` (remove “FROZEN”)
- Short label: UnverifiedPitchModel — same 0.07 as yaw; +dy look down; ±89°; UNCERTAIN
- `EXPERIMENT_VERSION` → `0.5.0`

### 3.4 Validation

- 360° Validation Lab remains **yaw-only** for expected counts / yaw error
- Under acceleration, yaw degrees path unchanged
- Pitch does not add a new validation experiment in this cycle

---

## 4. Testing

| Test | Expect |
|------|--------|
| Positive `processed_dy` | Pitch decreases (look down) |
| Negative `processed_dy` | Pitch increases |
| Large upward/downward motion | Clamped to ±89 |
| Equal \|dx\| and \|dy\| at same sens | Equal \|yaw_delta\| and \|pitch_delta\| magnitudes |
| Existing yaw / processor / session tests | Stay green |
| Transform smoke (optional) | Pure dy changes pitch component; no unintended roll |

---

## 5. Documentation

Update:

- `docs/VALORANT_INPUT_MODEL.md` — pitch section with formulas + confidence
- `docs/BASELINE.md` — pitch enabled (unverified)
- `docs/ARCHITECTURE.md` — remove “pitch disabled”; cite UnverifiedPitchModel
- `README.md` — look mode includes pitch
- Link research findings spec

---

## 6. Explicitly out of scope

- Pitch validation experiment / elevation 360-style test  
- Invert Y toggle  
- ADS / scoped multipliers  
- Changing yaw constant or silent compensation  
- Aim tasks  
- Claiming CONFIRMED Riot pitch  

---

## 7. Success criteria

- Look mode: mouse Y pitches camera with locked sign and clamp  
- Telemetry `pitch_deg` updates per sample  
- HUD + docs labeled UNCERTAIN  
- Unit tests cover sign, clamp, shared magnitude  
- Experiment version `0.5.0`  
- Stop after this cycle  

---

## 8. Stop

After implementation + tests + docs, **STOP**. Further pitch empiricism or ADS = new design cycle.

---

## Revision Notes

- **2026-09-17:** Approach 1 minimal enable. Assumptions from research findings + operator reliable detail (same 0.07, +dy look down, ±89, FOV-independent).

# M3 Research — VALORANT Pitch / Vertical Look — Findings Spec

**Date:** 2026-09-17  
**Status:** Research complete — candidate model approved for a **future** implementation cycle; **no code in this cycle**  
**Depends on:** `docs/VALORANT_INPUT_MODEL.md`, `docs/BASELINE.md`, M1–M2.x STOP gates  
**Goal:** Establish the most accurate pitch model evidence allows, with explicit confidence labels, before enabling camera pitch.

---

## 1. Purpose

Decide how vertical mouse look should work in sense-maxer if we want **maximum fidelity to VALORANT**, not convenience.

This document records research findings and a **candidate** model. It does **not** authorize implementation by itself; a separate implementation design + plan is required after this review.

---

## 2. Locked decisions from brainstorming

| Decision | Choice |
|----------|--------|
| Accuracy priority | Match VALORANT vertical as closely as evidence allows |
| Cycle type | **Research-only** (findings + protocol; no code) |
| Research approach | Community/industry-aligned hypothesis + measurement gate |
| Candidate assumption | Hipfire **pitch uses the same `0.07` deg/count @ sens 1 as yaw** |

---

## 3. Evidence surveyed

### 3.1 Our project (yaw)

- Formula: `yaw_delta_deg = dx × sensitivity × 0.07`
- Confidence vs Riot primary docs: **UNCERTAIN** (community constant)
- Lab status: **EMPIRICALLY TESTED** in this trainer (e.g. ~0.05% error on physical 360° under accel path)

### 3.2 Community converters / cm/360 ecosystem

- Treat Valorant as a **single** hipfire sensitivity and yaw constant `0.07`
- Do not expose a separate Valorant pitch sensitivity for hipfire
- Supports: one scale for horizontal and vertical hipfire

### 3.3 Aimlabs (official + staff)

Sources:

- [How to Configure and Convert Your Sensitivity in Aimlabs](https://aimlabs.com/articles/aimlabs/how-to-configure-and-convert-your-sensitivity-in-aimlabs/)
- [Did You Know Aimlabs Has a Sensitivity Converter Built In?](https://aimlabs.com/articles/aimlabs/did-you-know-aimlabs-has-a-sensitivity-converter-built-in/)
- Staff reply ([r/aimlab](https://www.reddit.com/r/aimlab/comments/1k874lw/valorant_to_aimlab_sens_difference/)): Valorant Game Profile → FoV 103, sensitivity scaling **1:1 with Valorant**

What Aimlabs **does** claim:

- Game Profile selects the game’s sensitivity **scale/formula**
- Enter the **same** numeric sens as in Valorant
- Conversion preserves physical turn distance (**cm/360** / 1:1 across profiles)
- Valorant FOV matched (103 HFOV)

What Aimlabs **does not** publish (in these sources):

- An explicit statement `pitch_deg_per_count = 0.07`
- Pitch clamp limits for Valorant
- Mouse Y sign convention (up vs down)

**Interpretation:** Aimlabs models Valorant hipfire as **one sensitivity number** shared with the game UI — consistent with **same X/Y angular scale**, but not a primary proof of vertical math.

### 3.4 Pitch clamp / sign

- ±89° is a common Unreal/FPS look clamp; **not** confirmed from Riot documentation for Valorant in this research pass
- Cheat/offset discussions are **not** used as CONFIRMED sources for this project

### 3.5 Explicitly deferred

- ADS / scoped / zoom multipliers (separate from hipfire)
- Monitor-distance / FOV-based “feel” matching (distinct from deg/count)
- Actual Riot engine source or official API constants

---

## 4. Provenance table (locked)

| Claim | Sources | Confidence |
|-------|---------|------------|
| Hipfire yaw: `dx × sens × 0.07` | Community converters; our Validation Lab | **UNCERTAIN** (vs Riot) / **EMPIRICALLY TESTED** (trainer) |
| Valorant hipfire uses **one** sens (no separate pitch slider) | Valorant UI; Aimlabs Valorant profile; converters | **DERIVED** / industry practice |
| Hipfire pitch: `dy × sens × 0.07` (same constant as yaw) | Single-sens UI; Aimlabs 1:1 profile; converters; **operator-supplied reliable findings** | **UNCERTAIN** vs Riot primary — **approved implementation assumption** |
| HFOV 103° | Valorant fixed FOV; Aimlabs profile | **CONFIRMED** as established project baseline |
| Pitch clamp ≈ ±89° | UE4 gimbal-lock avoidance (operator findings; common UE pattern) | **UNCERTAIN** — **approved implementation assumption** |
| Mouse Y sign: **positive ΔY → look down** | Operator-supplied reliable findings; common FPS/UE | **UNCERTAIN** — **approved implementation assumption** |
| FOV/resolution do not change hipfire deg/count | Operator findings + existing yaw policy | **DERIVED** / approved assumption |

---

## 5. Candidate model (for next implementation cycle)

**Name:** `UnverifiedPitchModel` (or equivalent)

```
pitch_delta_deg = -(processed_dy × sensitivity × 0.07)
// positive mouse ΔY (forward) → look down ⇒ subtract from pitch if +pitch is look-up
// Equivalently: if engine pitch+ is look-up, apply negative of (dy × sens × 0.07)

pitch_deg = clamp(pitch_deg + pitch_delta_deg, -89.0, +89.0)
```

| Parameter | Locked default | Confidence |
|-----------|----------------|------------|
| Constant | `0.07` (shared with yaw) | **UNCERTAIN** (approved assumption) |
| `PITCH_MIN` / `PITCH_MAX` | `-89.0` / `+89.0` | **UNCERTAIN** (approved assumption) |
| Sign | Positive `processed_dy` → **look down** | **UNCERTAIN** (approved assumption) |
| Input | **Processed** `dy` | **DERIVED** from M2 |

**Rules:**

- Never present as CONFIRMED VALORANT pitch
- No silent compensation if vertical validation disagrees later
- Yaw 360° Validation Lab remains primary; pitch enablement does not replace it
- ADS/zoom out of scope until a later cycle

---

## 6. Measurement protocol (raise confidence later)

Optional operator / future experiment — not part of this research commit:

1. Fixed DPI + Valorant sens; Raw Accel off or known profile documented.
2. In Valorant: move mouse purely vertical between two known elevations (map marks / recorded VOD), or scripted relative moves if available legally/safely.
3. Log net `dy` counts (external or our lab mirrored settings).
4. Compare observed elevation Δ° to `net_dy × sens × 0.07`.
5. Promote claim only if error is within an agreed tolerance → **EMPIRICALLY TESTED**.

Aimlabs cross-check: same sens/DPI/FOV 103 on Valorant profile; compare vertical travel feel / measured Δ° if tooling allows — secondary, not primary.

---

## 7. Explicitly out of scope (this research cycle)

- Writing pitch camera code
- Changing yaw constant
- Claiming Riot-official pitch math
- ADS/scoped models
- Aim tasks (STATIC_CLICK, etc.)

---

## 8. Success criteria (research)

- [x] Evidence surveyed (converters, Aimlabs official/staff, project yaw baseline)
- [x] Confidence labels assigned without overclaiming
- [x] Candidate model written with shared `0.07` assumption
- [x] Measurement path defined for later
- [ ] User review of this file
- [ ] Separate implementation design/plan only after approval to code

---

## 9. Stop

**STOP after this research document.**

Next step when ready: new brainstorm/design for **implementing** `UnverifiedPitchModel` (look + telemetry + docs), using §5 as the assumed math.

---

## Revision Notes

- **2026-09-17:** Research-only cycle. Approach 1. Aimlabs Valorant Game Profile (1:1 scaling, HFOV 103) recorded. Proceed with pitch = yaw constant `0.07` as best available UNCERTAIN hypothesis; clamp/sign remain UNCERTAIN.
- **2026-09-17 (operator-supplied reliable findings):** Locked candidate details for implementation:
  - Yaw: `ΔX × Sensitivity × 0.07`
  - Pitch scalar: **same** `0.07`/count; no separate vertical sens
  - Pitch direction: **positive ΔY → look down**
  - Pitch limit: **clamp ≈ ±89°** (UE4 gimbal-lock avoidance)
  - FOV/resolution independence: rotation per count is angular only (same policy as yaw)
  Confidence vs Riot primary docs remains **UNCERTAIN** unless/until a primary/official citation is attached; treated as **approved implementation assumption** for `UnverifiedPitchModel`.

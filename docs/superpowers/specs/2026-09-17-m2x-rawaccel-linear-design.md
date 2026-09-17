# M2.x Phase 1 — Raw Accel Linear — Design Spec

**Date:** 2026-09-17  
**Status:** Draft for user review  
**Depends on:** M2 COMPLETE (`docs/M2_PROCESSOR.md`); `docs/BASELINE.md`  
**Scope:** Faithful, deliberately limited reproduction of **Raw Accel Linear** as an `InputProcessor` — **not** a generic acceleration framework and **not** full Raw Accel parity

---

## 1. Purpose

Add one real acceleration experiment on the existing M2 pipeline:

```
WM_INPUT
  → immutable raw dx/dy + QPC timestamp
  → QPC inter-sample dt
  → dt_ms
  → Whole vector speed (counts/ms)
  → Raw Accel Linear scale
  → sensitivity multiplier
  → processed dx/dy
  → existing VALORANT sensitivity × 0.07
  → yaw / camera
```

M2.x Phase 1 proves we can reproduce **Raw Accel Linear** according to the official implementation (Guide + source), with unit-test proof before trusting live trainer behavior.

---

## 2. Naming & Identity

| Surface | Value |
|---------|--------|
| Human name | **Raw Accel Linear** |
| Factory / telemetry id | `rawaccel_linear` |
| Processor version | `1.0.0` |
| Struct | `RawAccelLinear` implementing `InputProcessor` |

**Provenance wording (required in docs and code comments):**

> Raw Accel Linear, reproduced according to the official implementation; mathematically equivalent to Classic with exponent 2 **where the documented equivalence applies**.

We implement **Linear**, not “Classic and calling it Linear.” Do not brand the feature as “Classic² / Linear.”

---

## 3. Locked Baseline (must not regress)

From `docs/BASELINE.md` and M2:

| Setting | Value |
|---------|--------|
| VSync | **ON** (`PresentMode::AutoVsync`) |
| Raw input | WM_INPUT + SetWindowSubclass |
| Timestamp | QPC → ns |
| `dt_s` | Consecutive raw mouse QPC timestamps only |
| Pitch | Disabled |
| Yaw constant | `0.07` **UNCERTAIN** |
| Mid-session processor swap | Forbidden |

Raw table remains immutable. Processed rows continue to store processor id / version / `config_json` per row.

---

## 4. Mathematics (Phase 1 subset)

### 4.1 Units

- Trainer `dt_s` from QPC: `(T_n - T_{n-1}) / 1e9`
- Raw Accel speed uses **counts per millisecond**:

\[
dt_{ms} = dt_s \times 1000
\]

Translating our QPC-derived interval into ms is **DERIVED** (our time base → units the Linear formula expects). QPC itself is not something Raw Accel’s formula requires.

### 4.2 Whole-vector input speed

When \(dt_{ms} > 0\):

\[
\boxed{v = \frac{\sqrt{dx^{2} + dy^{2}}}{dt_{ms}}}
\]

Units: **counts/ms**. The Guide example \((30,40)\) over 1 ms → \(v = 50\) is consistent with this formula only (not \((dx^{2}+dy^{2})/dt\)).

### 4.3 Linear scale and apply

Sensitivity / legacy Linear path with inactive offset/cap (Phase 1 defaults):

\[
\text{acceleration\_scale} = 1 + a \cdot v
\]

\[
(out_x, out_y) = (dx, dy) \times \text{acceleration\_scale} \times m
\]

where \(a\) = `acceleration`, \(m\) = `sensitivity_multiplier`.

Game/experiment yaw sensitivity remains **outside** the processor (existing `sens × 0.07` on processed dx).

### 4.4 Non-positive `dt_ms` (trainer boundary)

**EXPLICIT trainer rule — not a claim about Raw Accel’s internal first-event behavior.**

If \(dt_{ms} \le 0\):

| Field | Value |
|-------|--------|
| `dt_ms` | as computed (≤ 0) |
| `input_speed` | `0` |
| `acceleration_scale` | `1` |
| `bypassed_nonpositive_dt` | `true` |
| `processed_dx`, `processed_dy` | equal to raw `dx`, `dy` (identity; multiplier **not** applied on this path) |

Defined debug values keep the boundary deterministic and testable.

### 4.5 Normal path (`dt_ms > 0`)

```
input_speed         = sqrt(dx² + dy²) / dt_ms
acceleration_scale  = 1 + acceleration * input_speed
processed           = raw × acceleration_scale × sensitivity_multiplier
bypassed_nonpositive_dt = false
```

---

## 5. Config

`config_json` (snapshotted at session start / per processed row):

```json
{
  "acceleration": 0.01,
  "sensitivity_multiplier": 0.5
}
```

Only these two parameters in Phase 1.

**Idle / session defaults** (explicit, editable while Idle):

| Field | Default | Notes |
|-------|---------|--------|
| `acceleration` | `0.01` | Matches Guide example magnitude; user may change before Start |
| `sensitivity_multiplier` | `1.0` | Neutral RA multiplier; Guide’s `0.5` is for the unit-test vector, not the live default |

**Deferred (not in Phase 1 config):** input offset, output cap, Gain switch, By Component, anisotropy, EMA coalescing, DPI normalization, Classic general exponent, lookup tables.

---

## 6. Architecture

### 6.1 Crate layout (`sense-accel`)

- `RawAccelLinear` — `InputProcessor` impl (`id`, `version`, `config_json`, `process`)
- Pure helpers (no Bevy):
  - `vector_speed_counts_per_ms(dx, dy, dt_ms) -> f64`
  - `linear_acceleration_scale(v, acceleration) -> f64`
  - `apply_whole(dx, dy, acceleration_scale, sensitivity_multiplier) -> (f64, f64)`
  - Debug/evaluation helper returning a structured result (see §6.2)
- Factory: extend `create_processor` (or a thin wrapper) so `rawaccel_linear` is constructed with the session’s `acceleration` and `sensitivity_multiplier`; `"none"` ignores those fields. Unknown ids still fail closed.
- **No** shared LUT framework, **no** multi-mode accel enum scaffold beyond what this single processor needs

### 6.2 Debug / in-memory calculation path

Before adding permanent DB columns, expose a processor/test-layer result:

| Field | Meaning |
|-------|---------|
| `raw_dx`, `raw_dy` | Input counts |
| `dt_ms` | Interval in ms |
| `input_speed` | \(v\) (or `0` on bypass) |
| `acceleration_scale` | \(f(v)\) before multiplier (or `1` on bypass) |
| `processed_dx`, `processed_dy` | Output counts |
| `bypassed_nonpositive_dt` | Trainer boundary flag |

Purpose: if live behavior later disagrees with a real Raw Accel setup, isolate whether the gap is QPC→dt, speed, Linear equation, multiplier, or post-processor yaw — not only “final processed_dx differs.”

SQLite columns for these fields are **out of scope** for Phase 1 unless trivial; tests and in-memory debug come first.

### 6.3 App wiring (after unit tests green)

- Idle HUD: allow selecting `none` or `rawaccel_linear`; edit `acceleration` / `sensitivity_multiplier` while Idle
- Start Validation: `create_processor`; snapshot id/version/config; existing QPC `dt_s` path unchanged
- Running: `process(dx, dy, dt_s)` → yaw from processed dx
- Do not change WM_INPUT, integrity, or yaw constant

### 6.4 Separation of sensitivities

| Parameter | Where | Role |
|-----------|--------|------|
| `sensitivity_multiplier` (`m`) | Inside `RawAccelLinear` | Raw Accel post-scale (Guide) |
| Experiment / VALORANT `sens` | Camera path | `yaw = processed_dx × sens × 0.07` |

---

## 7. Provenance & Confidence

| Claim | Source | Confidence |
|-------|--------|------------|
| Guide Linear example: speed from whole magnitude; \((1 + a\cdot v)\cdot m\); \((30,40)\) @ 1 ms → sens \(0.75\), output velocity \(37.5\) | [Guide.md](https://github.com/RawAccelOfficial/rawaccel/blob/master/doc/Guide.md) Example | **CONFIRMED** |
| Whole-mode: scale entire vector by sensitivity function | Guide Whole section | **CONFIRMED** |
| Official tree has no separate Linear type; Linear matches Classic with exponent 2 on the Sensitivity/legacy path when offset/cap are inactive as in the Guide example | `accel-classic.hpp`, `accel-union.hpp`, `rawaccel-base.hpp` | **DERIVED** (equivalence where documented; we still implement Linear, not general Classic) |
| QPC `dt_s` → `dt_ms` for speed formula | Our M2 time base mapped to Guide units | **DERIVED** |
| `dt_ms ≤ 0` → identity + defined debug fields | Trainer implementation boundary | **EXPLICIT** (not RA-attributed) |

Primary references:

- https://github.com/RawAccelOfficial/rawaccel/blob/master/doc/Guide.md
- https://github.com/RawAccelOfficial/rawaccel/blob/master/common/accel-classic.hpp
- https://github.com/RawAccelOfficial/rawaccel/blob/master/common/rawaccel-base.hpp

---

## 8. Testing (unit tests first)

Live wiring and validation runs are **confirmation**, not first proof.

### Minimum mandatory suite

| Test | Expected | Proves |
|------|----------|--------|
| `(30,40)`, `dt_ms=1`, `a=0.01`, `m=0.5` | `v=50`, scale `1.5`, out `(22.5, 30)` | Official Guide reproduction |
| `(0,0)`, positive `dt_ms` | `(0,0)` | Zero vector stable |
| `dt_ms ≤ 0` | identity; `input_speed=0`; `acceleration_scale=1`; `bypassed_nonpositive_dt=true` | Trainer boundary |
| `(50,0)`, `dt_ms=1`, `a=0.01`, `m=0.5` | `(37.5, 0)` | Whole-vector, not By Component |
| `a=0`, positive motion | out = raw × `m` only | Acceleration term disappears |
| `(-30,-40)`, `dt_ms=1`, `a=0.01`, `m=0.5` | `(-22.5, -30)` | Magnitude scale; sign/direction preserved |

One negative-vector case is enough for the minimum suite (mixed-sign optional later).

### Also

- Factory accepts `rawaccel_linear`; unknown ids still fail closed
- `none` identity behavior unchanged
- Existing M1/M2 tests remain green

---

## 9. Explicitly Out of Scope (Phase 1)

- Shared LUT / multi-mode acceleration framework
- Natural, Power, Jump, Synchronous, lookup
- Classic as a general mode (beyond documenting Linear equivalence)
- Gain switch
- Input offset, output/in/io caps
- By Component mode
- Anisotropy (domain/range/Lp)
- EMA coalescing / smoothing
- DPI normalization (`NORMALIZED_DPI` / `output_dpi`)
- Depending on or calling the real Raw Accel driver
- Permanent processed-table columns for speed/scale (deferred)
- Mid-session processor swap
- Aim tasks; pitch; changing yaw constant; VSync off

---

## 10. Success Criteria

- `RawAccelLinear` reproduces Guide vector and minimum suite in unit tests
- Processor selectable as `rawaccel_linear` at session start; config snapshotted
- Dual telemetry continues; raw immutable
- Debug/in-memory path exposes `dt_ms`, `input_speed`, `acceleration_scale`, bypass flag
- Docs: short `docs/M2X_RAWACCEL_LINEAR.md` (or section under processor docs) with provenance table and confidence labels
- No scope creep into other Raw Accel modes or shared curve infrastructure

---

## 11. Stop After Phase 1

After Phase 1 lands and unit + smoke validation pass, **STOP**.

Further Raw Accel modes, Gain, caps/offsets, or driver-side comparison are separate design cycles.

---

## Revision Notes

- **2026-09-17:** Approach 1 (minimal faithful Linear). Naming: Raw Accel Linear reproduced from official implementation; Classic exponent-2 equivalence only where documented. Explicit \(\sqrt{\cdot}/dt_{ms}\) speed; trainer `dt_ms≤0` boundary with defined debug fields; mandatory Guide + sign-preservation tests; debug fields before DB columns.

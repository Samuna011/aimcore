# M2.x Phase 1 — Raw Accel Linear (Legacy / Sensitivity)

**Date:** 2026-09-17  
**Processor id:** `rawaccel_linear`  
**Processor version:** `1.0.0`  
**Experiment version:** `0.3.0` (sessions using this processor)  
**Status:** COMPLETE / STOPPED — Phase 1 only

---

## Purpose

M2.x Phase 1 adds one real acceleration experiment on the M2 pipeline:

```
WM_INPUT → raw dx/dy + QPC timestamp → dt_s → dt_ms → whole-vector speed
  → Linear scale (Legacy / Sensitivity) → sensitivity multiplier
  → processed dx/dy → VALORANT sens × 0.07 → yaw / camera
```

**Important:** This documents **mathematical behavior** reproduced from the official Raw Accel Guide and source tree. It is **not** actual-driver comparison proof. Do not claim full Raw Accel reproduction until a later cycle compares against the real driver.

**Provenance wording:**

> Raw Accel Linear (Legacy / Sensitivity), reproduced according to the official implementation; mathematically equivalent to Classic with exponent 2 **where the documented equivalence applies**.

We implement **Linear + Legacy/Sensitivity**, not Gain mode and not “Classic and calling it Linear.”

---

## Mode

| Surface | Value |
|---------|--------|
| Human name | Raw Accel Linear (Legacy / Sensitivity) |
| Factory / telemetry id | `rawaccel_linear` |
| Selected mode | **Linear** + **Legacy / Sensitivity** (not Gain) |

---

## Mathematics

### Units

- Trainer `dt_s` from consecutive raw QPC timestamps: `(T_n - T_{n-1}) / 1e9`
- Raw Accel speed uses **counts per millisecond**: `dt_ms = dt_s × 1000`

### Whole-vector input speed

When `dt_ms > 0`:

```
v = sqrt((dx * dx) + (dy * dy)) / dt_ms
```

Units: **counts/ms**. The Guide example `(30, 40)` over 1 ms → `v = 50`.

### Linear scale and apply

**Linear + Legacy / Sensitivity** with inactive offset/cap (Phase 1):

```
acceleration_scale = 1 + acceleration * v
(out_x, out_y)     = (dx, dy) × acceleration_scale × sensitivity_multiplier
```

Game/experiment yaw sensitivity (`sens × 0.07`) remains **outside** the processor.

### Trainer boundary: non-positive `dt_ms`

**EXPLICIT trainer rule — not a claim about Raw Accel’s internal first-event behavior.**

If `dt_ms ≤ 0`:

| Field | Value |
|-------|--------|
| `dt_ms` | as computed (≤ 0) |
| `input_speed` | `0` |
| `acceleration_scale` | `1` |
| `bypassed_nonpositive_dt` | `true` |
| `processed_dx`, `processed_dy` | equal to raw `dx`, `dy` (identity; multiplier **not** applied) |

### Normal path (`dt_ms > 0`)

```
input_speed         = sqrt((dx * dx) + (dy * dy)) / dt_ms
acceleration_scale  = 1 + acceleration * input_speed
processed           = raw × acceleration_scale × sensitivity_multiplier
bypassed_nonpositive_dt = false
```

---

## Config

`config_json` (snapshotted at session start / per processed row):

```json
{
  "acceleration": 0.01,
  "sensitivity_multiplier": 0.5
}
```

### Trainer defaults vs official Raw Accel defaults

These are **trainer** defaults (editable while Idle), **not** claimed as official Raw Accel GUI/driver defaults:

| Field | Trainer default | Notes |
|-------|-----------------|-------|
| `acceleration` | `0.01` | Chosen to match the Guide example’s magnitude for local checks; **not** an official Raw Accel default |
| `sensitivity_multiplier` | `1.0` | Neutral RA multiplier; Guide’s `0.5` is for the unit-test vector, not the live default |

---

## Provenance & Confidence

| Claim | Source | Confidence |
|-------|--------|------------|
| Guide Linear example: speed from whole magnitude; `(1 + a·v)·m`; `(30,40)` @ 1 ms → sens `0.75`, output velocity `37.5` | [Guide.md](https://github.com/RawAccelOfficial/rawaccel/blob/master/doc/Guide.md) Example | **CONFIRMED** |
| Whole-mode: scale entire vector by sensitivity function | Guide Whole section | **CONFIRMED** |
| Official tree has no separate Linear type; Linear + Legacy/Sensitivity matches Classic with exponent 2 when offset/cap are inactive as in the Guide example | `accel-classic.hpp`, `accel-union.hpp`, `rawaccel-base.hpp` | **DERIVED** (equivalence where documented; we still implement Linear + Legacy/Sensitivity, not general Classic or Gain) |
| QPC `dt_s` → `dt_ms` for speed formula | Our M2 time base mapped to Guide units | **DERIVED** |
| `dt_ms ≤ 0` → identity + defined debug fields | Trainer implementation boundary | **EXPLICIT** (not RA-attributed) |

Primary references:

- https://github.com/RawAccelOfficial/rawaccel/blob/master/doc/Guide.md
- https://github.com/RawAccelOfficial/rawaccel/blob/master/common/accel-classic.hpp
- https://github.com/RawAccelOfficial/rawaccel/blob/master/common/rawaccel-base.hpp

Design spec: [2026-09-17-m2x-rawaccel-linear-design.md](./superpowers/specs/2026-09-17-m2x-rawaccel-linear-design.md)

---

## Unit-test proof (minimum suite)

Live wiring is confirmation, not first proof. Mandatory tests in `crates/sense-accel/tests/processor_tests.rs`:

| Test | Expected | Proves |
|------|----------|--------|
| `(30,40)`, `dt_ms=1`, `a=0.01`, `m=0.5` | `v=50`, scale `1.5`, out `(22.5, 30)` | Official Guide reproduction |
| `(0,0)`, positive `dt_ms` | `(0,0)` | Zero vector stable |
| `dt_ms ≤ 0` | identity; bypass flag | Trainer boundary |
| `(50,0)`, `dt_ms=1`, `a=0.01`, `m=0.5` | `(37.5, 0)` | Whole-vector, not By Component |
| `a=0`, positive motion | out = raw × `m` only | Acceleration term disappears |
| `(-30,-40)`, `dt_ms=1`, `a=0.01`, `m=0.5` | `(-22.5, -30)` | Sign/direction preserved |

---

## Explicitly Out of Scope (Phase 1)

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
- Actual-driver comparison (separate design cycle)

---

## Stop After Phase 1

**STOP.** Phase 1 is complete. Unit tests prove documented math; smoke validation confirms wiring.

Further Raw Accel modes (Natural, Classic general, Gain), caps/offsets, LUT infrastructure, or driver-side comparison require a **new design spec and approval cycle**. Do not implement without one.

---

## Related Documents

- [M2_PROCESSOR.md](./M2_PROCESSOR.md) — M2 framework and processor registry
- [ARCHITECTURE.md](./ARCHITECTURE.md) — Bevy wiring and crate layout
- [BASELINE.md](./BASELINE.md) — locked VSync / WM_INPUT / pitch settings

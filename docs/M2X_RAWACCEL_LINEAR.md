# M2.x Phase 1.1 — Raw Accel Linear (Gain + Caps)

**Date:** 2026-09-17  
**Processor id:** `rawaccel_linear`  
**Processor version:** `1.1.0`  
**Experiment version:** `0.4.0` for all new sessions, regardless of processor  
**Status:** COMPLETE / STOPPED — Phase 1.1 only

---

## Purpose

M2.x Phase 1.1 extends the Linear processor so **Gain** and all three official **cap modes** (`out`, `in`, `io`) match Raw Accel **1:1** (source port from `accel-classic.hpp`).

```
WM_INPUT → raw dx/dy + QPC timestamp → dt_s → dt_ms → whole-vector speed
  → Classic Linear scale (Gain or Legacy/Sensitivity; caps per config)
  → sensitivity multiplier → processed dx/dy → VALORANT sens × 0.07 → yaw / camera
```

**Important:** This documents **mathematical behavior 1:1 with the official source**. It is **not** actual-driver comparison proof. Do not claim full Raw Accel reproduction until a later cycle compares against the real driver.

**Provenance wording (required):**

> Raw Accel Linear, reproduced according to the official implementation (`accel-classic.hpp`); mathematically equivalent to Classic with exponent 2 where that documented equivalence applies. Gain vs Legacy/Sensitivity and cap modes follow the official classic templates **1:1**.

Do not brand as “Classic² / Linear.”

Phase 1 Guide / Sensitivity path remains available via `gain: false` and inactive output cap for regression verification.

---

## Mode

| Surface | Value |
|---------|--------|
| Human name | Raw Accel Linear (Gain or Legacy/Sensitivity; caps per config) |
| Factory / telemetry id | `rawaccel_linear` |
| Primary path | **Linear** + **Gain** + **Output cap** (trainer defaults) |
| Verification path | **Linear** + **Legacy / Sensitivity** + inactive cap → Phase 1 Guide behavior |

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

### Shared Linear / Classic exponent 2

Official Linear uses Classic with `exponent_classic = 2`.

```
accel_raised = pow(acceleration, exponent - 1)
// for exponent 2: accel_raised = acceleration

base_fn(x) = accel_raised * pow(x - input_offset, exponent) / x
```

If `x <= input_offset`: return scale `1` (official).

Reference: [accel-classic.hpp](https://github.com/RawAccelOfficial/rawaccel/blob/master/common/accel-classic.hpp) (`classic_base::base_fn`, classic templates).

### Legacy / Sensitivity (`gain = false`)

Port of non-gain `classic` template:

```
scale = sign * min(base_fn(x), cap) + 1
(out_x, out_y) = (dx, dy) × scale × sensitivity_multiplier
```

Cap init (official):

| `cap_mode` | Behavior (summary; implement exactly as source) |
|------------|--------------------------------------------------|
| `out` | `cap = cap_y - 1` (e.g. Output 2 → addend cap 1 → sens ≤ 2); handle negative cap → flip `sign` |
| `in` | `cap = base_fn(cap_x, …)` when `cap_x > 0` |
| `io` | derive acceleration via `base_accel(cap_x, cap_y-1, …)`, set `accel_raised`, set addend `cap` |

Inactive output cap: when `cap_y` not positive → effectively uncapped (`cap = +inf`). With offset 0, reduces to Phase 1 `1 + acceleration * v`.

### Gain (`gain = true`)

Port of gain `classic` template, including helpers:

```
gain(x, accel, power, offset) = power * pow(accel * (x - offset), power - 1)
gain_inverse(y, accel, power, offset) = (accel * offset + pow(y / power, 1 / (power - 1))) / accel
gain_accel(...)  // for io mode — copy source exactly
```

After init (`cap.x`, `cap.y`, `constant`, `sign`, `accel_raised`):

```
if x <= input_offset: scale = 1
else if x < cap.x:    output = base_fn(x)
else:                 output = constant / x + cap.y
scale = sign * output + 1
(out_x, out_y) = (dx, dy) × scale × sensitivity_multiplier
```

**Example profile (trainer default orientation):** Gain on, Cap Type Output, Cap Output 2, Offset 0, Acceleration 0.007, Sens Multiplier 1 → `cap_mode = out`, `cap_y = 2`, `cap_x` from `gain_inverse`, piecewise as above.

Game/experiment yaw sensitivity (`sens × 0.07`) remains **outside** the processor.

### Trainer boundary: non-positive `dt_ms`

**EXPLICIT trainer rule — not attributed to Raw Accel.**

`dt_s` comes from consecutive QPC timestamps. The first sample in a session/burst has no previous timestamp → `dt_s = 0` → `dt_ms = 0`. Non-increasing timestamps can also yield `dt_ms <= 0`.

Velocity-based Linear requires a positive interval. The trainer does **not** invent official first-event behavior.

When `dt_ms <= 0`:

| Field | Value |
|-------|--------|
| `bypassed_nonpositive_dt` | `true` |
| `input_speed` | `0` |
| `acceleration_scale` | `1` |
| `processed_dx`, `processed_dy` | equal to raw `dx`, `dy` (identity; Gain, caps, and `sensitivity_multiplier` **not** applied) |

Official Gain/cap math runs **only** when `dt_ms > 0`.

### Normal path (`dt_ms > 0`)

```
input_speed         = sqrt((dx * dx) + (dy * dy)) / dt_ms
acceleration_scale  = classic.scale(input_speed)   // LEGACY or GAIN from source
processed           = raw × acceleration_scale × sensitivity_multiplier
bypassed_nonpositive_dt = false
```

---

## Config

`config_json` (snapshotted at session start / per processed row):

```json
{
  "acceleration": 0.007,
  "sensitivity_multiplier": 1.0,
  "gain": true,
  "input_offset": 0.0,
  "cap_mode": "out",
  "cap_x": 0.0,
  "cap_y": 2.0
}
```

### Trainer defaults vs official Raw Accel defaults

These are **trainer** defaults (editable while Idle), **not** claimed as official Raw Accel GUI/driver defaults:

| Field | Trainer default | Notes |
|-------|-----------------|-------|
| `acceleration` | `0.007` | Play-profile oriented; **not** an official Raw Accel default |
| `sensitivity_multiplier` | `1.0` | |
| `gain` | `true` | Primary path = Gain |
| `input_offset` | `0.0` | |
| `cap_mode` | `"out"` | `"out"` \| `"in"` \| `"io"` |
| `cap_x` | `0.0` | Used by `in` / `io`; for `out` Gain path, effective `cap.x` is computed in ctor |
| `cap_y` | `2.0` | Output ratio style (e.g. Output 2) |

**Optional verification preset:** `gain: false`, inactive cap (non-positive `cap_y` for out) → Phase 1 Guide path; use `RawAccelLinearConfig::phase1_sensitivity(0.01, 0.5)` in tests.

Factory: `sense_accel::create_processor(id, &RawAccelLinearConfig)` — config ignored for `"none"`.

---

## Provenance & Confidence

| Claim | Source | Confidence |
|-------|--------|------------|
| Linear ≡ Classic exponent 2 in official tree | Guide + `accel-union` / classic | **DERIVED** (documented equivalence; we implement Linear surface) |
| `base_fn`, Legacy min+1, Gain piecewise + helpers | `accel-classic.hpp` | **CONFIRMED** (1:1 port target) |
| Cap modes out / in / io init | same | **CONFIRMED** |
| Whole-vector apply × multiplier | Guide Whole | **CONFIRMED** |
| Phase 1 Guide `(30,40)` with gain off + inactive cap | Guide example | **CONFIRMED** (regression path) |
| QPC `dt_s` → `dt_ms` | Trainer time base | **DERIVED** |
| `dt_ms <= 0` identity bypass | Trainer boundary | **EXPLICIT** (not RA-attributed) |

Primary references:

- https://github.com/RawAccelOfficial/rawaccel/blob/master/common/accel-classic.hpp
- https://github.com/RawAccelOfficial/rawaccel/blob/master/common/rawaccel-base.hpp
- https://github.com/RawAccelOfficial/rawaccel/blob/master/doc/Guide.md

Design specs:

- [2026-09-17-m2x-rawaccel-linear-design.md](./superpowers/specs/2026-09-17-m2x-rawaccel-linear-design.md) — Phase 1
- [2026-09-17-m2x-rawaccel-linear-gain-caps-design.md](./superpowers/specs/2026-09-17-m2x-rawaccel-linear-gain-caps-design.md) — Phase 1.1

---

## Unit-test proof (minimum suite)

Live wiring and GUI smoke are confirmation only; operator smoke remains **operator-pending**. Mandatory tests in `crates/sense-accel/tests/processor_tests.rs`:

| Test | Proves |
|------|--------|
| Phase 1 Guide `(30,40)` with gain off + inactive cap → `(22.5, 30)` | Regression |
| Gain + `out` + `cap_y=2`: point below `cap.x` matches `base_fn+1` | Gain uncapped region |
| Same: point above `cap.x` matches `constant/x + cap.y + 1` | Gain output-cap knee |
| Legacy + `out` + `cap_y=2`: `min(base_fn, 1)+1` | Sensitivity output cap |
| Cap `in` and `io` init + evaluate at least one vector each | Mode coverage |
| `v <= input_offset` → scale 1 | Official offset |
| `dt_ms <= 0` → identity + debug fields | Trainer EXPLICIT rule |
| Sign preservation on negative vector | Direction |
| Factory v1.1.0 + config JSON includes all fields | Wiring |

---

## Explicitly Out of Scope (Phase 1.1)

- Shared LUT / multi-mode acceleration framework
- Natural, Power, Jump, Synchronous, lookup
- Classic as a general mode (beyond documented Linear equivalence)
- By Component mode
- Anisotropy (domain/range/Lp)
- EMA coalescing / smoothing
- DPI normalization (`NORMALIZED_DPI` / `output_dpi`)
- Depending on or calling the real Raw Accel driver
- Actual Raw Accel driver comparison harness
- Permanent processed-table columns for speed/scale (deferred)
- Mid-session processor swap
- Rotation, Y/X output ratio beyond multiplier

---

## Stop After Phase 1.1

**STOP.** Phase 1.1 is complete. Unit tests prove the documented math. Live GUI/SQLite smoke validation remains **operator-pending** and is confirmation only.

Further work (Natural, anisotropy, other modes, driver comparison) requires a **new design spec and approval cycle**. Do not implement without one.

---

## Related Documents

- [M2_PROCESSOR.md](./M2_PROCESSOR.md) — M2 framework and processor registry
- [ARCHITECTURE.md](./ARCHITECTURE.md) — Bevy wiring and crate layout
- [BASELINE.md](./BASELINE.md) — locked VSync / WM_INPUT / pitch settings

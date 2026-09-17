# M2.x Phase 1.1 — Raw Accel Linear Gain + Caps — Design Spec

**Date:** 2026-09-17  
**Status:** Approved for implementation  
**Depends on:** M2.x Phase 1 (`docs/superpowers/specs/2026-09-17-m2x-rawaccel-linear-design.md`); `docs/M2X_RAWACCEL_LINEAR.md`  
**Scope:** Extend `rawaccel_linear` so Gain switch and all three official cap modes match Raw Accel **1:1** (source port). Keep Phase 1 Sensitivity / inactive-cap path as optional verification.

---

## 1. Purpose

Make the trainer’s Linear processor usable against a real Raw Accel Linear profile that uses **Gain on** and **caps** (e.g. Cap Type Output 2), without inventing simplified math.

Primary path defaults toward a play profile (Gain + Output cap). Optional path: Gain off + inactive cap = Phase 1 Guide / Sensitivity behavior for regression.

Claim level remains: **documented mathematical behavior 1:1 with official source** — not actual-driver A/B proof until a later cycle.

---

## 2. Naming & Identity

| Surface | Value |
|---------|--------|
| Human name | **Raw Accel Linear** (Gain or Legacy/Sensitivity; caps per config) |
| Factory / telemetry id | `rawaccel_linear` (unchanged) |
| Processor version | `1.1.0` |
| Experiment version | `0.4.0` (all new sessions; align docs to code) |

**Provenance wording (required):**

> Raw Accel Linear, reproduced according to the official implementation (`accel-classic.hpp`); mathematically equivalent to Classic with exponent 2 where that documented equivalence applies. Gain vs Legacy/Sensitivity and cap modes follow the official classic templates **1:1**.

Do not brand as “Classic² / Linear.”

---

## 3. Locked Baseline

- VSync ON; WM_INPUT; QPC `dt_s`; mid-session processor swap forbidden  
- Dual telemetry; raw table immutable  
- Always `migrate()` on DB open (processed table)  
- Accelerated validation degrees use camera yaw (`total_yaw_delta_deg`); raw counts retained for telemetry  
- No anisotropy, rotation, Y/X ratio domain/range, EMA, DPI norm, By Component, other styles, LUT framework, driver dependency  

---

## 4. Approaches (locked)

**Approach 1:** Extend `rawaccel_linear` in place with Gain + all cap modes; Sensitivity/no-cap remains configurable.

**Cap scope:** all three official modes — `out`, `in`, `io`.

---

## 5. Mathematics (1:1 with `accel-classic.hpp`)

### 5.1 Speed (unchanged)

When `dt_ms > 0`:

```
v = sqrt((dx * dx) + (dy * dy)) / dt_ms
```

Then:

```
scale = classic_linear_scale(v)   // LEGACY or GAIN from source
(out_x, out_y) = (dx, dy) * scale * sensitivity_multiplier
```

Game/VALORANT `sens × 0.07` remains outside the processor.

### 5.2 Shared Linear / Classic exponent 2

Official Linear uses Classic with `exponent_classic = 2`.

```
accel_raised = pow(acceleration, exponent - 1)
// for exponent 2: accel_raised = acceleration

base_fn(x) = accel_raised * pow(x - input_offset, exponent) / x
```

If `x <= input_offset`: return scale `1` (official).

Reference: [accel-classic.hpp](https://github.com/RawAccelOfficial/rawaccel/blob/master/common/accel-classic.hpp) (`classic_base::base_fn`, classic templates).

### 5.3 Legacy / Sensitivity (`gain = false`)

Port of non-gain `classic` template:

```
scale = sign * min(base_fn(x), cap) + 1
```

Cap init (official):

| `cap_mode` | Behavior (summary; implement exactly as source) |
|------------|--------------------------------------------------|
| `out` | `cap = cap_y - 1` (e.g. Output 2 → addend cap 1 → sens ≤ 2); handle negative cap → flip `sign` |
| `in` | `cap = base_fn(cap_x, …)` when `cap_x > 0` |
| `io` | derive acceleration via `base_accel(cap_x, cap_y-1, …)`, set `accel_raised`, set addend `cap` |

Inactive output cap: treat as source does when `cap_y` not positive → effectively uncapped (`cap = +inf`). Then with offset 0, reduces to Phase 1 `1 + acceleration * v`.

### 5.4 Gain (`gain = true`)

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
```

**Example profile (trainer default orientation):** Gain on, Cap Type Output, Cap Output 2, Offset 0, Acceleration 0.007, Sens Multiplier 1 → `cap_mode = out`, `cap_y = 2`, `cap_x` from `gain_inverse`, piecewise as above.

### 5.5 Non-positive `dt_ms` (trainer boundary)

**EXPLICIT — not attributed to Raw Accel.**

`dt_s` comes from consecutive QPC timestamps. The first sample in a session/burst has no previous timestamp → `dt_s = 0` → `dt_ms = 0`. Non-increasing timestamps can also yield `dt_ms <= 0`.

Velocity-based Linear requires a positive interval. The trainer does **not** invent official first-event behavior.

When `dt_ms <= 0`:

| Field | Value |
|-------|--------|
| `bypassed_nonpositive_dt` | `true` |
| `input_speed` | `0` |
| `acceleration_scale` | `1` |
| `processed_dx`, `processed_dy` | equal to raw (identity; Gain, caps, and `sensitivity_multiplier` **not** applied) |

Official Gain/cap math runs **only** when `dt_ms > 0`.

---

## 6. Config & defaults

`config_json` example (snapshotted per row / session):

```json
{
  "acceleration": 0.007,
  "sensitivity_multiplier": 1.0,
  "gain": true,
  "input_offset": 0.0,
  "cap_mode": "out",
  "cap_x": 2.0,
  "cap_y": 2.0
}
```

| Field | Trainer default | Notes |
|-------|-----------------|--------|
| `acceleration` | `0.007` | Trainer default (play-profile oriented); **not** an official Raw Accel default |
| `sensitivity_multiplier` | `1.0` | |
| `gain` | `true` | Primary path = Gain |
| `input_offset` | `0.0` | |
| `cap_mode` | `"out"` | `"out"` \| `"in"` \| `"io"` |
| `cap_x` | `2.0` | Same default as Cap Y; for `out` Gain, effective knee still from Cap Y + accel |
| `cap_y` | `2.0` | Output ratio style (e.g. Output 2) |

**Optional verification preset:** `gain: false`, inactive cap (per source: non-positive `cap_y` for out) → Phase 1 Guide path must still pass unit tests.

---

## 7. Architecture

### 7.1 Crate layout

```
crates/sense-accel/src/
  lib.rs                 # trait, factory, RawAccelLinear wrapper
  classic_linear.rs      # 1:1 port of classic LEGACY + GAIN (exponent 2)
```

- Ctor precomputes the same state the C++ classic constructors store (`accel_raised`, `cap`, `constant`, `sign`, …).
- `eval_*` debug struct continues to expose `dt_ms`, `input_speed`, `acceleration_scale` (= scale before multiplier), bypass flag, processed counts.
- Factory still: `create_processor(id, …)` extended to pass full Linear args (or a small `RawAccelLinearConfig` struct). Prefer a config struct to avoid an exploding arity.

### 7.2 App / HUD

- Idle: Gain toggle; Cap mode combo; Cap X / Cap Y (show/enable as mode requires); Offset; Acceleration; Sensitivity multiplier.
- Running: locked; snapshotted config in telemetry.
- Copy: Linear + Gain or Legacy/Sensitivity; caps per official modes; trainer defaults labeled as such.

### 7.3 Separation of sensitivities

| Parameter | Where |
|-----------|--------|
| `sensitivity_multiplier` | Inside processor (Raw Accel post-scale) |
| Experiment VALORANT `sens` | Camera: `yaw = processed_dx × sens × 0.07` |

---

## 8. Provenance & confidence

| Claim | Source | Confidence |
|-------|--------|------------|
| Linear ≡ Classic exponent 2 in official tree | Guide + `accel-union` / classic | **DERIVED** (documented equivalence; we implement Linear surface) |
| `base_fn`, Legacy minsd+1, Gain piecewise + helpers | `accel-classic.hpp` | **CONFIRMED** (1:1 port target) |
| Cap modes out / in / io init | same | **CONFIRMED** |
| Whole-vector apply × multiplier | Guide Whole | **CONFIRMED** |
| QPC `dt_s` → `dt_ms` | Trainer time base | **DERIVED** |
| `dt_ms <= 0` identity bypass | Trainer boundary | **EXPLICIT** |

Primary references:

- https://github.com/RawAccelOfficial/rawaccel/blob/master/common/accel-classic.hpp  
- https://github.com/RawAccelOfficial/rawaccel/blob/master/common/rawaccel-base.hpp  
- https://github.com/RawAccelOfficial/rawaccel/blob/master/doc/Guide.md  

---

## 9. Testing (unit tests first)

Live feel is confirmation only.

Minimum suite:

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

Factory / HUD wiring tests as needed; workspace green before trusting live.

---

## 10. Explicitly out of scope

- Anisotropy (domain/range/Lp), rotation, Y/X output ratio beyond multiplier  
- EMA / coalescing / smoothing  
- DPI normalization  
- By Component  
- Natural / Power / Jump / Synchronous / LUT  
- Actual Raw Accel driver comparison harness  
- Changing yaw constant; aim tasks; pitch  

---

## 11. Success criteria

- Classic Linear Gain + out/in/io match source formulas in unit tests  
- Defaults usable for Gain + Output 2 style profile  
- Phase 1 Guide path still selectable and green  
- Docs updated with provenance and EXPLICIT `dt_ms` rule  
- Stop after 1.1 — no other styles without a new spec  

---

## 12. Stop after Phase 1.1

After implementation + unit tests + operator smoke, **STOP**.

Further work (driver comparison, anisotropy, other modes) = new design cycle.

---

## Revision Notes

- **2026-09-17:** Approach 1; primary Gain + all cap modes 1:1 with `accel-classic.hpp`; Sensitivity/no-cap optional; trainer `dt_ms<=0` bypass clarified as EXPLICIT; trainer defaults oriented to Gain/Output 2 / a=0.007 / m=1.

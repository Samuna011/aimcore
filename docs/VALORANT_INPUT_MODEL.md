# VALORANT Input Model (M1)

**Date:** 2026-09-17  
**Scope:** Horizontal yaw only in M1. Pitch is telemetry-only; camera pitch rotation is disabled.

---

## Core Formulas

### Yaw delta (per raw sample)

```
degrees_per_count = sensitivity × 0.07
yaw_delta_deg     = raw_dx × degrees_per_count
```

Equivalently:

```
yaw_delta_deg = raw_dx × sensitivity × 0.07
```

### Derived quantities

```
eDPI            = DPI × sensitivity
counts_per_360  = 360 / (sensitivity × 0.07)
inches_per_360  = counts_per_360 / DPI
cm_per_360      = inches_per_360 × 2.54
```

**DPI does not enter camera math.** DPI only affects how many counts the hardware emits for a physical distance.

---

## Units

| Quantity | Unit |
|----------|------|
| `raw_dx`, `raw_dy` | mouse counts |
| `sensitivity` | dimensionless (game setting) |
| `DPI` | counts/inch |
| `degrees_per_count`, yaw/pitch/FOV | degrees |
| `cm_per_360`, `inches_per_360` | centimeters / inches |

Never mix pixels, counts, and degrees without an explicit conversion.

---

## Worked Examples (Unrounded Internal Math)

| Sensitivity | deg/count | 1000 counts |
|-------------|-----------|-------------|
| 0.15 | 0.0105 | 10.5° |
| 0.175 | 0.01225 | 12.25° |
| 0.20 | 0.014 | 14.0° |

Default experimental config (not a recommendation):

- DPI 1600, sensitivity 0.175, eDPI 280
- Yaw constant 0.07°/count at sens 1.0
- Horizontal FOV 103°
- Acceleration OFF

At 1600 DPI / 0.175:

- eDPI = 280
- yaw/count = 0.01225°
- counts/360 ≈ 29387.755…
- cm/360 ≈ 46.65 cm (display rounding only)

---

## Same-eDPI Equivalence

Same eDPI configurations share the same theoretical **cm/360**, but they do **not** necessarily share the same **counts/360**.

| Relationship | Depends on |
|--------------|------------|
| cm/360 | sensitivity **and** DPI (via eDPI) |
| counts/360 | sensitivity only |

| DPI | Sens | eDPI | counts/360 | cm/360 |
|-----|------|------|------------|--------|
| 800 | 0.35 | 280 | ≈14693.88 | ≈46.65 cm |
| 1600 | 0.175 | 280 | ≈29387.76 | ≈46.65 cm |
| 3200 | 0.0875 | 280 | ≈58775.51 | ≈46.65 cm |

Internal math is unrounded; table values are illustrative.

**Unit test expectation:** 800×0.35, 1600×0.175, 3200×0.0875 → same eDPI and same cm/360; counts/360 must differ according to sensitivity.

---

## FOV and Resolution Independence

Profile fields:

- `fov_axis = Horizontal`
- `fov_degrees = 103`

Vertical FOV is derived from horizontal FOV and viewport aspect ratio. Do not hand-tune vertical FOV.

**Angular sensitivity model independence:**

Changing resolution or FOV must **not** change:

- `degrees_per_count`
- `counts_per_360`
- `cm_per_360`

FOV/resolution **may** change projection and screen-space representation (how many pixels a given angular movement subtends). That is expected and must not be “fixed” by altering yaw math.

---

## Provenance: Yaw Constant `0.07`

| Field | Value |
|-------|-------|
| Constant | `0.07` degrees per count at sensitivity 1.0 |
| Source | Community-derived VALORANT yaw model (Reddit/wiki/community posts; not an official Riot API document) |
| Date checked | 2026-09-17 |
| Claim | Horizontal look rotation in VALORANT follows `yaw_delta = dx × sensitivity × 0.07` degrees |
| Confidence | **UNCERTAIN** |

This constant is treated as a **hypothesis** until empirically confirmed against VALORANT via the Validation Lab 360° test. Do not present it as CONFIRMED or official.

Confidence labels used in this project:

| Label | Meaning |
|-------|---------|
| CONFIRMED | Verified against primary/official source |
| DERIVED | Logically derived from confirmed facts |
| EMPIRICALLY TESTED | Measured in Validation Lab or controlled test |
| **UNCERTAIN** | Community-derived or unverified assumption |

M1 status for `0.07`: **UNCERTAIN**.

---

## M1 Compensation Policy

**M1 does not silently compensate** for discrepancies between observed rotation and expected VALORANT math.

If expected vs observed counts or degrees differ:

1. Report the discrepancy in the HUD and persisted validation result.
2. Report input-integrity counters separately so pipeline faults can be distinguished from math mismatch.
3. Do **not** auto-adjust sensitivity, yaw constant, or camera state to hide the error.

Mathematical equivalence is the target; tuning until it “feels right” is forbidden.

---

## Pitch (M1)

Vertical input (`raw_dy`) is recorded but does not rotate the camera. VALORANT pitch behavior is **unverified** in M1. No pitch formula is asserted as correct.

---

## Related Documents

- [ARCHITECTURE.md](./ARCHITECTURE.md) — system layout and input path
- [EXPERIMENT_MODEL.md](./EXPERIMENT_MODEL.md) — 360° validation using signed net counts
- [TELEMETRY_SCHEMA.md](./TELEMETRY_SCHEMA.md) — where raw and derived values are stored

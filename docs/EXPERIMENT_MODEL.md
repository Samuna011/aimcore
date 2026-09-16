# Experiment Model (M1)

**Date:** 2026-09-17  
**M1 experiment:** Validation Lab only  
**experiment_id:** `validation_lab`  
**experiment_version:** `0.1.0`

---

## Overview

M1 ships one experiment: the **Validation Lab** — a controlled 360° horizontal rotation test that compares observed mouse counts and degrees against VALORANT yaw math without silent compensation.

Future experiments (aim tasks, movement segmentation, etc.) are out of M1 scope.

---

## Identifiers

| ID type | Format | When allocated |
|---------|--------|----------------|
| `config_id` | `config_NNNNNN` | Start Validation (configuration snapshot) |
| `session_id` | `session_YYYYMMDD_NNNNNN` | Start Validation |

At session start, snapshot into SQLite:

- App version
- `experiment_id` = `validation_lab`
- `experiment_version` = `0.1.0`
- DPI, sensitivity, eDPI
- FOV (horizontal 103° default), resolution, refresh
- Acceleration state (`NoAcceleration` in M1)
- Polling rate if known
- Random seed (stored even if unused)

Do not rely on live application settings to reconstruct historical runs.

---

## Validation Lab Flow

```
┌─────────────┐     ┌──────────────┐     ┌─────────────────────┐
│ Start       │────▶│ Active       │────▶│ End Validation      │
│ Validation  │     │ rotation     │     │ persist + report    │
└─────────────┘     └──────────────┘     └─────────────────────┘
       │                    │                       │
       ▼                    ▼                       ▼
  session_id           sample-by-sample        validation_results
  config snapshot      yaw + telemetry         + integrity report
```

### Step-by-step

1. **Start Validation**
   - Allocate `session_id` and `config_id`.
   - Snapshot configuration + versions into `configurations` and `sessions`.
   - Begin recording raw mouse events and input camera samples.

2. **Reset Camera** (optional, any time before/during)
   - Set yaw to a known forward value (e.g. 0°).
   - Pitch remains frozen (M1).
   - Emit `InputCameraSample` with `event_kind = camera_reset`.

3. **Reset Counters** (optional)
   - Zero horizontal/vertical count accumulators in HUD.
   - Emit telemetry marker; does not end session.

4. **User rotation**
   - User performs **one continuous horizontal rotation in a single direction without reversing**.
   - Each raw sample: drain queue → apply yaw via `sense-math` → record telemetry.

5. **End Validation**
   - Compute results using **the same `sense-math` functions as the live app**.
   - Persist raw events, validation result, and integrity report.
   - **Do not** auto-compensate for discrepancy.

---

## Signed Net Counts Rule

360° validation uses **signed net horizontal counts**, not absolute path length.

| Metric | Formula | Used for validation? |
|--------|---------|----------------------|
| Observed counts | **signed** Σ `raw_dx` (net counts) | **Yes** |
| Absolute path counts | Σ \|raw_dx\| | No — telemetry only |

**Rationale:** A full 360° rotation in one direction should yield net counts equal to expected counts. Absolute path counts include reversals and over-rotation and must not replace signed net for the primary validation calculation.

### Expected values (via `sense-math`)

```
expected_counts  = 360 / (sensitivity × 0.07)
expected_degrees = 360
observed_degrees = observed_counts × sensitivity × 0.07
difference       = observed_degrees − expected_degrees
error_percent    = (difference / expected_degrees) × 100
```

Example at 1600 DPI / 0.175 sens:

- eDPI = 280
- expected counts/360 ≈ 29387.755…
- cm/360 ≈ 46.65 cm

---

## Input Integrity → Pipeline-Suspect

During validation, track pipeline health separately from math match:

| Counter | Meaning |
|---------|---------|
| `samples_received` | Total raw samples in window |
| `sequence_gaps` | Missing sequence numbers |
| `duplicate_sequences` | Repeated sequence numbers |
| `out_of_order_samples` | Sequence regressions |
| `timestamp_regressions` | `timestamp_ns` going backward |

**Rule:** If any integrity counter is non-zero, set `pipeline_suspect = 1` on the validation result.

| Condition | Action |
|-----------|--------|
| `pipeline_suspect = 0` | Math discrepancy likely reflects model vs reality or user protocol deviation |
| `pipeline_suspect = 1` | Treat run as **pipeline-suspect** — still show math discrepancy; do not auto-correct |

Report integrity counters in HUD and persist with `validation_results`.

---

## HUD Requirements (egui)

### Live display

- DPI, Sensitivity, eDPI, Yaw (°/count), HFOV, Resolution, Refresh, cm/360, counts/360
- raw dx, raw dy, total horizontal counts, total vertical counts
- current yaw, current pitch (frozen), total degrees rotated (yaw)
- Pitch banners: `PITCH MODEL: UNVERIFIED`, `PITCH ROTATION: DISABLED`

### Controls

| Control | Effect |
|---------|--------|
| Reset Camera | Known forward yaw |
| Reset Counters | Zero count accumulators |
| Start Validation | New session + config snapshot |
| End Validation | Compute, persist, report |

---

## Compensation Policy

M1 **never silently compensates** for expected vs observed mismatch. Discrepancies are reported; constants and settings are not auto-adjusted to hide errors.

See [VALORANT_INPUT_MODEL.md](./VALORANT_INPUT_MODEL.md) for yaw constant provenance (`0.07` = **UNCERTAIN**).

---

## Out of Scope (M1 Experiments)

- STATIC_CLICK / flick / tracking / target switching
- Movement segmentation and flick phase classifier
- Multi-experiment scheduling
- User/device management tables
- Export pipelines (CSV/JSON)

---

## Related Documents

- [ARCHITECTURE.md](./ARCHITECTURE.md) — Bevy app and telemetry pipeline
- [TELEMETRY_SCHEMA.md](./TELEMETRY_SCHEMA.md) — table definitions and integrity fields
- [VALORANT_INPUT_MODEL.md](./VALORANT_INPUT_MODEL.md) — formulas and same-eDPI table

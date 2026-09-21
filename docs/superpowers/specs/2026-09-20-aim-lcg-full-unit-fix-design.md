# Aim LCG Full-Unit Fix — Design Spec

**Date:** 2026-09-20  
**Status:** Approved for implementation (follow-up from TRACKING final review)  
**Depends on:** TRACKING v1 (`0.12.0`)  
**Experiment version after ship:** `0.12.1`  
**Scope:** Fix `next_unit` half-range bias in STATIC_CLICK / GRIDSHOT; bump their `task_version` to `"2"`

## Goal

Make aim LCG `next_unit` return values in **[0, 1)** as documented, so STATIC_CLICK uses the full yaw/pitch cone and GRIDSHOT can select all 9 cells. Preserve interpretability of old DB rows via `task_version`.

## Bug

```text
((*rng >> 33) as f64) / (u32::MAX as f64 + 1.0)
→ numerator ~31 bits, divisor 2^32 → unit ∈ [0, 0.5)
```

## Fix

Shared helper (preferred): `pub(crate) fn lcg_next_unit(rng: &mut u64) -> f64` in `aim_trial.rs`:

```text
((*rng >> 33) as f64) / ((1u64 << 31) as f64)   // [0, 1)
```

- `aim_gridshot` and `aim_tracking` call the shared helper (remove duplicates).
- Bump `STATIC_CLICK_TASK_VERSION` and `GRIDSHOT_TASK_VERSION` → **`"2"`**.
- Keep TRACKING `task_version` at `"1"` (already used corrected divisor); optionally switch TRACKING to shared helper without bumping TRACKING task_version (same math).
- Bump `rng_version` in STATIC_CLICK / GRIDSHOT `task_config_json` to `"2"` if present, else rely on `task_version` alone.
- `EXPERIMENT_VERSION` / `AIM_EXPERIMENT_VERSION` → **`0.12.1`**.

## Non-goals

- Re-seeding or migrating old trials
- Changing cone limits, grid layout, or scoring
- New task types

## Tests

- `next_unit` can exceed 0.5 (many draws)
- GRIDSHOT `next_index(..., 9)` can return indices ≥ 5 over many draws
- STATIC_CLICK `next_range(-25, 25)` can be positive over many draws
- Same seed still deterministic; update golden “same seed same cells” to assert equality only (not old half-range fixtures)
- Docs: BASELINE / README / TELEMETRY note task_version 2 = full-unit LCG

## Stop

Ship `0.12.1`; stop.

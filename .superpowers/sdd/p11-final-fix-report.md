# Phase 1.1 Final Fix Re-Review Report

**Date:** 2026-09-17  
**Scope:** Re-review of Important fixes from Phase 1.1 final review  
**Diff reviewed:** `76837bd…de301f3` (`p11-final-fix-review-pkg.md`)

## Verdict

**Needs fixes**

## Prior Important Findings — Status

| # | Finding | Status |
|---|---------|--------|
| 1 | `io` mode with `cap_x = 0` (and `cap_x <= input_offset`) accepted at factory → NaN at runtime | **Fixed** — `RawAccelLinearConfig::validate()` rejects before `ClassicLinearState` construction; `create_processor("rawaccel_linear", …)` fail-closed; regression tests pass |
| 2 | `TELEMETRY_SCHEMA.md` version drift (`0.3.0` / `1.0.0` vs code `0.4.0` / `1.1.0`) | **Fixed** — schema examples now match `session.rs` `EXPERIMENT_VERSION` and `RawAccelLinear` v1.1.0 |

## Remaining Critical / Important

### Important — `in` mode rejects valid inactive-cap configs

`validate()` rejects `CapMode::In` when `cap_x <= 0`:

```67:68:crates/sense-accel/src/lib.rs
            CapMode::In if self.cap_x <= 0.0 => {
                return Err("in cap_x must be greater than 0".into());
```

Official classic behavior treats `cap_x <= 0` in `in` mode as an **inactive input cap** (no NaN; `ClassicLinearState` skips cap init when `cap_x > 0` is false). The HUD exposes `in` mode with trainer-default `cap_x = 0.0`, so switching cap mode to `in` without raising `cap_x` causes session start to fail even though the math is well-defined.

**Suggested fix:** Drop the `CapMode::In` `cap_x <= 0` rejection (or allow `cap_x == 0` explicitly). Keep the `io` guard (`cap_x > input_offset`) and the gain/out zero-acceleration guard.

## Verification Performed

- `cargo test --workspace`: 55 passed, 0 failed
- Confirmed `factory_rejects_io_with_zero_cap_x` and `factory_accepts_valid_io_and_processes_finite` pass
- Confirmed `docs/TELEMETRY_SCHEMA.md` contains no stale `0.3.0` / example `1.0.0` processor version strings
- Confirmed production wiring (`session.rs`, `validation_lab.rs`) routes through validated `create_processor`

---

## Fix Applied — inactive `in` cap (2026-09-17)

**Commit message:** `fix: allow inactive Linear in-cap (cap_x<=0) like official classic`

### Change

- Removed `CapMode::In` `cap_x <= 0` rejection from `RawAccelLinearConfig::validate()` — official classic treats non-positive `cap_x` in `in` mode as inactive (safe; `ClassicLinearState` only initializes when `cap_x > 0`).
- Retained fail-closed guards: `io` when `cap_x <= input_offset`, non-finite numerics, negative `input_offset`, gain/out zero-acceleration combo.

### Tests added

- `validate_accepts_in_with_zero_cap_x` — `CapMode::In`, `cap_x = 0` → `validate().is_ok()`
- `factory_accepts_in_with_zero_cap_x` — same config → `create_processor` succeeds, finite output
- `factory_rejects_io_with_zero_cap_x` — unchanged, still `Err`

### Verification

- `cargo test -p sense-accel`: 24 passed, 0 failed
- `cargo test --workspace`: 57 passed, 0 failed

### Verdict (updated)

**All Important findings resolved**

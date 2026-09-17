# M2.x Phase 1 Final Review Fix Report

**Date:** 2026-09-17
**Scope:** All Important findings from the M2.x Phase 1 final review

## Fixes

1. **Processor-aware validation semantics**
   - `none` retains M1/M2 raw-count-derived observed degrees and count-based error percentage.
   - Non-`none` processors use `LiveInputStats.total_yaw_delta_deg` as observed degrees and compute error against the 360-degree target.
   - Raw `observed_net_counts`, `observed_abs_path_counts`, and `count_difference` remain intact as honest telemetry.
   - The completed result stores its processor id so the HUD can explain that expected-counts is not a 360-degree proof under acceleration and that displayed degrees use camera yaw delta.
   - Added a regression test proving accelerated results preserve raw counts while using camera yaw for degrees and error.

2. **Experiment version documentation**
   - Aligned `README.md`, `docs/M2_PROCESSOR.md`, `docs/M2X_RAWACCEL_LINEAR.md`, and `docs/TELEMETRY_SCHEMA.md` with code: all new sessions use experiment version `0.3.0`, regardless of processor.
   - Updated validation-result field documentation for processor-aware degree/error semantics.

3. **Smoke-validation claim**
   - Replaced the unsupported wiring-confirmation claim with the research-honest statement that unit tests prove documented math while live GUI/SQLite smoke remains operator-pending and confirmation-only.

## Verification

- Focused regression tests: 3 passed, 0 failed.
- `cargo test --workspace`: 42 passed, 0 failed.
- `cargo fmt --all -- --check`: passed.
- IDE diagnostics for edited Rust files: no errors.
- `git diff --check`: passed; only Git's informational LF-to-CRLF warnings were emitted for edited Rust files.

## Remaining Concern

- Live GUI/SQLite smoke validation remains operator-pending by design; no live wiring proof is claimed.

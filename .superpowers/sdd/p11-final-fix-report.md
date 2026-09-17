# Phase 1.1 Final Review Fix Report

**Date:** 2026-09-17
**Scope:** Important findings from the Phase 1.1 final review

## Fixes

- Added `RawAccelLinearConfig::validate` and invoked it in the `rawaccel_linear` factory path before constructing `ClassicLinearState`.
- Validation rejects non-finite numeric arguments, negative input offsets, `io` caps where `cap_x <= input_offset`, and `in` caps where `cap_x <= 0`.
- Validation also rejects the gain/output-cap zero-acceleration combination that would make `gain_inverse` non-finite. `out` with the trainer default `cap_x = 0` remains valid.
- Added regression coverage proving `io` with `cap_x = 0` returns an error and `io` with `cap_x = 40`, `cap_y = 2` constructs and processes finite output.
- Updated `docs/TELEMETRY_SCHEMA.md` examples to experiment version `0.4.0` and processor version `1.1.0`.

## Verification

- TDD regression reproduced before implementation: invalid `io` configuration was accepted and the new test failed.
- `cargo test --workspace`: 55 passed, 0 failed.
- `git diff --check`: passed.
- IDE diagnostics for edited Rust files: no errors.

## Concern

- Repository-wide `cargo fmt --all -- --check` still reports a pre-existing formatting difference in `crates/sense-telemetry/tests/db_roundtrip.rs`, outside this fix's scope.

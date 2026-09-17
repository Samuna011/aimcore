# M1 Final Whole-Branch Review Fixes

## 2026-09-17

- Reset Counters now resets live counters, telemetry buffers, and `IntegrityTracker` together.
- End Validation now calls `TelemetryDb::complete_validation`, which flushes all sample streams, inserts the validation result, and records the session end time in one SQLite transaction.
- Raw-input read failures are baselined at Start Validation. The session delta is folded into `sequence_gaps` at End Validation because each failed read represents a missing sample, making `pipeline_suspect` true without a schema migration.
- Reset Camera now appends an `InputCameraSample` while Running using the current QPC monotonic timestamp and reserved sequence number `0` (raw mouse sequences begin at `1`).
- Signed-net validation math remains unchanged; yaw is never auto-compensated.
- Regression evidence: the new tests failed before implementation because the completion method, integrity-aware reset, raw-failure report adjustment, and camera-reset sample path did not exist.

## Verification

- `cargo test --workspace` — passed (26 tests, 0 failures).
- `cargo build` — passed.

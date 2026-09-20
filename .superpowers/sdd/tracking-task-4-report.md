# Task 4 Report: TRACKING v1 experiment 0.12.0 + docs stop

**Date:** 2026-09-20
**Status:** COMPLETE / STOPPED

## Summary

Bumped `AIM_EXPERIMENT_VERSION` and `EXPERIMENT_VERSION` to `0.12.0`. Updated BASELINE, README, and TELEMETRY_SCHEMA for TRACKING v1 (hold∧ray scoring, 30s active, `direction_change` events, no `aim_shots`, no schema change). Fixed telemetry test fixtures. Did not bump STATIC_CLICK / GRIDSHOT `task_version`.

## Commit

`6ac03a3` — `docs(app): TRACKING v1 experiment 0.12.0`

## Files changed

| File | Change |
|------|--------|
| `src/aim_trial.rs` | `AIM_EXPERIMENT_VERSION = "0.12.0"` + test assert |
| `src/session.rs` | `EXPERIMENT_VERSION = "0.12.0"` + test assert |
| `docs/BASELINE.md` | exp 0.12.0, TRACKING v1 in progression, stop criteria |
| `README.md` | TRACKING v1 status section, updated final status line |
| `docs/TELEMETRY_SCHEMA.md` | TRACKING trial_type, scoring semantics, 0.12.0 |
| `crates/sense-telemetry/tests/db_roundtrip.rs` | fixture `experiment_version` → 0.12.0 |

## Tests

`cargo test --workspace` — **151 passed**, 0 failed.

## Stop criteria met

- TRACKING playable from Lobby, persisted, History-replayable (Tasks 1–3)
- Hold∧ray sample scoring + pause-safe motion (Task 2)
- Docs + `0.12.0` (Task 4)
- **STOP** — no 1wall6 / vertical tracking / shot-based tracking scoring

## Concerns

None. Untracked `crates/sense-telemetry/examples/` left out of commit (pre-existing, not part of Task 4).

## Final review fixes

- C-1: Added a per-frame TRACKING tick driven by `monotonic_now_ns()`. The frame tick exclusively owns motion, hold-and-ray scoring, reversal events, and the 30s end check; the WM_INPUT path now only updates LMB level and buffers input/camera telemetry. `AimTrial::last_tracking_tick_ns` is reset on resume so pause time cannot enter frame `dt`.
- I-2: TRACKING persisted duration and both record/HUD accuracy denominators clamp active elapsed to 30.0s.
- I-3: Fixed only `aim_tracking.rs` LCG normalization to cover `[0, 1)`, allowing reverse delays across the full `[1.5, 3.5)` range. STATIC_CLICK and GRIDSHOT RNG code was not changed.
- I-4: TRACKING pause emits a zero-velocity `direction_change`; resume emits another with restored live `vx`, preserving History freeze semantics.

### Test evidence

- `cargo test -p sense-maxer aim_tracking::tests` — **18 passed**, 0 failed.
- `cargo test -p sense-maxer` — **92 passed**, 0 failed.
- Added regression coverage for idle-mouse frame completion, reverse delays above 2.5s, pause/resume velocity events, and 30s duration/accuracy clamping.

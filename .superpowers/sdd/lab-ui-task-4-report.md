# Lab UI Task 4 Report — Experiment 0.10.0 + docs stop

**Date:** 2026-09-19  
**Status:** COMPLETE / STOPPED

## Summary

Final Lab UI Shell task: bumped experiment version to `0.10.0`, updated docs for UI shell + pause semantics, fixed persisted `duration_secs` to use pause-excluded active time.

## Changes

| File | Change |
|------|--------|
| `src/aim_trial.rs` | `AIM_EXPERIMENT_VERSION = "0.10.0"`; `build_completed_aim_trial_record` sets `duration_secs` from `score_secs` / `active_elapsed_ns` (not wall-clock); new unit test `completed_record_duration_excludes_pause` |
| `src/session.rs` | `EXPERIMENT_VERSION = "0.10.0"`; unit assert updated |
| `docs/BASELINE.md` | Exp `0.10.0`; Lab UI shell in progression; stop line updated |
| `README.md` | Lab UI shell status block; Esc = pause (aim) vs look (validation); exp `0.10.0` |
| `docs/TELEMETRY_SCHEMA.md` | Exp `0.10.0`; `duration_secs` / `score_secs` documented as pause-excluded active time |

## Not changed (per brief)

- `STATIC_CLICK` / `GRIDSHOT` `task_version` unchanged
- No new task types (tracking / 1wall6)
- No SQLite schema changes

## Tests

```
cargo test -p sense-maxer   → 60 passed
cargo test -p sense-telemetry → 7 passed
```

## Commit

```
docs(app): Lab UI shell experiment 0.10.0
```

## Concerns

None. Lab UI Shell milestone is complete; next work requires a new task spec.

## Final review fixes

**Date:** 2026-09-19

- Added an explicit `LabScreen::Validating`. Start Validation enters it; successful End Validation returns to Lab tools unlocked. Esc toggles validation look/cursor capture without touching aim pause state, while Playing Esc retains pause/resume behavior and Lobby Esc remains a no-op.
- Mouse draining now accepts captured input in `Playing` or `Validating`, restoring validation samples.
- Restored Lab tools Reset Camera and Reset Counters actions using the existing reset paths.
- Restored validation expected/observed count and degree feedback plus integrity counters, and Settings eDPI/counts-per-360/cm-per-360 readouts.
- Resume now clears both aim raw-sample timing and processor timing so the first post-resume sample starts with a zero interval.
- Lobby and Pause Home now display session/trial start failure status.
- Added/extended regression coverage for pause-state reset on start/cancel, STATIC_CLICK active-time scoring across a pause, validation look behavior/drain gating, processor timing reset, and preservation of seed/targets/hits/log lengths/start timestamp across resume.
- Updated README Esc instructions for the explicit Validating workflow.

### Test evidence

```text
cargo test -p sense-maxer
PASS — 63 passed; 0 failed

cargo check -p sense-maxer
PASS — finished dev profile successfully
```

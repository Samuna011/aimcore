# M3.y final review — follow-up

## Critical fixes (2026-09-19)

### Critical 1 — Look mode gaps
**Finding:** `drain_mouse_to_camera` drained and discarded the mouse queue when `!look.enabled` while aim could remain Armed → silent gaps if the run later completed.

**Fix:** Added `on_look_disabled(&mut AimTrial) -> bool` which calls `cancel_aim_trial` when phase is Armed (clear buffers, no DB write). Wired from `drain_mouse_to_camera` before return; sets status `"Aim run cancelled — look unlocked (ESC) mid-trial."`

### Critical 2 — Mutable snapshot at finish
**Finding:** Processor/settings/resolution could change while Armed; persist built the trial record from live values at finish.

**Fix:**
1. `AimRunConfigSnapshot` captured at `start_aim_trial` (processor id/version/config, dpi, sens, poll, FOV, pitch fields, resolution/aspect, hardware/view JSON).
2. `build_completed_aim_trial_record` / `persist_completed_aim_trial` use **only** that Start snapshot (+ score/timing/seed/buffers).
3. HUD: processor/gain/caps/poll and DPI/sens controls disabled while Armed (same pattern as validation running); live processor sync skipped while Armed.

### Tests
- `start_captures_config_snapshot_fields`
- `settings_mutation_after_start_does_not_change_built_record`
- `on_look_disabled_cancels_armed_only`

### Verify
- `cargo test -p sense-maxer` — pass
- `cargo test -p sense-telemetry --test db_roundtrip` — pass

### Commit
`fix(aim): snapshot config at Start and cancel Armed on look unlock`

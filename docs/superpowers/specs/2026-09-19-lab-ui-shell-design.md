# Lab UI Shell — Lobby / Playing / Pause — Design Spec

**Date:** 2026-09-19  
**Status:** Approved for implementation (user locked flow + surfaces + internals)  
**Depends on:** M4.a GRIDSHOT v1 (`0.9.0`), M3.y aim telemetry, M3.x completed-only persist  
**Experiment version after ship:** `0.10.0`  
**Scope:** UI/navigation + pause gameplay freeze; no new aim task types; no new SQLite tables

## Goal

Replace the always-on Validation Lab egui dump with a proper **screen-state machine**: trial selection and configuration in **Lobby**, gameplay in **Playing**, interruption in **Paused**, with research telemetry behind **V**. Preserve aim-task semantics, immutable Start snapshots, and completed-only persistence.

## Non-goals

- New task types (tracking / 1wall6)
- SQLite schema changes or history browser
- Natural / anisotropy / driver harness
- Fancy non-egui UI framework or heavy visual redesign beyond clear screen layouts
- Changing STATIC_CLICK or GRIDSHOT hit/miss/scoring rules except **pause-excluded active time**

---

## Architecture approach

**Full screen-state machine** (Approach 1):

```
Lobby ──Start──► Playing ◄──Esc/Resume──► Paused
                     │                      │
                     │ finish               ├── Settings (locked if Armed)
                     ▼                      ├── Lab tools
                   Lobby                    ├── Restart / Change trial / Exit
```

One egui pass (`draw_hud`) gates which surface draws. New `LabUi` resource owns screen, selected task, V preference, and lightweight last-result display. Aim/validation runtime resources stay authoritative for trial data.

---

## Screen modes

| Mode | Cursor / look | Trial clock | Aim input |
|------|---------------|-------------|-----------|
| Lobby | Free; look off | N/A | N/A |
| Playing | Locked; look on | Active (running) | LMB shoots if Armed |
| Paused | Free; look off | Frozen | No shots; no camera apply |

Nested panels **Settings** and **Lab tools** are reachable from **Lobby** (pre-trial) and from **Paused → Home**. Pause **Home** also has Resume / Restart / Change trial / Exit. Lobby has no Resume/Restart (no Armed trial).

---

## Escape rules (unambiguous)

| Context | Esc |
|---------|-----|
| Lobby | No-op (does not abort or start anything) |
| Playing | Enter **Paused** |
| Paused | **Resume** → Playing |

**Esc never means Abort** anywhere. Abort only via Restart, Change trial, or Exit while Armed.

---

## Pause must stop gameplay (not only the timer)

On enter **Paused** while an aim trial is Armed:

1. Trial clock freezes (see Pause accounting).
2. Mouse-look stops affecting the camera.
3. Target/task simulation stops (no timer-driven end while paused; no respawn driven by paused time).
4. **No** aim input samples or camera gameplay samples attributed to the paused interval.
5. Cursor unlocks.
6. Trial remains **Armed / in progress** — not completed, not aborted.

This is especially required for Gridshot (60s active duration).

---

## Resume continues the same trial

Resume must **not** reset:

- `random_seed`
- Target layout / live targets
- Hit / shot / miss counters
- In-memory telemetry buffers
- Trial start QPC
- Accumulated active-play time so far

Pause duration is excluded from active trial duration only.

---

## Start

From Lobby, with selected task (Static Click | Gridshot):

1. Snapshot experiment-affecting settings into the existing aim run config snapshot (M3.y immutability).
2. Arm trial (existing start paths).
3. Enable look / cursor grab.
4. Enter **Playing**.

Start = arm **and** enter arena (no second Esc to lock look).

---

## Restart (destructive)

While Paused with Armed trial:

1. Abort current Armed trial (discard all in-memory trial telemetry; **no** DB insert).
2. Re-arm the **same** task kind.
3. New run state: **new** `random_seed`, reset camera (existing aim Start reset), clear counters/buffers.
4. Enter Playing with look on.

Restart is **not** a continuation and must not reuse the previous trial’s seed or in-memory identity.

---

## Change trial

While Paused with Armed trial:

1. Abort (no persist).
2. Enter Lobby (mode picker). Settings become editable again.

---

## Exit to desktop

From Pause (or Lobby): if Armed, abort (no persist), then quit the app.

---

## Settings lock (global rule)

Experiment-affecting settings (DPI, sens, HFOV, processor, rawaccel knobs, etc.) lock based on **Armed**, not screen name:

| State | Settings |
|-------|----------|
| Lobby, no aim Armed | Editable |
| Playing (Armed) | Locked |
| Paused with Armed trial | Locked |
| After completed trial → Lobby | Editable |
| After explicit abort → Lobby | Editable |

No exception allows changing sensitivity / DPI / processor while paused — that would invalidate the trial snapshot.

**Lobby** is the only pre-trial configuration surface for the next aim run:

```
Lobby
├── Trial selection
├── Settings
├── Lab tools
└── Start
```

Once Start is pressed, settings remain immutable until the trial is **completed** or **explicitly aborted**.

---

## Lab tools vs Settings

- **Settings** modify the **next** aim trial’s conditions.
- **Lab tools** operate the separate Validation Lab (Start / End Validation + last validation result).

Mutual exclusion (unchanged):

- No aim Armed → Validation may Start / End.
- Aim Armed → Validation Start blocked.

---

## Playing HUD

### Normal (V off)

Only information needed to play:

- Center crosshair
- Compact score line: mode · hits/shots · accuracy · time (time remaining for Gridshot; elapsed for Static Click) using **active** (pause-excluded) time

### Detail overlay (V on)

Additive overlay (not a second window). Research/debug telemetry for the **current run**.

Read-only values that describe the run must come from the **trial config snapshot**, not live `ExperimentSettings`:

- DPI, sensitivity, HFOV
- Processor id + short cfg
- `random_seed`
- Experiment version

Also allowed on V overlay while Playing: FPS / frame ms, live raw Δ / yaw / pitch rates, last shot HIT/MISS, DB status line. Validation session status lives under **Lab tools**, not the Playing V overlay (Validation cannot run while Armed).

### V preference

`detail_overlay: bool` lives on **UI state** (`LabUi`), not trial state. Persists across pause / resume / restart / change-task unless the user toggles **V**. Pausing or restarting must not reset it.

---

## Persist toast

Persist status appears **only** after successful or failed trial completion/persistence — never during an active trial or pause. On finish, return to **Lobby** and show the toast there (alongside the lightweight last-result strip).

| Status | Meaning |
|--------|---------|
| SAVED | Trial + all child telemetry committed (atomic txn) |
| SAVE FAILED | No completed trial persisted; in-memory completed result may remain visible |

Matches M3.x completed-only rules. Aborts never show SAVED.

---

## Lobby last result (lightweight)

Optional strip after a completed run — not a lab dump. Example shape:

```
Last result
Gridshot · 42 hits · 91.3% · 58.2s
Saved: aim_2026-09-19_000042
```

No raw samples, processor dumps, or analysis metrics in Lobby.

---

## Pause accounting

Maintain:

- `start_qpc` (unchanged on pause/resume)
- `accumulated_pause_ns`
- `paused_at_qpc` while currently paused

**Active elapsed** = `(now_qpc − start_qpc) − accumulated_pause_ns` (and while paused, use `paused_at_qpc` instead of `now` for the open interval, or equivalently freeze displayed/elapsed at pause entry).

Gridshot end condition: first sample/frame where **active elapsed ≥ 60.0 s**.  
STATIC_CLICK `score_secs`: active elapsed at 5th hit.

Persisted duration / score fields must reflect **active** time only (document in TELEMETRY / BASELINE).

While paused: do not append aim `input_samples` / `camera_samples` for gameplay.

---

## `LabUi` resource (conceptual)

```text
screen: Lobby | Playing | Paused
nested: None | Settings | LabTools | PauseHome   // PauseHome only valid when screen=Paused
selected_task: StaticClick | Gridshot
detail_overlay: bool          // V preference
last_result: Option<LightweightResult>  // lobby strip
```

LookCapture remains the cursor/look gate; Esc routing is owned by the UI/input layer per Escape rules above. **Remove** the previous behavior where look-unlock cancelled an Armed trial.

---

## Version / docs

- Bump `EXPERIMENT_VERSION` → **`0.10.0`** (UI shell + Esc semantics + pause-excluded active duration — run comparability vs `0.9.0`)
- Update BASELINE / README / TELEMETRY_SCHEMA: UI shell; Esc = pause; active duration excludes pause; Esc no longer cancels Armed
- Do **not** bump `task_version` for STATIC_CLICK / GRIDSHOT: hit rules and grid invariants unchanged; timing semantics change is covered by `experiment_version`

---

## Testing

- Esc never aborts; Playing→Paused→Resume round-trip
- Pause freezes active elapsed; wall-clock advance while paused does not consume Gridshot budget
- Resume preserves seed, live targets, counters, buffers, start QPC
- Restart aborts + new seed + Playing
- Change trial → Lobby, settings editable
- Settings widgets locked whenever Armed (including Paused)
- V preference independent of trial lifecycle
- V overlay DPI/sens/processor/HFOV/seed/version match snapshot while Armed
- Validation Start blocked while Armed
- Existing STATIC_CLICK / GRIDSHOT / persist tests remain green with pause helpers where needed

## Stop criteria

- Lobby / Playing / Pause flow playable for Static Click + Gridshot
- Pause freezes gameplay + active clock; Resume continues same Armed run
- Settings immutable from Start until complete/abort; Lab tools mutually exclusive with Armed
- V toggles detail overlay from snapshot; Esc never aborts
- Docs + `0.10.0`; **Stop** — no new task types in this milestone

## Architectural summary

**Lobby owns pre-trial configuration. Playing owns gameplay. Pause owns temporary interruption. Settings become immutable at Start and remain immutable until the trial is completed or explicitly aborted. Lab tools remain independently gated from Armed aim trials.**

# Aim History Browser — 3D Arena Replay — Design Spec

**Date:** 2026-09-19  
**Status:** Approved for implementation (user locked flow + fidelity + internals)  
**Depends on:** Lab UI shell (`0.10.0`), M3.y five-layer aim telemetry, M4.a GRIDSHOT  
**Experiment version after ship:** `0.11.0`  
**Scope:** Read-only History list + Bevy 3D arena replay of completed aim trials; no schema changes

## Goal

Let the operator open **Lobby → History**, pick a completed aim trial, and **replay it in the Bevy arena** (play / pause / speed) driven by persisted `aim_camera_samples`, `aim_target_events`, and `aim_shots` — so recording completeness and spatial fidelity can be verified visually.

## Non-goals

- Completeness dashboard / `dt_ns` waveform charts (can follow later)
- Validation-session replay
- Delete / edit / compare two runs
- Side-by-side export HTML viewer
- Schema changes or new telemetry columns
- New aim task types

---

## Architecture approach

**Bevy 3D arena replay (Approach 2):**

1. Read-only SQLite loaders for trial summaries + full replay bundles.  
2. Extend `LabScreen` with `HistoryList` and `HistoryReplay`.  
3. While replaying: freeze live input→camera; advance a replay clock; apply absolute camera poses from samples; sync spheres from target events; flash shot markers.

Live aim Armed / Validation remain mutually exclusive with History replay.

---

## Screen flow

Lobby gains **History** (only when no aim Armed and not Validating).

| Screen | Cursor / look | Behavior |
|--------|---------------|----------|
| HistoryList | Free; look off | List completed trials; select → Replay; Back → Lobby |
| HistoryReplay | Free; look does **not** steer camera | Arena reconstruct + transport HUD; Back → HistoryList (unload) |

**Esc in HistoryReplay:** pause / resume replay playback (soft pause of the replay clock). Does **not** leave History or abort anything.  
**Back:** leave replay, unload bundle, return to HistoryList.

Cannot Start aim or Start Validation while on HistoryList or HistoryReplay.

---

## HistoryList

Scrollable list, newest first (by `end_unix_ms`). Columns (minimum):

- `id`
- `trial_type` (`STATIC_CLICK` / `GRIDSHOT`)
- hits / shots / accuracy
- `score_secs` (or duration)
- `experiment_version`
- end time (from `end_unix_ms`)

Actions: **Replay** (selected row), **Back**.

Empty DB → empty-state message, no crash.

---

## HistoryReplay — reconstruct fidelity

### Camera

- Drive `YawPitch` **only** from `aim_camera_samples` absolute `yaw_deg` / `pitch_deg`.
- Replay time `t` in `[start_timestamp_ns, end_timestamp_ns]` (from trial row).
- Pose at `t`: **hold** the latest sample with `timestamp_ns ≤ t` (step hold). No interpolation in v1 — preserves visible sample cadence.
- If no sample yet at `t`, hold start pose `(0,0)` or first sample when reached.

### Targets

- Apply `aim_target_events` in timestamp order.
- `spawn`: show sphere for `target_id` at event position (center from stored yaw/pitch or x/y/z / cell fields as persisted).
- `despawn`: hide/remove that `target_id`.
- Live set at `t` = net of events with `timestamp_ns ≤ t`.
- Do **not** infer despawn from shots; events own lifecycle.

### Shots

- At each `aim_shots` timestamp ≤ `t` (or on crossing): brief HIT (green) / MISS (red) flash at crosshair or aim ray.
- Optional short-lived marker; shots must not mutate target occupancy.

### Transport HUD

- Play / Pause  
- Speed: **1×, 2×, 4×**  
- Scrub bar (seek `t` within trial span) — required for inspection even with animated play  
- Compact readout: trial id, type, playback time / duration, speed, live target count, last shot outcome  
- Optional snapshot strip from trial row (DPI, sens, processor, seed, experiment_version) — read-only  

### Clock

- While playing: `t += speed * real_dt_ns` (QPC or Bevy `Time`), clamped to end; at end, pause at final pose.
- Pause: freeze `t`.
- Seek: set `t`, recompute pose + live targets from scratch (or incremental cursor reset).

---

## Data layer (read-only)

No schema changes. New APIs on `TelemetryDb` (names indicative):

```text
list_aim_trials_summary(limit: usize) -> Vec<AimTrialSummary>
load_aim_trial_bundle(id: &str) -> Result<AimTrialReplayBundle, _>
```

`AimTrialReplayBundle` contains:

- `AimTrialRecord` (summary + snapshot fields)
- `Vec<AimTargetEventRecord>` ordered by timestamp
- `Vec<AimShotRecord>` ordered by timestamp
- `Vec<AimCameraSampleRecord>` ordered by timestamp

`aim_input_samples` **not required** for v1 camera drive (may load later for overlays).

Fail loudly if trial id missing or child streams fail to load (surface error on HistoryList; do not enter Replay half-loaded).

---

## LabUi / runtime integration

```text
LabScreen += HistoryList | HistoryReplay
AimReplay resource: bundle Option, playing, speed, t_ns, last_shot flash state
```

- `HistoryReplay`: `drain_mouse_to_camera` must **not** apply live look to the camera (gate like non-Playing, or explicit Replay exclusion).
- Replay system runs in Update: advance clock → apply yaw/pitch → sync aim target entities from net events.
- Reuse existing aim sphere pool / sync path where practical.
- Leaving Replay restores Lobby/HistoryList camera policy (e.g. reset look to identity or leave last replay pose — prefer **reset to identity** on Back for predictable Lobby).

---

## Version / docs

- Bump `EXPERIMENT_VERSION` / `AIM_EXPERIMENT_VERSION` → **`0.11.0`**
- Update BASELINE / README / TELEMETRY_SCHEMA: History browser + 3D replay is read-only reconstruct; no schema bump
- Do not bump `task_version`

---

## Testing

- `list` / `load` round-trip against a fixture completed trial insert
- At synthetic `t`, camera pose equals last sample ≤ `t`; live targets equal event netting
- Seek + speed do not panic; end clamps
- Cannot Start aim / Validation from History screens
- Existing STATIC_CLICK / GRIDSHOT / persist / Lab UI tests remain green

## Stop criteria

- HistoryList + HistoryReplay playable for STATIC_CLICK and GRIDSHOT completed rows  
- Hold-sample camera + event-driven targets + shot flashes  
- Play / pause / speed / scrub; Esc pauses replay only  
- Read-only DB APIs; `0.11.0` docs  
- **Stop** — no completeness charts, validation replay, or trial delete/compare in this milestone

## Architectural summary

**Lobby opens History. HistoryList picks a completed trial. HistoryReplay reconstructs the arena from persisted camera samples, target events, and shots. Replay never writes telemetry and never steers from live mouse.**

# M2.x — Trainer Time Clamp + Speed Debug HUD — Design Spec

**Date:** 2026-09-17  
**Status:** Approved (Approach C) — **manual feel A/B passed** (2026-09-17): Linear-on trainer much closer to official RA; residual slight diffs OK as human error.  
**Depends on:** `2026-09-17-rawaccel-dpi-poll-research-findings.md`  
**Goal:** Reduce Linear-on “hotter than official RA” feel from user-mode `WM_INPUT` stamp bursts; expose live `dt_ms` / speed so we can verify.

---

## 1. Decision

**Approach C:** Poll-period floor on speed `dt_ms` + live HUD debug.

- **EXPLICIT trainer rule** (not claimed as Device poll = 0 RA behavior).
- Compensates user-mode QPC-at-handle vs kernel QPC; accel-off path unchanged (identity).

---

## 2. Mathematics

When raw `dt_ms > 0` (after existing bypass):

```text
min_ms = polling_rate_hz > 0 ? (1000 / polling_rate_hz) : 0.0625   // RA DEFAULT_TIME_MIN
max_ms = 100.0                                                     // RA DEFAULT_TIME_MAX
dt_speed_ms = clamp(raw_dt_ms, min_ms, max_ms)
v = hypot(dx, dy) / dt_speed_ms
```

- Bypass `raw_dt_ms <= 0` unchanged (identity; no multiplier).
- Classic Gain/cap scale unchanged; only the **time used for speed** is clamped.
- Trainer default: `polling_rate_hz = 1000` → `min_ms = 1.0`.

---

## 3. Config / version

- `RawAccelLinearConfig.polling_rate_hz: u32` (0 = RA-default min only).
- `trainer_default()` → `1000`; `phase1_sensitivity` → `0` (Guide vectors use dt=1 ms anyway).
- Processor version → `1.2.0`; experiment → `0.5.1`.
- `config_json` includes `polling_rate_hz`.

---

## 4. HUD

While `rawaccel_linear` (Idle or Running):

- `DT_MS: raw … → speed …` (flag if clamped)
- `INPUT SPEED: … counts/ms`
- `ACCEL SCALE: …`
- Note: EXPLICIT poll-period floor for user-mode timestamps; not Device-DPI.

---

## 5. Out of scope

- Device DPI factor  
- Kernel-faithful `num_packets` split  
- Changing Linear `acceleration` defaults  
- EMA / smoothing  

---

## 6. Acceptance

- Unit: tiny positive `dt` with `polling_rate_hz=1000` clamps to 1 ms and lowers scale vs unclamped.
- Guide `(30,40)` @ 1 ms still passes.
- Workspace tests green.
- Manual: Linear-on feel closer to official; HUD shows fewer sub-1 ms speed dts when moving.

**Manual result (2026-09-17):** Pass — feel noticeably similar to official vs prior much-hotter trainer; slight residual differences accepted as human/measurement noise, not a reopen of classic scale.

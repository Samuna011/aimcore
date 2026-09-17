# M3.1 — STATIC_CLICK Multi-Target Front Cone — Design Spec

**Date:** 2026-09-17  
**Status:** Approved (implement per user request)  
**Extends:** `2026-09-17-m3-static-click-design.md`

## Behavior

- One **run** = **5 hits** required.
- Each target is a sphere at **random pose inside a front cone** (not full sphere / behind camera).
- **Hit:** destroy current target; if hits &lt; 5, spawn next random; if hits == 5, end run and **score = elapsed seconds** (start → 5th hit).
- **Miss:** keep the **same** target; no respawn.
- Start run: reset look to 0,0; start timer; hits = 0; spawn first target.

## Front cone (defaults)

| Param | Value |
|-------|--------|
| Distance `D` | 10 world units from camera origin |
| Yaw half-range | ±25° from identity forward (−Z) |
| Pitch half-range | ±12° |
| Radius `R` | 0.25 |
| Hits to finish | 5 |

Position = `camera_origin + look_dir(yaw_off, pitch_off) * D` with uniform random offsets in ranges above.

## Score

`score_secs = (t_fifth_hit_ns - t_start_ns) / 1e9` — lower is better. HUD shows hits `k/5`, last HIT/MISS, and final time.

## Out of scope

Flick/tracking; miss penalties beyond time; DB aim table; changing ray–sphere / raw LMB rules.

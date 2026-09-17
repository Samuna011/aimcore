# Research — Raw Accel DPI / Poll Rate Fields vs Live Feel

**Date:** 2026-09-17  
**Status:** Research findings (no implementation) — Charts-only DPI/poll **CONFIRMED** by user 2026-09-17  
**Sources:** Raw Accel v1.7.1 tree (`RawAccelOfficial/rawaccel` master), Guide.md, driver + grapher source  
**Trigger:** Live A/B — trainer Linear felt hotter than official with same curve numbers; Charts DPI 3200 + poll 1000 only; Devices DPI/poll unset; RA disabled on trainer side.

---

## 1. Verdict (short)

Raw Accel exposes **two different DPI/poll controls**. They are easy to conflate:

| Control | Where in GUI | Can be 0? | Affects driver / live feel? |
|---------|--------------|-----------|------------------------------|
| **Charts → Scale by Mouse Settings → DPI / Poll rate** | Charts menu | **DPI min = 1** (cannot be 0) | **No** — chart window / Last Mouse Move display only |
| **Advanced → Devices → DPI / Polling rate** | Device menu | **Yes** (default 0) | **Yes, when DPI > 0 or poll rate > 0** |

Community language often says “RA DPI is just for the graph.” That is **true for Charts scale**, **false for Device DPI**.

Earlier assistant advice that treated “set DPI to 0” as the fix for the Charts-style field was **wrong** for that field.

---

## 2. Charts: “Scale by Mouse Settings”

### GUI

- Menu text: **“Scale by Mouse Settings”** (designer: `scaleByDPIToolStripMenuItem`)
- Subfields: DPI textbox, Poll rate textbox
- Construction (`AccelGUIFactory.cs`):

```csharp
new Field(dpiTextBox.TextBox, form, Constants.DefaultChartsScalingDPI, 1),
new Field(pollRateTextBox.TextBox, form, Constants.DefaultChartsScalingPollRate, 1));
```

- Fourth argument is `minData` → **Charts DPI / poll cannot be below 1** (matches user: “can’t be 0”).
- Defaults (`Constants.cs`): DPI **1200**, Poll **1000**.

### Behavior

- Guide (§ Charts >> Scale by DPI and Poll Rate):

  > These options **does not scale your acceleration curve in any way**. Rather, DPI scales the set of points used to graph your curve…

- `AccelCalculator` uses Charts DPI mainly as:

  `MaxVelocity = DPI.Data * Constants.MaxMultiplier`  
  plus fixed `MeasurementTime = 1` for simulated chart inputs.

- Values persist in **`.config` / `GUISettings`**, not as the device driver’s DPI factor.

### Naming

- UI does **not** call this “normalization.”
- It asks for **actual mouse DPI / poll** so the **graph scale** matches the user’s hardware window.
- User description (“actual dpi settings for scale”, “can’t be 0”) matches **this** control, not Device DPI.

**Confidence:** **CONFIRMED** (Guide + factory min=1 + calculator).

---

## 3. Devices: DPI / Polling rate (driver-affecting)

### GUI

- Form: `DeviceMenuForm` — labels **“DPI:”** and **“Polling rate:”**
- `NumericUpDown` for DPI: `Minimum = 0`, `Maximum = 999999` → **0 is allowed**
- Tooltips (source, not always read by users):
  - DPI: **“Normalizes sensitivity and input speed to 1000 DPI”**
  - Poll: **“Keep at 0 for automatic adjustment”**

### Guide

- Section titled **“DPI Normalization”** (docs word — **not** the on-screen label).
- Setting device DPI to the mouse’s actual DPI scales input so sens/accel feel as if at **1000 DPI**; converts speed domain toward **in/s**-like units via the 1000 DPI reference.
- Poll rate in Device menu: leave **0** unless stutter; non-zero changes how the driver derives/clamps time.

### Driver math (`driver.cpp` + `rawaccel.hpp`)

```text
input_dpi_normalization_factor = (cfg.dpi > 0) ? (NORMALIZED_DPI / cfg.dpi) : 1
NORMALIZED_DPI = 1000
```

Then in `modifier::modify`:

```text
ips_factor = dpi_factor / time
abs_weighted_vel = |in| * ips_factor * domain_weights   // curve input speed
…
out *= (output_dpi / NORMALIZED_DPI) * dpi_factor       // post-scale (sens path)
```

If **device DPI = 3200**:

- `dpi_factor = 1000/3200 = 0.3125`
- Curve sees **lower** input speed than raw counts/ms
- Output counts also × **0.3125** (with sens multiplier 1 and `output_dpi = 1000`)

If **device polling rate = 1000** (and time lock not forcing fixed interval):

- `clamp.min = 1000 / polling_rate = 1.0 ms`
- Measured packet intervals below 1 ms are clamped up → **lower** speed estimates vs unclamped QPC deltas

JSON key text (historical settings examples) still literally says normalization; GUI label is only **“DPI:”**.

**Confidence:** **CONFIRMED** (driver + DeviceMenuForm + Guide).

---

## 4. Implications for the feel mismatch

### Locked (user 2026-09-17)

- DPI 3200 + poll 1000 are **Charts → Scale by Mouse Settings only**.
- **Devices** DPI / polling rate are **not** set → driver `dpi_factor = 1`; Device poll clamp override **not** applied from user config.
- Charts values **do not** change official live accel vs our trainer.
- Hypothesis “DPI 3200 → official ×0.3125 slower” is **REJECTED** for this A/B.

### Still open (ranked suspects after Charts-only lock)

| # | Suspect | Why it can still make trainer feel hotter | Confidence |
|---|---------|---------------------------------------------|------------|
| 1 | ~~Sens / FOV / accel-off baseline mismatch~~ | — | **REJECTED** (user 2026-09-17: accel off matches RA vs trainer) |
| 2 | **Timebase / `dt_ms` path** (see §8) | Inflated `input_speed` → hotter Linear while identity path stays matched | **PRIMARY OPEN** |
| 3 | Classic Linear scale bug in our port | Unlikely if Guide/Gain unit tests pass | **LOW** until identical `(dx,dy,dt)` A/B |

### Rejected for this A/B

| Hypothesis | Status |
|------------|--------|
| Device DPI normalization at 3200 | **REJECTED** (Devices unset) |
| Charts DPI/poll changing live curve | **REJECTED** (Guide + source) |
| Double accel (RA on while trainer runs) | **REJECTED** (user: RA disabled on trainer side) |
| Sens / FOV / baseline mismatch | **REJECTED** (user: accel off feels the same) |

---

## 8. Timebase check (#2) — research

### 8.1 Official RA (Devices poll = 0)

```text
raw_time_ms = (QPC_delta_ticks * tick_interval_ms) / num_packets
time_ms     = clamp(raw_time_ms, clamp.min, clamp.max)
```

Defaults (`rawaccel-base.hpp`):

- `DEFAULT_TIME_MIN = 1000 / 8000 / 2 = 0.0625 ms`
- `DEFAULT_TIME_MAX = 100 ms`

Then `speed = magnitude(counts) * (dpi_factor / time_ms)` with `dpi_factor = 1`.

Same formula shape as ours: **counts / ms**. Multi-packet callbacks **split** the interval equally across packets.

Stamping happens in the **kernel filter** at packet receive (`KeQueryPerformanceCounter`).

### 8.2 Trainer

```text
timestamp_ns = QueryPerformanceCounter at WM_INPUT handle time (user mode)
dt_s         = (t_n - t_{n-1}) / 1e9     // first sample → 0 → identity bypass
dt_ms        = dt_s * 1000
v            = hypot(dx, dy) / dt_ms     // when dt_ms > 0
```

No RA-style min/max time clamp. No `num_packets` split (each `WM_INPUT` is its own sample).

### 8.3 Does DEFAULT_TIME_MIN alone explain “much hotter”?

**Probably not as the main effect at 1000 Hz steady motion.**

- Typical poll interval ≈ **1 ms**, far above **0.0625 ms**.
- Min clamp only raises extremely short `raw` times → lowers speed → makes official **cooler** than a path with tinier dts.
- So min clamp matters only if **our** `dt_ms` often drops well below ~0.0625–0.5 ms while counts stay poll-sized.

### 8.4 Stronger timebase mechanism (still #2)

| Mechanism | Effect |
|-----------|--------|
| **User-mode stamp vs kernel stamp** | We stamp when the app handles `WM_INPUT`, not when the mouse filter saw the packet. Message-pump / scheduling can **compress** consecutive handle times. |
| **Burst compression** | Several packets handled back-to-back → `dt_ms` ≪ 1 ms with full per-packet counts → huge `v` → Linear scale ≫ official. Accel-**off** still matches (identity). Accel-**on** feels hotter. |
| **No `num_packets` equalization** | RA spreads one interrupt’s wall time across packets; we never do that equalization. |

This fits the user’s report: **baseline (accel off) same**, **Linear on hotter on trainer**.

### 8.5 What would falsify / confirm

Live trainer stats while moving at steady 1 kHz-ish speed:

- `dt_ms` p50 near **1.0** → timebase less likely
- Many samples with `dt_ms` ≪ **0.5** (or ≪ **0.1**) and non-trivial `|dx,dy|` → timebase **CONFIRMED** as primary suspect
- Compare `input_speed` to RA chart “Last mouse move” at similar hand motion (Charts poll=1000 is display-only but LMM points use real driver output timing)

**Status after this check:** Timebase remains the **best open hypothesis**; mechanism refined from “default min clamp” → **user-mode / burst `dt_ms` inflation**. Not yet empirically confirmed in-app — needs `dt_ms` / `input_speed` instrumentation.

---

## 9. Next gate

1. Add short-lived live debug (HUD or log): last/`p50`/`min` `dt_ms`, `input_speed`, `acceleration_scale` under `rawaccel_linear`.
2. If short-`dt` confirmed → design: RA-like time clamp and/or packet-batch time split (parity with driver), not an `acceleration` fudge.
3. If `dt_ms` healthy (~1 ms) → reopen classic scale / harness A/B on fixed vectors.

---

## 5. Corrected guidance (research-only)

1. Do **not** tell users to set Charts DPI to 0 — the Charts field **rejects 0** and is for **graph scale only**.
2. Do **not** call Charts DPI “normalization”; reserve that word for **Device DPI** (Guide title + tooltip + driver).
3. For **this** A/B (Charts-only, Devices unset): ignore DPI/poll as the feel cause; next evidence is accel-off sens match, then `dt_ms` / time-clamp instrumentation.

---

## 6. Provenance

| Claim | Source | Label |
|-------|--------|-------|
| Charts scale does not change curve | Guide.md “Scale by DPI and Poll Rate” | **CONFIRMED** |
| Charts DPI min = 1 | `AccelGUIFactory` `new Field(..., 1)` | **CONFIRMED** |
| Charts defaults 1200 / 1000 | `Constants.DefaultChartsScalingDPI/PollRate` | **CONFIRMED** |
| Device DPI label plain “DPI:”; tip mentions 1000 DPI | `DeviceMenuForm.cs` | **CONFIRMED** |
| Device DPI 0 → factor 1; else 1000/dpi | `driver.cpp` `DeviceSetup` | **CONFIRMED** |
| `NORMALIZED_DPI = 1000` | `rawaccel-base.hpp` | **CONFIRMED** |
| Poll rate in Devices sets `clamp.min = 1000/rate` when rate given | `driver.cpp` | **CONFIRMED** |
| User’s 3200/1000 are Charts-only; Devices unset | User 2026-09-17 | **CONFIRMED** |
| DPI-norm explains this A/B hotter feel | — | **REJECTED** |
| Accel-off baseline matches | User 2026-09-17 | **CONFIRMED** |
| Approach C time clamp + HUD | `2026-09-17-trainer-time-clamp-design.md` | **IMPLEMENTED** |
| Linear-on feel vs official after poll-period floor | User A/B 2026-09-17 | **EMPIRICALLY IMPROVED** — noticeably similar; residual slight diffs attributed to human error / remaining unmodeled pipeline gaps |

---

## 7. Next research / design gate

Before changing `sense-accel` code:

1. ~~**Accel off A/B**~~ — **DONE / REJECTED as cause** (user: matches).
2. **Instrument trainer `dt_ms` / `input_speed`** during Linear-on motion (§8.5).
3. Only then design time-clamp / batch-time parity — not a blind `acceleration` tweak.

use crate::geometry::{
    camera_origin, endpoint_error_yaw_pitch, overshoot_yaw_pitch, path_length_yaw_pitch,
    target_dir, yaw_pitch_from_dir,
};
use crate::reconstruct::ReconstructedTrial;
use crate::segment::MovementCandidate;
use crate::AnalysisConfig;
use sense_types::AimShotRecord;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BehaviorMetrics {
    pub endpoint_error_deg: f64,
    pub overshoot_deg: f64,
    /// Candidate start → this shot timestamp (not full candidate end).
    pub movement_duration_ns: u64,
    pub path_length_deg: f64,
    pub path_efficiency: Option<f64>,
    pub angular_velocity_mean_deg_s: f64,
    pub angular_velocity_peak_deg_s: f64,
    pub angular_acceleration_peak_deg_s2: f64,
    pub jitter_rms_deg_s: f64,
    pub correction_magnitude_deg: Option<f64>,
    pub correction_duration_ns: Option<u64>,
}

pub fn behavior_for_shot(
    recon: &ReconstructedTrial,
    cand: &MovementCandidate,
    shot: &AimShotRecord,
    correction_start_ns: Option<u64>,
    correction_end_ns: Option<u64>,
    config: &AnalysisConfig,
) -> Option<BehaviorMetrics> {
    // Local acquisition window: last onset before this shot inside the candidate
    // (not the multi-shot candidate's global start — that caused ~±720° false errors).
    let acq_start = local_acquisition_start_ns(recon, cand, shot.timestamp_ns, config);
    let primary_end = shot.timestamp_ns;
    let cam: Vec<_> = recon
        .camera
        .iter()
        .filter(|p| p.timestamp_ns >= acq_start && p.timestamp_ns <= primary_end)
        .collect();
    if cam.is_empty() {
        return None;
    }

    let start = cam.first().unwrap();
    let target = target_dir(
        camera_origin(),
        [shot.target_x, shot.target_y, shot.target_z],
    );
    let (target_yaw_w, target_pitch) = yaw_pitch_from_dir(target);

    // Click pose on the continuous branch nearest the acquisition path.
    let click_yaw = {
        let last = cam.last().unwrap().unwrapped_yaw_deg;
        last + crate::geometry::wrap_to_180(shot.yaw_deg - crate::geometry::wrap_to_180(last))
    };
    let click_pitch = shot.pitch_deg;

    let endpoint_error = endpoint_error_yaw_pitch(
        start.unwrapped_yaw_deg,
        start.pitch_deg,
        target_yaw_w,
        target_pitch,
        click_yaw,
        click_pitch,
    );

    let path_yaw: Vec<f64> = cam.iter().map(|p| p.unwrapped_yaw_deg).collect();
    let path_pitch: Vec<f64> = cam.iter().map(|p| p.pitch_deg).collect();
    let pre_n = cam
        .iter()
        .take_while(|p| p.timestamp_ns < shot.timestamp_ns)
        .count();
    let overshoot = overshoot_yaw_pitch(
        start.unwrapped_yaw_deg,
        start.pitch_deg,
        target_yaw_w,
        target_pitch,
        &path_yaw[..pre_n],
        &path_pitch[..pre_n],
    );

    let path_len = path_length_yaw_pitch(&path_yaw, &path_pitch);
    let chord = {
        let dy = click_yaw - start.unwrapped_yaw_deg;
        let dp = click_pitch - start.pitch_deg;
        (dy * dy + dp * dp).sqrt()
    };
    let path_efficiency = if path_len > 1e-9 {
        Some((chord / path_len).clamp(0.0, 1.0))
    } else {
        None
    };

    let speeds: Vec<f64> = cam.iter().map(|p| p.angular_speed_deg_s).collect();
    let mean_v = if speeds.is_empty() {
        0.0
    } else {
        speeds.iter().sum::<f64>() / speeds.len() as f64
    };
    let peak_v = speeds.iter().cloned().fold(0.0_f64, f64::max);

    let mut peak_acc = 0.0_f64;
    for w in cam.windows(2) {
        let dt_s = (w[1].timestamp_ns.saturating_sub(w[0].timestamp_ns)) as f64 / 1e9;
        if dt_s > 0.0 {
            let a = ((w[1].angular_speed_deg_s - w[0].angular_speed_deg_s) / dt_s).abs();
            if a > peak_acc {
                peak_acc = a;
            }
        }
    }

    let jitter = jitter_rms(&speeds, cam.as_slice(), config.jitter_window_ms);

    let (corr_mag, corr_dur) = match (correction_start_ns, correction_end_ns) {
        (Some(a), Some(b)) if b > a => {
            let corr: Vec<_> = recon
                .camera
                .iter()
                .filter(|p| p.timestamp_ns >= a && p.timestamp_ns <= b)
                .collect();
            let cy: Vec<f64> = corr.iter().map(|p| p.unwrapped_yaw_deg).collect();
            let cp: Vec<f64> = corr.iter().map(|p| p.pitch_deg).collect();
            (
                Some(path_length_yaw_pitch(&cy, &cp)),
                Some(b.saturating_sub(a)),
            )
        }
        _ => (None, None),
    };

    Some(BehaviorMetrics {
        endpoint_error_deg: endpoint_error,
        overshoot_deg: overshoot,
        // Explicit: local acquisition start → this shot (not full multi-shot candidate end).
        movement_duration_ns: primary_end.saturating_sub(acq_start),
        path_length_deg: path_len,
        path_efficiency,
        angular_velocity_mean_deg_s: mean_v,
        angular_velocity_peak_deg_s: peak_v,
        angular_acceleration_peak_deg_s2: peak_acc,
        jitter_rms_deg_s: jitter,
        correction_magnitude_deg: corr_mag,
        correction_duration_ns: corr_dur,
    })
}

/// Latest onset inside `[cand.start, shot]` after a completed settle, else `cand.start`.
///
/// Angular speed on sample `i` is the speed of segment `(i-1 → i)`. So when the first
/// sample with speed ≥ onset is the click sample itself, acquisition starts at the
/// **previous** sample (segment start). That prevents `movement_duration_ns == 0` for
/// flicks that end on the click.
pub(crate) fn local_acquisition_start_ns(
    recon: &ReconstructedTrial,
    cand: &MovementCandidate,
    shot_ns: u64,
    config: &AnalysisConfig,
) -> u64 {
    let settle_ns = config.settle_ms.saturating_mul(1_000_000);
    let cams: Vec<_> = recon
        .camera
        .iter()
        .filter(|p| p.timestamp_ns >= cand.start_ns && p.timestamp_ns <= shot_ns)
        .collect();
    if cams.is_empty() {
        return cand.start_ns;
    }

    let segment_start_for_onset = |onset_idx: usize| -> u64 {
        if onset_idx > 0 {
            cams[onset_idx - 1].timestamp_ns
        } else {
            cams[onset_idx].timestamp_ns
        }
    };

    // Walk backward from the click: find a completed settle, then the following onset.
    let mut i = cams.len() - 1;
    while i > 0 {
        while i > 0 && cams[i].angular_speed_deg_s >= config.settle_deg_per_s {
            i -= 1;
        }
        if i == 0 {
            break;
        }
        let settle_end_i = i;
        let mut settle_start_t = cams[i].timestamp_ns;
        while i > 0 && cams[i].angular_speed_deg_s < config.settle_deg_per_s {
            settle_start_t = cams[i].timestamp_ns;
            i -= 1;
        }
        let settle_end_t = cams[settle_end_i].timestamp_ns;
        if settle_end_t.saturating_sub(settle_start_t) >= settle_ns {
            for (k, p) in cams.iter().enumerate().skip(settle_end_i + 1) {
                if p.angular_speed_deg_s >= config.onset_deg_per_s {
                    return segment_start_for_onset(k);
                }
            }
            // Settled through the click: no flick segment — use settle end (may be short).
            return cams[settle_end_i].timestamp_ns.min(shot_ns);
        }
        if i == 0 {
            break;
        }
    }

    // No completed settle before shot (e.g. first movement of a candidate): start of
    // contiguous active bout ending at the shot, else candidate start.
    let mut j = cams.len() - 1;
    while j > 0 && cams[j].angular_speed_deg_s >= config.settle_deg_per_s {
        j -= 1;
    }
    // j is last settled sample (or 0). If there is an active bout after j, its first
    // high-speed sample's *segment* starts at the previous sample.
    if let Some((k, _)) = cams
        .iter()
        .enumerate()
        .skip(j)
        .find(|(_, p)| p.angular_speed_deg_s >= config.onset_deg_per_s)
    {
        return segment_start_for_onset(k).max(cand.start_ns);
    }
    cand.start_ns
}

fn jitter_rms(
    speeds: &[f64],
    cam: &[&crate::reconstruct::CameraPoint],
    window_ms: u64,
) -> f64 {
    if speeds.len() < 2 {
        return 0.0;
    }
    let window_ns = window_ms.saturating_mul(1_000_000);
    let mut residuals = Vec::new();
    for (i, p) in cam.iter().enumerate() {
        let t0 = p.timestamp_ns.saturating_sub(window_ns / 2);
        let t1 = p.timestamp_ns.saturating_add(window_ns / 2);
        let mut sum = 0.0;
        let mut n = 0usize;
        for (j, q) in cam.iter().enumerate() {
            if q.timestamp_ns >= t0 && q.timestamp_ns <= t1 {
                sum += speeds[j];
                n += 1;
            }
        }
        if n > 0 {
            let local_mean = sum / n as f64;
            residuals.push(speeds[i] - local_mean);
        }
    }
    if residuals.is_empty() {
        return 0.0;
    }
    let mean_sq = residuals.iter().map(|r| r * r).sum::<f64>() / residuals.len() as f64;
    mean_sq.sqrt()
}

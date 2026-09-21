use crate::quality::QualityFlag;
use crate::reconstruct::ReconstructedTrial;
use crate::AnalysisConfig;
use sense_types::AimShotRecord;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MovementCandidate {
    pub id: u32,
    pub start_ns: u64,
    pub end_ns: u64,
    pub flags: Vec<QualityFlag>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShotLink {
    pub shot_index: u32,
    pub primary_movement_id: Option<u32>,
    pub correction_start_ns: Option<u64>,
    pub correction_end_ns: Option<u64>,
    pub flags: Vec<QualityFlag>,
}

pub fn segment(
    recon: &ReconstructedTrial,
    shots: &[AimShotRecord],
    config: &AnalysisConfig,
) -> (Vec<MovementCandidate>, Vec<ShotLink>) {
    let mut candidates = detect_candidates(recon, config);
    flag_multi_shot(&mut candidates, shots);

    let mut links = Vec::with_capacity(shots.len());
    for shot in shots {
        let (primary, mut flags) = attach_shot(&candidates, shot.timestamp_ns, config);
        let (c_start, c_end) = if primary.is_some() {
            correction_window(recon, &candidates, shot.timestamp_ns, config)
        } else {
            (None, None)
        };
        if primary.is_none() && !flags.contains(&QualityFlag::OrphanShot) {
            flags.push(QualityFlag::OrphanShot);
        }
        links.push(ShotLink {
            shot_index: shot.shot_index,
            primary_movement_id: primary,
            correction_start_ns: c_start,
            correction_end_ns: c_end,
            flags,
        });
    }

    (candidates, links)
}

fn detect_candidates(recon: &ReconstructedTrial, config: &AnalysisConfig) -> Vec<MovementCandidate> {
    let cam = &recon.camera;
    if cam.is_empty() {
        return Vec::new();
    }

    let settle_ns = config.settle_ms.saturating_mul(1_000_000);
    let min_ns = config.min_movement_ms.saturating_mul(1_000_000);

    let mut out = Vec::new();
    let mut id = 0u32;
    let mut i = 0usize;
    while i < cam.len() {
        if cam[i].angular_speed_deg_s < config.onset_deg_per_s {
            i += 1;
            continue;
        }
        let start_ns = cam[i].timestamp_ns;
        let start_i = i;
        let mut settle_start: Option<u64> = None;
        let mut end_i = i;
        let mut j = i + 1;
        let mut no_settle = true;
        while j < cam.len() {
            end_i = j;
            let speed = cam[j].angular_speed_deg_s;
            if speed < config.settle_deg_per_s {
                let t = cam[j].timestamp_ns;
                match settle_start {
                    None => settle_start = Some(t),
                    Some(s0) => {
                        if t.saturating_sub(s0) >= settle_ns {
                            // End at first timestamp where continuous settle holds.
                            no_settle = false;
                            break;
                        }
                    }
                }
            } else {
                settle_start = None;
            }
            j += 1;
        }

        let end_ns = cam[end_i].timestamp_ns;
        let dur = end_ns.saturating_sub(start_ns);
        if dur >= min_ns {
            let mut flags = Vec::new();
            if no_settle {
                flags.push(QualityFlag::NoSettle);
            }
            out.push(MovementCandidate {
                id,
                start_ns,
                end_ns,
                flags,
            });
            id += 1;
        }
        i = end_i.max(start_i) + 1;
    }
    out
}

fn flag_multi_shot(candidates: &mut [MovementCandidate], shots: &[AimShotRecord]) {
    for c in candidates.iter_mut() {
        let count = shots
            .iter()
            .filter(|s| s.timestamp_ns >= c.start_ns && s.timestamp_ns <= c.end_ns)
            .count();
        if count > 1 {
            c.flags.push(QualityFlag::MultiShotMovement);
        }
    }
}

fn attach_shot(
    candidates: &[MovementCandidate],
    click_ns: u64,
    config: &AnalysisConfig,
) -> (Option<u32>, Vec<QualityFlag>) {
    let attach_ns = config.shot_attach_ms.saturating_mul(1_000_000);
    let containing: Vec<_> = candidates
        .iter()
        .filter(|c| click_ns >= c.start_ns && click_ns <= c.end_ns)
        .collect();
    if containing.len() == 1 {
        return (Some(containing[0].id), Vec::new());
    }
    if containing.len() > 1 {
        return (None, vec![QualityFlag::AmbiguousShot]);
    }

    let mut preceding: Vec<_> = candidates
        .iter()
        .filter(|c| c.end_ns <= click_ns && click_ns.saturating_sub(c.end_ns) <= attach_ns)
        .collect();
    preceding.sort_by_key(|c| click_ns.saturating_sub(c.end_ns));
    if preceding.is_empty() {
        return (None, vec![QualityFlag::OrphanShot]);
    }
    // If two share the same gap, ambiguous.
    if preceding.len() >= 2 {
        let g0 = click_ns.saturating_sub(preceding[0].end_ns);
        let g1 = click_ns.saturating_sub(preceding[1].end_ns);
        if g0 == g1 {
            return (None, vec![QualityFlag::AmbiguousShot]);
        }
    }
    (Some(preceding[0].id), Vec::new())
}

fn correction_window(
    recon: &ReconstructedTrial,
    candidates: &[MovementCandidate],
    click_ns: u64,
    config: &AnalysisConfig,
) -> (Option<u64>, Option<u64>) {
    let max_end = click_ns.saturating_add(config.post_click_max_ms.saturating_mul(1_000_000));
    let next_onset = candidates
        .iter()
        .filter(|c| c.start_ns > click_ns)
        .map(|c| c.start_ns)
        .min();

    let settle_ns = config.settle_ms.saturating_mul(1_000_000);
    let mut settle_start: Option<u64> = None;
    let mut settle_end: Option<u64> = None;
    for p in recon.camera.iter().filter(|p| p.timestamp_ns > click_ns) {
        if p.timestamp_ns > max_end {
            break;
        }
        if let Some(no) = next_onset {
            if p.timestamp_ns >= no {
                break;
            }
        }
        if p.angular_speed_deg_s < config.settle_deg_per_s {
            match settle_start {
                None => settle_start = Some(p.timestamp_ns),
                Some(s0) => {
                    if p.timestamp_ns.saturating_sub(s0) >= settle_ns {
                        settle_end = Some(p.timestamp_ns);
                        break;
                    }
                }
            }
        } else {
            settle_start = None;
        }
    }

    let end = settle_end
        .into_iter()
        .chain(next_onset.into_iter())
        .chain(std::iter::once(max_end))
        .min()
        .unwrap_or(max_end);

    if end <= click_ns {
        return (None, None);
    }
    (Some(click_ns), Some(end))
}

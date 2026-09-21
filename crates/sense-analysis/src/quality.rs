use crate::exposure::expected_scale_for_sample;
use crate::reconstruct::ReconstructedTrial;
use crate::AnalysisConfig;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QualityFlag {
    OrphanShot,
    AmbiguousShot,
    MultiShotMovement,
    NoSettle,
    InsufficientTelemetry,
    TimestampGap,
    CameraInputMismatch,
    ReconstructionDiscontinuity,
    ZeroAcquisitionDuration,
}

pub fn scan_timestamp_gaps(
    recon: &ReconstructedTrial,
    config: &AnalysisConfig,
) -> Vec<QualityFlag> {
    let gap_ns = config.gap_ms.saturating_mul(1_000_000);
    let mut flags = Vec::new();
    for w in recon.camera.windows(2) {
        if w[1].timestamp_ns.saturating_sub(w[0].timestamp_ns) > gap_ns {
            flags.push(QualityFlag::TimestampGap);
            break;
        }
    }
    if flags.is_empty() {
        for w in recon.inputs.windows(2) {
            if w[1].timestamp_ns.saturating_sub(w[0].timestamp_ns) > gap_ns {
                flags.push(QualityFlag::TimestampGap);
                break;
            }
        }
    }
    flags
}

pub fn scale_mismatch(
    recon: &ReconstructedTrial,
    start_ns: u64,
    end_ns: u64,
    processor_id: &str,
    processor_config_json: &str,
    eps: f64,
) -> bool {
    for s in recon
        .inputs
        .iter()
        .filter(|s| s.timestamp_ns >= start_ns && s.timestamp_ns <= end_ns)
    {
        let Some(stored) = s.acceleration_scale else {
            continue;
        };
        let Some(expected) = expected_scale_for_sample(
            processor_id,
            processor_config_json,
            s.raw_dx,
            s.raw_dy,
            s.dt_used_ns,
        ) else {
            continue;
        };
        if (stored - expected).abs() > eps {
            return true;
        }
    }
    false
}

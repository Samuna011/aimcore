//! Deterministic aim-trial analysis (M4.1). Persisted telemetry only.

pub mod behavior;
pub mod compare;
pub mod demand;
pub mod exposure;
pub mod geometry;
pub mod loader;
pub mod output;
pub mod quality;
pub mod reconstruct;
pub mod scope;
pub mod segment;

pub use behavior::BehaviorMetrics;
pub use compare::{
    compare_trials, compare_trials_with_demand, ComparedMetric, ComparisonValidity,
    ConditionComparison, ConditionMismatch, ConditionSnapshot, DemandCompareConfig,
    DemandStratum, DemandStratumComparison, MatchStatus, ProcessorDiff, QualitySummary,
    COMPARISON_VERSION,
};
pub use demand::{MovementDemand, ProcessorExposure};
pub use exposure::ExposureMetrics;
pub use loader::{
    compare_trial_ids, find_comparison_candidates, load_and_analyze, CandidateCriteria,
    ProcessorFilter,
};
pub use output::analysis_result_to_json;
pub use quality::QualityFlag;
pub use reconstruct::{CameraPoint, InputPoint, ReconstructedTrial, ReconstructionStats};
pub use scope::{metric_scope_for_trial_type, Applicability, MetricScope};
pub use segment::{MovementCandidate, ShotLink};

use sense_telemetry::AimTrialAnalysisBundle;
use serde::Serialize;

pub const ANALYSIS_VERSION: &str = "2";

/// Aim-lab camera origin matching `sense-maxer` STATIC_CLICK / GRIDSHOT.
pub const ANALYSIS_CAMERA_ORIGIN: [f64; 3] = [0.0, 1.6, 4.0];

#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisConfig {
    pub analysis_version: String,
    pub onset_deg_per_s: f64,
    pub settle_deg_per_s: f64,
    pub settle_ms: u64,
    pub min_movement_ms: u64,
    pub shot_attach_ms: u64,
    pub post_click_max_ms: u64,
    pub gap_ms: u64,
    pub jitter_window_ms: u64,
    pub scale_mismatch_eps: f64,
    /// Inter-sample dt below this is not used for °/s (avoids microsecond spikes).
    pub min_dt_ns_for_speed: u64,
    /// Single-step √(Δyaw²+Δpitch²) above this counts as discontinuity.
    pub max_step_deg: f64,
    /// Peak reconstructed angular speed (informational). Large values from real
    /// 180° flicks are valid BehaviorMetrics — not automatically a quality failure.
    pub max_angular_speed_deg_s: f64,
}

impl AnalysisConfig {
    pub fn v1() -> Self {
        Self {
            analysis_version: ANALYSIS_VERSION.into(),
            onset_deg_per_s: 40.0,
            settle_deg_per_s: 15.0,
            settle_ms: 80,
            min_movement_ms: 40,
            shot_attach_ms: 150,
            post_click_max_ms: 300,
            gap_ms: 50,
            jitter_window_ms: 40,
            scale_mismatch_eps: 1e-3,
            min_dt_ns_for_speed: 1_000_000, // 1 ms — floor for °/s (poll-scale)
            max_step_deg: 90.0,
            max_angular_speed_deg_s: 5_000.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnalysisError {
    UnsupportedTrialType(String),
    InsufficientTelemetry(String),
    ReconstructionFailed(String),
    Loader(String),
}

impl std::fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedTrialType(t) => write!(f, "unsupported trial type: {t}"),
            Self::InsufficientTelemetry(m) => write!(f, "insufficient telemetry: {m}"),
            Self::ReconstructionFailed(m) => write!(f, "reconstruction failed: {m}"),
            Self::Loader(m) => write!(f, "loader: {m}"),
        }
    }
}

impl std::error::Error for AnalysisError {}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ShotAnalysis {
    pub shot_index: u32,
    pub hit: bool,
    pub primary_movement_id: Option<u32>,
    pub correction_start_ns: Option<u64>,
    pub correction_end_ns: Option<u64>,
    pub behavior: Option<BehaviorMetrics>,
    pub exposure: Option<ExposureMetrics>,
    /// Config-independent movement demand (M4.3).
    pub movement_demand: Option<MovementDemand>,
    /// Processor exposure on the acquisition window (not a demand dimension).
    pub processor_exposure: Option<ProcessorExposure>,
    pub quality_flags: Vec<QualityFlag>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnalysisResult {
    pub analysis_version: String,
    pub trial_id: String,
    pub trial_type: String,
    pub experiment_version: String,
    pub task_version: String,
    pub processor_id: String,
    pub processor_version: String,
    pub metric_scope: MetricScope,
    pub candidates: Vec<MovementCandidate>,
    pub shots: Vec<ShotAnalysis>,
    pub trial_quality_flags: Vec<QualityFlag>,
    pub reconstruction_max_abs_step_deg: f64,
    pub reconstruction_max_angular_speed_deg_s: f64,
    pub reconstruction_discontinuity_count: u32,
    pub reconstruction_yaw_wrap_crossings: u32,
}

pub fn analyze_trial(
    bundle: &AimTrialAnalysisBundle,
    config: &AnalysisConfig,
) -> Result<AnalysisResult, AnalysisError> {
    let trial_type = bundle.trial.trial_type.as_str();
    if trial_type != "STATIC_CLICK"
        && trial_type != "GRIDSHOT"
        && trial_type != "TRACKING"
        && trial_type != "FLICK_LADDER"
        && trial_type != "ONE_WALL_SIX"
        && trial_type != "FLICK_DEMAND"
    {
        return Err(AnalysisError::UnsupportedTrialType(
            bundle.trial.trial_type.clone(),
        ));
    }

    if trial_type == "TRACKING" && bundle.shots.is_empty() {
        return Err(AnalysisError::InsufficientTelemetry(
            "TRACKING analysis requires aim_shots (task_version 2+ rapid-fire)".into(),
        ));
    }

    if bundle.camera_samples.is_empty() {
        return Err(AnalysisError::InsufficientTelemetry(
            "no camera samples".into(),
        ));
    }

    let recon = reconstruct::reconstruct(bundle, config)?;
    let mut trial_flags = quality::scan_timestamp_gaps(&recon, config);
    if recon.stats.discontinuity_count > 0 {
        trial_flags.push(QualityFlag::ReconstructionDiscontinuity);
    }

    let (candidates, links) = segment::segment(&recon, &bundle.shots, config);

    let mut shots_out = Vec::with_capacity(links.len());
    for link in &links {
        let shot = bundle
            .shots
            .iter()
            .find(|s| s.shot_index == link.shot_index)
            .expect("shot link must reference existing shot");

        let mut flags = link.flags.clone();
        if candidates.iter().any(|c| {
            c.id == link.primary_movement_id.unwrap_or(u32::MAX)
                && c.flags.contains(&QualityFlag::MultiShotMovement)
        }) {
            if !flags.contains(&QualityFlag::MultiShotMovement) {
                flags.push(QualityFlag::MultiShotMovement);
            }
        }

        let (behavior, exposure, movement_demand, processor_exposure) =
            if let Some(mid) = link.primary_movement_id {
                let cand = candidates.iter().find(|c| c.id == mid).expect("candidate");
                let acq_start =
                    behavior::local_acquisition_start_ns(&recon, cand, shot.timestamp_ns, config);
                let behavior = behavior::behavior_for_shot(
                    &recon,
                    cand,
                    shot,
                    link.correction_start_ns,
                    link.correction_end_ns,
                    config,
                );
                let exposure = Some(exposure::exposure_for_interval(
                    &recon,
                    cand.start_ns,
                    cand.end_ns.min(shot.timestamp_ns),
                    &bundle.trial.processor_id,
                    &bundle.trial.processor_config_json,
                ));
                if let Some(ref exp) = exposure {
                    if quality::scale_mismatch(
                        &recon,
                        cand.start_ns,
                        cand.end_ns.min(shot.timestamp_ns),
                        &bundle.trial.processor_id,
                        &bundle.trial.processor_config_json,
                        config.scale_mismatch_eps,
                    ) {
                        flags.push(QualityFlag::CameraInputMismatch);
                    }
                    let _ = exp;
                }
                if let Some(ref b) = behavior {
                    if b.movement_duration_ns == 0 {
                        flags.push(QualityFlag::ZeroAcquisitionDuration);
                    }
                }
                let (movement_demand, processor_exposure) = demand::demand_for_shot(
                    &recon,
                    &bundle.target_events,
                    cand,
                    shot,
                    acq_start,
                );
                (
                    behavior,
                    exposure,
                    Some(movement_demand),
                    Some(processor_exposure),
                )
            } else {
                (None, None, None, None)
            };

        flags.sort();
        flags.dedup();

        shots_out.push(ShotAnalysis {
            shot_index: link.shot_index,
            hit: shot.hit,
            primary_movement_id: link.primary_movement_id,
            correction_start_ns: link.correction_start_ns,
            correction_end_ns: link.correction_end_ns,
            behavior,
            exposure,
            movement_demand,
            processor_exposure,
            quality_flags: flags,
        });
    }

    shots_out.sort_by_key(|s| s.shot_index);
    trial_flags.sort();
    trial_flags.dedup();

    let mut candidates = candidates;
    candidates.sort_by_key(|c| c.id);

    Ok(AnalysisResult {
        analysis_version: config.analysis_version.clone(),
        trial_id: bundle.trial.id.clone(),
        trial_type: bundle.trial.trial_type.clone(),
        experiment_version: bundle.trial.experiment_version.clone(),
        task_version: bundle.trial.task_version.clone(),
        processor_id: bundle.trial.processor_id.clone(),
        processor_version: bundle.trial.processor_version.clone(),
        metric_scope: metric_scope_for_trial_type(trial_type),
        candidates,
        shots: shots_out,
        trial_quality_flags: trial_flags,
        reconstruction_max_abs_step_deg: recon.stats.max_abs_step_deg,
        reconstruction_max_angular_speed_deg_s: recon.stats.max_angular_speed_deg_s,
        reconstruction_discontinuity_count: recon.stats.discontinuity_count,
        reconstruction_yaw_wrap_crossings: recon.stats.yaw_wrap_crossings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sense_types::{AimCameraSampleRecord, AimInputSampleRecord, AimTrialRecord};

    fn empty_trial(trial_type: &str) -> AimTrialAnalysisBundle {
        AimTrialAnalysisBundle {
            trial: AimTrialRecord {
                id: "t1".into(),
                app_version: "0.1.0".into(),
                experiment_id: "aim_lab".into(),
                experiment_version: "0.12.1".into(),
                trial_type: trial_type.into(),
                status: "completed".into(),
                processor_id: "none".into(),
                processor_version: "1.0.0".into(),
                processor_config_json: "{}".into(),
                dpi: 3200.0,
                sensitivity: 0.09,
                polling_rate_hz: 1000.0,
                fov_degrees_h: 103.0,
                pitch_model_id: "unverified_0.1".into(),
                pitch_model_version: "1".into(),
                pitch_config_json: "{}".into(),
                resolution_width: 1920,
                resolution_height: 1080,
                aspect_ratio: 16.0 / 9.0,
                random_seed: 1,
                task_version: "2".into(),
                hardware_config_json: "{}".into(),
                view_config_json: "{}".into(),
                task_config_json: "{}".into(),
                metrics_json: "{}".into(),
                start_unix_ms: 0,
                end_unix_ms: 1,
                start_timestamp_ns: 0,
                end_timestamp_ns: 1_000_000_000,
                duration_secs: 1.0,
                hits: 0,
                shots: 0,
                misses: 0,
                score_secs: 0.0,
                accuracy: 0.0,
            },
            target_events: vec![],
            shots: vec![],
            camera_samples: vec![AimCameraSampleRecord {
                timestamp_ns: 0,
                yaw_deg: 0.0,
                pitch_deg: 0.0,
                yaw_delta_deg: 0.0,
                pitch_delta_deg: 0.0,
            }],
            input_samples: vec![AimInputSampleRecord {
                timestamp_ns: 0,
                sequence_number: 0,
                raw_dx: 0,
                raw_dy: 0,
                processed_dx: 0.0,
                processed_dy: 0.0,
                dt_ns: 0,
                dt_used_ns: 0,
                input_speed: None,
                acceleration_scale: None,
            }],
        }
    }

    #[test]
    fn tracking_without_shots_is_insufficient() {
        let err = analyze_trial(&empty_trial("TRACKING"), &AnalysisConfig::v1()).unwrap_err();
        assert!(matches!(err, AnalysisError::InsufficientTelemetry(_)));
    }

    #[test]
    fn tracking_with_shots_is_supported() {
        let mut b = empty_trial("TRACKING");
        b.shots.push(sense_types::AimShotRecord {
            shot_index: 0,
            timestamp_ns: 0,
            hit: true,
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            target_x: 0.0,
            target_y: 1.6,
            target_z: -2.0,
            target_radius: 0.25,
            target_id: "t".into(),
        });
        let r = analyze_trial(&b, &AnalysisConfig::v1()).unwrap();
        assert_eq!(r.analysis_version, "2");
        assert_eq!(r.shots.len(), 1);
    }

    #[test]
    fn static_click_empty_shots_ok() {
        let r = analyze_trial(&empty_trial("STATIC_CLICK"), &AnalysisConfig::v1()).unwrap();
        assert_eq!(r.analysis_version, "2");
        assert!(r.shots.is_empty());
    }
}

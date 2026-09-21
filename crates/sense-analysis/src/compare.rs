//! M4.2 condition comparison — pure (no SQLite).

use crate::scope::MetricScope;
use crate::{AnalysisResult, QualityFlag};
use sense_types::AimTrialRecord;
use serde::Serialize;

pub const COMPARISON_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchStatus {
    Matched,
    Mismatched,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonValidity {
    FairMatch,
    IntentionalParameterDiff,
    InvalidCrossTask,
    OtherMismatch,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConditionMismatch {
    pub key: String,
    pub value_a: String,
    pub value_b: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConditionSnapshot {
    pub id: String,
    pub trial_type: String,
    pub task_version: String,
    pub experiment_version: String,
    pub dpi: f64,
    pub sensitivity: f64,
    pub fov_degrees_h: f64,
    pub resolution_width: u32,
    pub resolution_height: u32,
    pub aspect_ratio: f64,
    pub processor_id: String,
    pub processor_version: String,
    pub hits: u32,
    pub shots: u32,
    pub accuracy: f64,
    pub score_secs: f64,
    pub duration_secs: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProcessorDiff {
    pub processor_id_a: String,
    pub processor_id_b: String,
    pub processor_version_a: String,
    pub processor_version_b: String,
    pub processor_config_json_a: String,
    pub processor_config_json_b: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ComparedMetric {
    pub name: String,
    pub a_mean: Option<f64>,
    pub a_median: Option<f64>,
    pub b_mean: Option<f64>,
    pub b_median: Option<f64>,
    pub delta_mean: Option<f64>,
    pub delta_median: Option<f64>,
    pub n_a: usize,
    pub n_b: usize,
    pub scope_hint: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QualitySummary {
    pub no_behavior_shots_a: usize,
    pub no_behavior_shots_b: usize,
    pub multi_shot_flag_shots_a: usize,
    pub multi_shot_flag_shots_b: usize,
    pub trial_quality_flags_a: Vec<QualityFlag>,
    pub trial_quality_flags_b: Vec<QualityFlag>,
    pub reconstruction_max_abs_step_deg_a: f64,
    pub reconstruction_max_abs_step_deg_b: f64,
    pub reconstruction_max_angular_speed_deg_s_a: f64,
    pub reconstruction_max_angular_speed_deg_s_b: f64,
    pub reconstruction_discontinuity_count_a: u32,
    pub reconstruction_discontinuity_count_b: u32,
    pub reconstruction_yaw_wrap_crossings_a: u32,
    pub reconstruction_yaw_wrap_crossings_b: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConditionComparison {
    pub comparison_version: String,
    pub trial_a: ConditionSnapshot,
    pub trial_b: ConditionSnapshot,
    pub match_status: MatchStatus,
    pub condition_mismatches: Vec<ConditionMismatch>,
    pub comparison_validity: ComparisonValidity,
    pub processor_diff: ProcessorDiff,
    pub metric_scope_a: MetricScope,
    pub metric_scope_b: MetricScope,
    pub metric_deltas: Vec<ComparedMetric>,
    pub exposure_deltas: Vec<ComparedMetric>,
    pub quality_summary: QualitySummary,
}

const INTENTIONAL_KEYS: &[&str] = &["sensitivity", "dpi"];

/// Fairness key diffs (processor ignored; realized duration/score ignored).
pub(crate) fn fairness_mismatches(
    a: &AimTrialRecord,
    b: &AimTrialRecord,
) -> Vec<ConditionMismatch> {
    let mut out = Vec::new();
    push_if_ne(&mut out, "trial_type", &a.trial_type, &b.trial_type);
    push_if_ne(&mut out, "task_version", &a.task_version, &b.task_version);
    push_if_ne(
        &mut out,
        "task_config_json",
        &a.task_config_json,
        &b.task_config_json,
    );
    push_if_ne_f64(&mut out, "dpi", a.dpi, b.dpi);
    push_if_ne_f64(&mut out, "sensitivity", a.sensitivity, b.sensitivity);
    push_if_ne_f64(&mut out, "fov_degrees_h", a.fov_degrees_h, b.fov_degrees_h);
    push_if_ne(
        &mut out,
        "resolution_width",
        &a.resolution_width.to_string(),
        &b.resolution_width.to_string(),
    );
    push_if_ne(
        &mut out,
        "resolution_height",
        &a.resolution_height.to_string(),
        &b.resolution_height.to_string(),
    );
    push_if_ne_f64(&mut out, "aspect_ratio", a.aspect_ratio, b.aspect_ratio);
    push_if_ne(
        &mut out,
        "hardware_config_json",
        &a.hardware_config_json,
        &b.hardware_config_json,
    );
    push_if_ne(
        &mut out,
        "view_config_json",
        &a.view_config_json,
        &b.view_config_json,
    );
    out
}

fn push_if_ne(out: &mut Vec<ConditionMismatch>, key: &str, va: &str, vb: &str) {
    if va != vb {
        out.push(ConditionMismatch {
            key: key.into(),
            value_a: va.into(),
            value_b: vb.into(),
        });
    }
}

fn push_if_ne_f64(out: &mut Vec<ConditionMismatch>, key: &str, va: f64, vb: f64) {
    if (va - vb).abs() > 1e-12 {
        out.push(ConditionMismatch {
            key: key.into(),
            value_a: format!("{va}"),
            value_b: format!("{vb}"),
        });
    }
}

pub(crate) fn validity_from_mismatches(mismatches: &[ConditionMismatch]) -> ComparisonValidity {
    if mismatches.is_empty() {
        return ComparisonValidity::FairMatch;
    }
    if mismatches
        .iter()
        .any(|m| m.key == "trial_type" || m.key == "task_version")
    {
        return ComparisonValidity::InvalidCrossTask;
    }
    if mismatches
        .iter()
        .all(|m| INTENTIONAL_KEYS.contains(&m.key.as_str()))
    {
        return ComparisonValidity::IntentionalParameterDiff;
    }
    ComparisonValidity::OtherMismatch
}

fn snapshot(t: &AimTrialRecord) -> ConditionSnapshot {
    ConditionSnapshot {
        id: t.id.clone(),
        trial_type: t.trial_type.clone(),
        task_version: t.task_version.clone(),
        experiment_version: t.experiment_version.clone(),
        dpi: t.dpi,
        sensitivity: t.sensitivity,
        fov_degrees_h: t.fov_degrees_h,
        resolution_width: t.resolution_width,
        resolution_height: t.resolution_height,
        aspect_ratio: t.aspect_ratio,
        processor_id: t.processor_id.clone(),
        processor_version: t.processor_version.clone(),
        hits: t.hits,
        shots: t.shots,
        accuracy: t.accuracy,
        score_secs: t.score_secs,
        duration_secs: t.duration_secs,
    }
}

fn empty_quality(a: &AnalysisResult, b: &AnalysisResult) -> QualitySummary {
    QualitySummary {
        no_behavior_shots_a: a.shots.iter().filter(|s| s.behavior.is_none()).count(),
        no_behavior_shots_b: b.shots.iter().filter(|s| s.behavior.is_none()).count(),
        multi_shot_flag_shots_a: a
            .shots
            .iter()
            .filter(|s| {
                s.quality_flags
                    .iter()
                    .any(|f| matches!(f, QualityFlag::MultiShotMovement))
            })
            .count(),
        multi_shot_flag_shots_b: b
            .shots
            .iter()
            .filter(|s| {
                s.quality_flags
                    .iter()
                    .any(|f| matches!(f, QualityFlag::MultiShotMovement))
            })
            .count(),
        trial_quality_flags_a: a.trial_quality_flags.clone(),
        trial_quality_flags_b: b.trial_quality_flags.clone(),
        reconstruction_max_abs_step_deg_a: a.reconstruction_max_abs_step_deg,
        reconstruction_max_abs_step_deg_b: b.reconstruction_max_abs_step_deg,
        reconstruction_max_angular_speed_deg_s_a: a.reconstruction_max_angular_speed_deg_s,
        reconstruction_max_angular_speed_deg_s_b: b.reconstruction_max_angular_speed_deg_s,
        reconstruction_discontinuity_count_a: a.reconstruction_discontinuity_count,
        reconstruction_discontinuity_count_b: b.reconstruction_discontinuity_count,
        reconstruction_yaw_wrap_crossings_a: a.reconstruction_yaw_wrap_crossings,
        reconstruction_yaw_wrap_crossings_b: b.reconstruction_yaw_wrap_crossings,
    }
}

fn mean(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        None
    } else {
        Some(xs.iter().sum::<f64>() / xs.len() as f64)
    }
}

fn median(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    if n % 2 == 1 {
        Some(v[n / 2])
    } else {
        Some((v[n / 2 - 1] + v[n / 2]) / 2.0)
    }
}

fn compared(name: &str, a: &[f64], b: &[f64], scope_hint: &str) -> ComparedMetric {
    let a_mean = mean(a);
    let a_median = median(a);
    let b_mean = mean(b);
    let b_median = median(b);
    ComparedMetric {
        name: name.into(),
        a_mean,
        a_median,
        b_mean,
        b_median,
        delta_mean: match (a_mean, b_mean) {
            (Some(x), Some(y)) => Some(y - x),
            _ => None,
        },
        delta_median: match (a_median, b_median) {
            (Some(x), Some(y)) => Some(y - x),
            _ => None,
        },
        n_a: a.len(),
        n_b: b.len(),
        scope_hint: scope_hint.into(),
    }
}

fn trial_scalar(name: &str, a: f64, b: f64, scope_hint: &str) -> ComparedMetric {
    ComparedMetric {
        name: name.into(),
        a_mean: Some(a),
        a_median: Some(a),
        b_mean: Some(b),
        b_median: Some(b),
        delta_mean: Some(b - a),
        delta_median: Some(b - a),
        n_a: 1,
        n_b: 1,
        scope_hint: scope_hint.into(),
    }
}

fn collect_metric_deltas(
    trial_a: &AimTrialRecord,
    analysis_a: &AnalysisResult,
    trial_b: &AimTrialRecord,
    analysis_b: &AnalysisResult,
) -> Vec<ComparedMetric> {
    let mut out = vec![
        trial_scalar(
            "shot_accuracy",
            trial_a.accuracy,
            trial_b.accuracy,
            "shot_accuracy",
        ),
        trial_scalar("hits", trial_a.hits as f64, trial_b.hits as f64, "shot_accuracy"),
        trial_scalar(
            "shots",
            trial_a.shots as f64,
            trial_b.shots as f64,
            "shot_accuracy",
        ),
        trial_scalar(
            "score_secs",
            trial_a.score_secs,
            trial_b.score_secs,
            "continuous_tracking_metrics",
        ),
    ];
    if trial_a.trial_type == "TRACKING" || trial_b.trial_type == "TRACKING" {
        let ratio = |t: &AimTrialRecord| {
            if t.duration_secs > 0.0 {
                t.score_secs / t.duration_secs
            } else {
                0.0
            }
        };
        out.push(trial_scalar(
            "on_target_ratio",
            ratio(trial_a),
            ratio(trial_b),
            "continuous_tracking_metrics",
        ));
    }

    let beh_a: Vec<_> = analysis_a
        .shots
        .iter()
        .filter_map(|s| s.behavior.as_ref())
        .collect();
    let beh_b: Vec<_> = analysis_b
        .shots
        .iter()
        .filter_map(|s| s.behavior.as_ref())
        .collect();

    let pick = |sel: fn(&crate::BehaviorMetrics) -> f64, name: &str| {
        let a: Vec<f64> = beh_a.iter().map(|b| sel(b)).collect();
        let b: Vec<f64> = beh_b.iter().map(|b| sel(b)).collect();
        compared(name, &a, &b, "acquisition_metrics")
    };
    out.push(pick(|b| b.endpoint_error_deg, "endpoint_error_deg"));
    out.push(pick(|b| b.overshoot_deg, "overshoot_deg"));
    out.push(pick(
        |b| b.movement_duration_ns as f64,
        "movement_duration_ns",
    ));
    out.push(pick(
        |b| b.angular_velocity_peak_deg_s,
        "angular_velocity_peak_deg_s",
    ));
    out.push(pick(|b| b.jitter_rms_deg_s, "jitter_rms_deg_s"));

    let corr_a: Vec<f64> = beh_a
        .iter()
        .filter_map(|b| b.correction_magnitude_deg)
        .collect();
    let corr_b: Vec<f64> = beh_b
        .iter()
        .filter_map(|b| b.correction_magnitude_deg)
        .collect();
    out.push(compared(
        "correction_magnitude_deg",
        &corr_a,
        &corr_b,
        "acquisition_metrics",
    ));

    let pe_a: Vec<f64> = beh_a.iter().filter_map(|b| b.path_efficiency).collect();
    let pe_b: Vec<f64> = beh_b.iter().filter_map(|b| b.path_efficiency).collect();
    out.push(compared(
        "path_efficiency",
        &pe_a,
        &pe_b,
        "acquisition_metrics",
    ));

    out
}

fn collect_exposure_deltas(
    analysis_a: &AnalysisResult,
    analysis_b: &AnalysisResult,
) -> Vec<ComparedMetric> {
    let exp_a: Vec<_> = analysis_a
        .shots
        .iter()
        .filter_map(|s| s.exposure.as_ref())
        .collect();
    let exp_b: Vec<_> = analysis_b
        .shots
        .iter()
        .filter_map(|s| s.exposure.as_ref())
        .collect();

    let mut out = Vec::new();
    let pick = |sel: fn(&crate::ExposureMetrics) -> f64, name: &str| {
        let a: Vec<f64> = exp_a.iter().map(|e| sel(e)).collect();
        let b: Vec<f64> = exp_b.iter().map(|e| sel(e)).collect();
        compared(name, &a, &b, "exposure")
    };
    out.push(pick(|e| e.raw_speed_mean, "raw_speed_mean"));
    out.push(pick(|e| e.raw_speed_peak, "raw_speed_peak"));
    out.push(pick(|e| e.processed_speed_mean, "processed_speed_mean"));
    out.push(pick(|e| e.processed_speed_peak, "processed_speed_peak"));
    out.push(pick(|e| e.gain_ratio_mean, "gain_ratio_mean"));
    out.push(pick(|e| e.gain_ratio_peak, "gain_ratio_peak"));
    out.push(pick(|e| e.cap_exposure, "cap_exposure"));

    let scale_a: Vec<f64> = exp_a
        .iter()
        .filter_map(|e| e.acceleration_scale_mean)
        .collect();
    let scale_b: Vec<f64> = exp_b
        .iter()
        .filter_map(|e| e.acceleration_scale_mean)
        .collect();
    out.push(compared(
        "acceleration_scale_mean",
        &scale_a,
        &scale_b,
        "exposure",
    ));
    let peak_a: Vec<f64> = exp_a
        .iter()
        .filter_map(|e| e.acceleration_scale_peak)
        .collect();
    let peak_b: Vec<f64> = exp_b
        .iter()
        .filter_map(|e| e.acceleration_scale_peak)
        .collect();
    out.push(compared(
        "acceleration_scale_peak",
        &peak_a,
        &peak_b,
        "exposure",
    ));
    out
}

/// Compare two analyzed trials. Always emits deltas when analyses are present;
/// `match_status` / `comparison_validity` annotate fairness only.
pub fn compare_trials(
    trial_a: &AimTrialRecord,
    analysis_a: &AnalysisResult,
    trial_b: &AimTrialRecord,
    analysis_b: &AnalysisResult,
) -> ConditionComparison {
    let mismatches = fairness_mismatches(trial_a, trial_b);
    let match_status = if mismatches.is_empty() {
        MatchStatus::Matched
    } else {
        MatchStatus::Mismatched
    };
    let comparison_validity = validity_from_mismatches(&mismatches);

    ConditionComparison {
        comparison_version: COMPARISON_VERSION.into(),
        trial_a: snapshot(trial_a),
        trial_b: snapshot(trial_b),
        match_status,
        condition_mismatches: mismatches,
        comparison_validity,
        processor_diff: ProcessorDiff {
            processor_id_a: trial_a.processor_id.clone(),
            processor_id_b: trial_b.processor_id.clone(),
            processor_version_a: trial_a.processor_version.clone(),
            processor_version_b: trial_b.processor_version.clone(),
            processor_config_json_a: trial_a.processor_config_json.clone(),
            processor_config_json_b: trial_b.processor_config_json.clone(),
        },
        metric_scope_a: analysis_a.metric_scope.clone(),
        metric_scope_b: analysis_b.metric_scope.clone(),
        metric_deltas: collect_metric_deltas(trial_a, analysis_a, trial_b, analysis_b),
        exposure_deltas: collect_exposure_deltas(analysis_a, analysis_b),
        quality_summary: empty_quality(analysis_a, analysis_b),
    }
}
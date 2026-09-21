use crate::compare::{compare_trials, fairness_mismatches, ConditionComparison};
use crate::{analyze_trial, AnalysisConfig, AnalysisError, AnalysisResult};
use sense_types::AimTrialRecord;
use sense_telemetry::TelemetryDb;
use std::path::Path;

pub fn load_and_analyze(db_path: &str, trial_id: &str) -> Result<AnalysisResult, AnalysisError> {
    let db = TelemetryDb::open(Path::new(db_path)).map_err(AnalysisError::Loader)?;
    db.migrate().map_err(AnalysisError::Loader)?;
    let bundle = db
        .load_aim_trial_analysis_bundle(trial_id)
        .map_err(AnalysisError::Loader)?;
    analyze_trial(&bundle, &AnalysisConfig::v1())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessorFilter {
    Any,
    MustDiffer,
    Exact { processor_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateCriteria {
    pub processor_filter: ProcessorFilter,
    pub list_limit: usize,
}

impl Default for CandidateCriteria {
    fn default() -> Self {
        Self {
            processor_filter: ProcessorFilter::MustDiffer,
            list_limit: 200,
        }
    }
}

pub(crate) fn processor_filter_ok(
    reference: &AimTrialRecord,
    candidate: &AimTrialRecord,
    filter: &ProcessorFilter,
) -> bool {
    match filter {
        ProcessorFilter::Any => true,
        ProcessorFilter::MustDiffer => candidate.processor_id != reference.processor_id,
        ProcessorFilter::Exact { processor_id } => candidate.processor_id == *processor_id,
    }
}

pub fn compare_trial_ids(
    db_path: &str,
    id_a: &str,
    id_b: &str,
) -> Result<ConditionComparison, AnalysisError> {
    let db = TelemetryDb::open(Path::new(db_path)).map_err(AnalysisError::Loader)?;
    db.migrate().map_err(AnalysisError::Loader)?;
    let ba = db
        .load_aim_trial_analysis_bundle(id_a)
        .map_err(AnalysisError::Loader)?;
    let bb = db
        .load_aim_trial_analysis_bundle(id_b)
        .map_err(AnalysisError::Loader)?;
    let aa = analyze_trial(&ba, &AnalysisConfig::v1())?;
    let ab = analyze_trial(&bb, &AnalysisConfig::v1())?;
    Ok(compare_trials(&ba.trial, &aa, &bb.trial, &ab))
}

pub fn find_comparison_candidates(
    db_path: &str,
    reference_trial_id: &str,
    criteria: CandidateCriteria,
) -> Result<Vec<AimTrialRecord>, AnalysisError> {
    let db = TelemetryDb::open(Path::new(db_path)).map_err(AnalysisError::Loader)?;
    db.migrate().map_err(AnalysisError::Loader)?;
    let reference = db
        .load_aim_trial_analysis_bundle(reference_trial_id)
        .map_err(AnalysisError::Loader)?
        .trial;
    let summaries = db
        .list_aim_trials_summary(criteria.list_limit)
        .map_err(AnalysisError::Loader)?;
    let mut out = Vec::new();
    for s in summaries {
        if s.id == reference.id {
            continue;
        }
        let cand = db
            .load_aim_trial_analysis_bundle(&s.id)
            .map_err(AnalysisError::Loader)?
            .trial;
        if !fairness_mismatches(&reference, &cand).is_empty() {
            continue;
        }
        if !processor_filter_ok(&reference, &cand, &criteria.processor_filter) {
            continue;
        }
        out.push(cand);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trial(processor_id: &str) -> AimTrialRecord {
        AimTrialRecord {
            id: "t".into(),
            app_version: "0.1.0".into(),
            experiment_id: "aim_lab".into(),
            experiment_version: "0.12.2".into(),
            trial_type: "GRIDSHOT".into(),
            status: "completed".into(),
            processor_id: processor_id.into(),
            processor_version: "1.0.0".into(),
            processor_config_json: "{}".into(),
            dpi: 3200.0,
            sensitivity: 0.09,
            polling_rate_hz: 1000.0,
            fov_degrees_h: 103.0,
            pitch_model_id: "".into(),
            pitch_model_version: "".into(),
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
            end_timestamp_ns: 1,
            duration_secs: 1.0,
            hits: 0,
            shots: 0,
            misses: 0,
            score_secs: 0.0,
            accuracy: 0.0,
        }
    }

    #[test]
    fn processor_must_differ_rejects_same() {
        let a = trial("none");
        let b = trial("none");
        assert!(!processor_filter_ok(
            &a,
            &b,
            &ProcessorFilter::MustDiffer
        ));
        let c = trial("rawaccel_linear");
        assert!(processor_filter_ok(
            &a,
            &c,
            &ProcessorFilter::MustDiffer
        ));
    }
}

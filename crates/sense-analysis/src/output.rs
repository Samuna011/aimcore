use crate::AnalysisResult;
use serde_json::json;

/// Deterministic JSON dump (stable field names; shots/candidates already sorted by caller).
pub fn analysis_result_to_json(result: &AnalysisResult) -> String {
    // Serialize via serde; pretty for dump readability.
    serde_json::to_string_pretty(result).unwrap_or_else(|_| json!({ "error": "serialize" }).to_string())
}

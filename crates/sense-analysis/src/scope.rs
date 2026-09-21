use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Applicability {
    Applicable,
    SecondaryReference,
    Partial,
    NotApplicable,
}

impl Applicability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Applicable => "applicable",
            Self::SecondaryReference => "secondary_reference",
            Self::Partial => "partial",
            Self::NotApplicable => "not_applicable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MetricScope {
    pub shot_accuracy: Applicability,
    pub acquisition_metrics: Applicability,
    pub continuous_tracking_metrics: Applicability,
    pub click_association: Applicability,
}

pub fn metric_scope_for_trial_type(trial_type: &str) -> MetricScope {
    match trial_type {
        "TRACKING" => MetricScope {
            shot_accuracy: Applicability::Applicable,
            acquisition_metrics: Applicability::SecondaryReference,
            continuous_tracking_metrics: Applicability::SecondaryReference,
            click_association: Applicability::Partial,
        },
        _ => MetricScope {
            shot_accuracy: Applicability::Applicable,
            acquisition_metrics: Applicability::Applicable,
            continuous_tracking_metrics: Applicability::NotApplicable,
            click_association: Applicability::Applicable,
        },
    }
}

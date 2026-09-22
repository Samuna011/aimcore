use sense_analysis::metric_scope_for_trial_type;

#[test]
fn static_click_scope_is_fully_applicable() {
    let s = metric_scope_for_trial_type("STATIC_CLICK");
    assert_eq!(s.shot_accuracy.as_str(), "applicable");
    assert_eq!(s.acquisition_metrics.as_str(), "applicable");
    assert_eq!(s.continuous_tracking_metrics.as_str(), "not_applicable");
    assert_eq!(s.click_association.as_str(), "applicable");
}

#[test]
fn tracking_scope_marks_acquisition_secondary() {
    let s = metric_scope_for_trial_type("TRACKING");
    assert_eq!(s.shot_accuracy.as_str(), "applicable");
    assert_eq!(s.acquisition_metrics.as_str(), "secondary_reference");
    assert_eq!(s.continuous_tracking_metrics.as_str(), "secondary_reference");
    assert_eq!(s.click_association.as_str(), "partial");
}

#[test]
fn gridshot_matches_static_click_scope() {
    let a = metric_scope_for_trial_type("GRIDSHOT");
    let b = metric_scope_for_trial_type("STATIC_CLICK");
    assert_eq!(a, b);
}

#[test]
fn flick_and_one_wall_match_gridshot_scope() {
    let g = metric_scope_for_trial_type("GRIDSHOT");
    assert_eq!(metric_scope_for_trial_type("FLICK_LADDER"), g);
    assert_eq!(metric_scope_for_trial_type("ONE_WALL_SIX"), g);
    assert_eq!(metric_scope_for_trial_type("FLICK_DEMAND"), g);
}

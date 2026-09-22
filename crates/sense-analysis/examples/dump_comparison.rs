//! Dump M4.2 condition comparison JSON for two trials.
//!
//! Usage:
//!   cargo run -p sense-analysis --release --example dump_comparison -- id_a id_b
//!
//! Writes `comparison_<a>_vs_<b>.json`. DB path from `SENSE_MAXER_DB` or `data/sense_maxer.db`.

use sense_analysis::{compare_trial_ids, ConditionComparison};
use std::env;
use std::fs;
use std::time::Instant;

fn main() {
    let db_path = env::var("SENSE_MAXER_DB").unwrap_or_else(|_| "data/sense_maxer.db".into());
    let mut args = env::args().skip(1);
    let id_a = args.next().unwrap_or_else(|| {
        eprintln!("usage: dump_comparison <trial_id_a> <trial_id_b>");
        std::process::exit(2);
    });
    let id_b = args.next().unwrap_or_else(|| {
        eprintln!("usage: dump_comparison <trial_id_a> <trial_id_b>");
        std::process::exit(2);
    });

    eprintln!("comparing {id_a} vs {id_b} from {db_path} …");
    let t0 = Instant::now();
    match compare_trial_ids(&db_path, &id_a, &id_b) {
        Ok(cmp) => {
            eprintln!("done in {:.1}s", t0.elapsed().as_secs_f64());
            print_summary(&cmp);
            let out_path = format!("comparison_{id_a}_vs_{id_b}.json");
            let json = serde_json::to_string_pretty(&cmp).expect("serialize");
            fs::write(&out_path, &json).expect("write json");
            eprintln!("wrote {out_path} ({:.1} KB)", json.len() as f64 / 1e3);
        }
        Err(e) => {
            eprintln!("comparison failed: {e}");
            std::process::exit(1);
        }
    }
}

fn print_summary(c: &ConditionComparison) {
    println!("comparison_version={}", c.comparison_version);
    println!(
        "a={} ({})  b={} ({})",
        c.trial_a.id, c.trial_a.processor_id, c.trial_b.id, c.trial_b.processor_id
    );
    println!("match_status={:?}", c.match_status);
    println!("comparison_validity={:?}", c.comparison_validity);
    if !c.condition_mismatches.is_empty() {
        println!("mismatches:");
        for m in &c.condition_mismatches {
            println!("  {} | {} vs {}", m.key, m.value_a, m.value_b);
        }
    }
    for name in ["shot_accuracy", "endpoint_error_deg", "overshoot_deg", "path_efficiency"] {
        if let Some(m) = c.metric_deltas.iter().find(|m| m.name == name) {
            println!(
                "{name}: a_mean={:?} b_mean={:?} delta_mean={:?}",
                m.a_mean, m.b_mean, m.delta_mean
            );
        }
    }
    for name in [
        "physical_raw_speed_mean",
        "processor_input_speed_mean",
        "acceleration_scale_mean",
        "cap_exposure",
    ] {
        if let Some(m) = c.exposure_deltas.iter().find(|m| m.name == name) {
            println!(
                "{name}: a_mean={:?} b_mean={:?} delta_mean={:?}",
                m.a_mean, m.b_mean, m.delta_mean
            );
        }
    }
}

//! Dump M4.1 analysis JSON for one trial.
//!
//! Usage:
//!   cargo run -p sense-analysis --release --example dump_analysis -- aim_20260920_000005
//!
//! Writes `analysis_<trial_id>.json` in the current directory and prints a short
//! summary to the terminal (full pretty JSON is too large to scroll for GRIDSHOT).
//!
//! DB path from `SENSE_MAXER_DB` or default `data/sense_maxer.db`.

use sense_analysis::{load_and_analyze, AnalysisResult};
use sense_telemetry::TelemetryDb;
use std::env;
use std::fs;
use std::path::Path;
use std::time::Instant;

fn main() {
    let db_path = env::var("SENSE_MAXER_DB").unwrap_or_else(|_| "data/sense_maxer.db".into());
    let trial_id = env::args().nth(1).unwrap_or_else(|| {
        let db = TelemetryDb::open(Path::new(&db_path)).expect("open db");
        db.migrate().expect("migrate");
        let list = db.list_aim_trials_summary(1).expect("list");
        list.first()
            .map(|s| s.id.clone())
            .expect("no trials in db; pass trial_id")
    });

    eprintln!("loading + analyzing {trial_id} from {db_path} …");
    let t0 = Instant::now();
    match load_and_analyze(&db_path, &trial_id) {
        Ok(result) => {
            eprintln!("done in {:.1}s", t0.elapsed().as_secs_f64());
            print_summary(&result);
            let out_path = format!("analysis_{trial_id}.json");
            let json = serde_json::to_string_pretty(&result).expect("serialize");
            fs::write(&out_path, &json).expect("write json");
            eprintln!("wrote {out_path} ({:.1} MB)", json.len() as f64 / 1e6);
            eprintln!("open that file in the editor — this tool does not open a window.");
        }
        Err(e) => {
            eprintln!("analysis failed: {e}");
            std::process::exit(1);
        }
    }
}

fn print_summary(r: &AnalysisResult) {
    println!("trial_id={}", r.trial_id);
    println!("trial_type={}", r.trial_type);
    println!("analysis_version={}", r.analysis_version);
    println!("candidates={}", r.candidates.len());
    println!("shots={}", r.shots.len());
    println!(
        "recon max_step_deg={:.3} max_speed_deg_s={:.1} discontinuities={} yaw_wrap_crossings={}",
        r.reconstruction_max_abs_step_deg,
        r.reconstruction_max_angular_speed_deg_s,
        r.reconstruction_discontinuity_count,
        r.reconstruction_yaw_wrap_crossings
    );
    let with_behavior = r.shots.iter().filter(|s| s.behavior.is_some()).count();
    let orphans = r
        .shots
        .iter()
        .filter(|s| {
            s.quality_flags
                .iter()
                .any(|f| matches!(f, sense_analysis::QualityFlag::OrphanShot))
        })
        .count();
    println!("shots_with_behavior={with_behavior}");
    println!("orphan_shots={orphans}");
    if let Some(s) = r.shots.first() {
        if let Some(b) = &s.behavior {
            println!(
                "shot0 endpoint_error_deg={:.3} overshoot_deg={:.3} path_efficiency={:?}",
                b.endpoint_error_deg, b.overshoot_deg, b.path_efficiency
            );
        }
        if let Some(e) = &s.exposure {
            println!(
                "shot0 accel_scale_mean={:?} processor_input_speed={:?} physical_raw={:.3} cap_exposure={:.3}",
                e.acceleration_scale_mean,
                e.processor_input_speed_mean,
                e.physical_raw_speed_mean,
                e.cap_exposure
            );
        }
    }
}

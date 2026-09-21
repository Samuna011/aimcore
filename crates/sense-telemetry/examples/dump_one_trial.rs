//! One-off: print a recent aim trial bundle from data/sense_maxer.db
use sense_telemetry::TelemetryDb;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "data/sense_maxer.db".into());
    let db = TelemetryDb::open(Path::new(&path))?;
    let list = db.list_aim_trials_summary(3)?;
    println!("recent trials: {}", list.len());
    for s in &list {
        println!(
            "  {} | {} | hits {}/{} acc {:.1}% score {:.3}s ver {}",
            s.id,
            s.trial_type,
            s.hits,
            s.shots,
            s.accuracy * 100.0,
            s.score_secs,
            s.experiment_version
        );
    }
    let Some(pick) = list.first() else {
        println!("(empty database)");
        return Ok(());
    };
    let b = db.load_aim_trial_bundle(&pick.id)?;
    let t = &b.trial;
    println!("\n=== TRIAL {} ===", t.id);
    println!(
        "type={} status={} experiment_version={}",
        t.trial_type, t.status, t.experiment_version
    );
    println!(
        "hits={} shots={} misses={} accuracy={:.4} score_secs={:.3} duration_secs={:.3}",
        t.hits, t.shots, t.misses, t.accuracy, t.score_secs, t.duration_secs
    );
    println!(
        "dpi={} sens={} fov_h={} processor={}/{}",
        t.dpi, t.sensitivity, t.fov_degrees_h, t.processor_id, t.processor_version
    );
    println!(
        "random_seed={} task_version={}",
        t.random_seed, t.task_version
    );
    println!(
        "start_unix_ms={} end_unix_ms={} start_ts_ns={} end_ts_ns={}",
        t.start_unix_ms, t.end_unix_ms, t.start_timestamp_ns, t.end_timestamp_ns
    );
    println!(
        "child counts: events={} shots={} camera={} (input not loaded in v1 bundle)",
        b.target_events.len(),
        b.shots.len(),
        b.camera_samples.len()
    );

    println!("\n--- shots ---");
    for s in &b.shots {
        println!(
            "  #{:<3} t={} hit={} yaw={:.3} pitch={:.3} target={} @({:.2},{:.2},{:.2}) r={}",
            s.shot_index,
            s.timestamp_ns,
            s.hit,
            s.yaw_deg,
            s.pitch_deg,
            s.target_id,
            s.target_x,
            s.target_y,
            s.target_z,
            s.target_radius
        );
    }

    println!("\n--- target events (first 16) ---");
    for e in b.target_events.iter().take(16) {
        println!(
            "  #{:<3} {} {} t={} pos=({:.2},{:.2},{:.2})",
            e.event_index,
            e.event_type,
            e.target_id,
            e.timestamp_ns,
            e.position_x,
            e.position_y,
            e.position_z
        );
    }
    if b.target_events.len() > 16 {
        println!("  ... {} more", b.target_events.len() - 16);
    }

    println!(
        "\n--- camera samples (first 3 + last 3 of {}) ---",
        b.camera_samples.len()
    );
    let cams = &b.camera_samples;
    let head = cams.iter().take(3);
    let tail_start = cams.len().saturating_sub(3);
    for c in head.chain(cams.iter().skip(tail_start)) {
        println!(
            "  t={} yaw={:.4} pitch={:.4} dyaw={:.4} dpitch={:.4}",
            c.timestamp_ns, c.yaw_deg, c.pitch_deg, c.yaw_delta_deg, c.pitch_delta_deg
        );
    }
    Ok(())
}

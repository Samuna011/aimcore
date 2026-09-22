//! Physical vs processor-path exposure speeds (analysis_version 2).

use sense_analysis::exposure::exposure_for_interval;
use sense_analysis::reconstruct::{InputPoint, ReconstructedTrial};

fn point(
    t_ns: u64,
    raw_dx: i32,
    processed_dx: f64,
    dt_ns: u64,
    dt_used_ns: u64,
    physical_raw: f64,
    physical_proc: f64,
    processor_input: Option<f64>,
    scale: Option<f64>,
) -> InputPoint {
    InputPoint {
        timestamp_ns: t_ns,
        sequence_number: 0,
        raw_dx,
        raw_dy: 0,
        processed_dx,
        processed_dy: 0.0,
        dt_ns,
        dt_used_ns,
        physical_raw_speed: physical_raw,
        physical_processed_speed: physical_proc,
        acceleration_scale: scale,
        processor_input_speed: processor_input,
    }
}

#[test]
fn none_has_physical_speeds_and_null_processor_path() {
    // 10 counts over 1 ms physical → 10 counts/ms
    let recon = ReconstructedTrial {
        camera: vec![],
        inputs: vec![point(
            1_000_000,
            10,
            10.0,
            1_000_000,
            1_000_000,
            10.0,
            10.0,
            None,
            None,
        )],
        stats: sense_analysis::reconstruct::ReconstructionStats {
            max_abs_step_deg: 0.0,
            max_angular_speed_deg_s: 0.0,
            timestamp_monotonic: true,
            discontinuity_count: 0,
            yaw_wrap_crossings: 0,
        },
    };
    let exp = exposure_for_interval(&recon, 0, 2_000_000, "none", "{}");
    assert!((exp.physical_raw_speed_mean - 10.0).abs() < 1e-12);
    assert!((exp.physical_processed_speed_mean - 10.0).abs() < 1e-12);
    assert!(exp.processor_input_speed_mean.is_none());
    assert!(exp.acceleration_scale_mean.is_none());
}

#[test]
fn linear_keeps_processor_input_speed_from_stored_column() {
    let recon = ReconstructedTrial {
        camera: vec![],
        inputs: vec![point(
            1_000_000,
            10,
            15.0,
            100_000, // 0.1 ms physical → high physical speed if recomputed
            1_000_000,
            100.0, // physical via dt_ns
            150.0,
            Some(4.5), // stored processor path — must not be overwritten
            Some(1.2),
        )],
        stats: sense_analysis::reconstruct::ReconstructionStats {
            max_abs_step_deg: 0.0,
            max_angular_speed_deg_s: 0.0,
            timestamp_monotonic: true,
            discontinuity_count: 0,
            yaw_wrap_crossings: 0,
        },
    };
    let exp = exposure_for_interval(&recon, 0, 2_000_000, "rawaccel_linear", "{}");
    assert!((exp.physical_raw_speed_mean - 100.0).abs() < 1e-12);
    assert!((exp.processor_input_speed_mean.unwrap() - 4.5).abs() < 1e-12);
    assert!((exp.acceleration_scale_mean.unwrap() - 1.2).abs() < 1e-12);
}

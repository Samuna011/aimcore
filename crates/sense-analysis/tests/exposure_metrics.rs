use sense_analysis::exposure::exposure_for_interval;
use sense_analysis::quality::{scale_mismatch, QualityFlag};
use sense_analysis::reconstruct::{InputPoint, ReconstructedTrial};
use sense_analysis::AnalysisConfig;

fn recon_with_scales(scales: &[(u64, f64, i32)]) -> ReconstructedTrial {
    ReconstructedTrial {
        camera: vec![],
        inputs: scales
            .iter()
            .enumerate()
            .map(|(i, (t_ms, scale, dx))| InputPoint {
                timestamp_ns: t_ms * 1_000_000,
                sequence_number: i as u64,
                raw_dx: *dx,
                raw_dy: 0,
                processed_dx: *dx as f64 * scale,
                processed_dy: 0.0,
                dt_ns: 1_000_000,
                dt_used_ns: 1_000_000,
                physical_raw_speed: 1.0,
                physical_processed_speed: *scale,
                acceleration_scale: Some(*scale),
                processor_input_speed: Some(1.0),
            })
            .collect(),
        stats: sense_analysis::reconstruct::ReconstructionStats {
            max_abs_step_deg: 0.0,
            max_angular_speed_deg_s: 0.0,
            timestamp_monotonic: true,
            discontinuity_count: 0,
            yaw_wrap_crossings: 0,
        },
    }
}

#[test]
fn none_processor_cap_not_applicable() {
    let recon = recon_with_scales(&[(0, 1.0, 1), (1, 1.0, 1)]);
    let exp = exposure_for_interval(&recon, 0, 10_000_000, "none", "{}");
    assert!(!exp.cap_applicable);
    assert_eq!(exp.cap_exposure, 0.0);
}

#[test]
fn capped_samples_raise_cap_exposure() {
    let cfg = r#"{"acceleration":0.007,"sensitivity_multiplier":1.0,"gain":true,"input_offset":0.0,"cap_mode":"out","cap_x":2.0,"cap_y":2.0,"polling_rate_hz":1000}"#;
    // At high speed, gain out cap_y=2 → scale approaches ~2
    let recon = recon_with_scales(&[(0, 2.0, 100), (1, 2.0, 100), (2, 1.2, 5)]);
    let exp = exposure_for_interval(&recon, 0, 10_000_000, "rawaccel_linear", cfg);
    assert!(exp.cap_applicable);
    assert!(exp.cap_exposure > 0.5, "got {}", exp.cap_exposure);
}

#[test]
fn scale_mismatch_flag_without_mutating_exposure() {
    let cfg = r#"{"acceleration":0.0,"sensitivity_multiplier":1.0,"gain":false,"input_offset":0.0,"cap_mode":"out","cap_x":0.0,"cap_y":0.0,"polling_rate_hz":1000}"#;
    // accel=0 → expected scale 1.0; store wrong scale
    let recon = recon_with_scales(&[(0, 1.5, 10)]);
    let exp = exposure_for_interval(&recon, 0, 1_000_000, "rawaccel_linear", cfg);
    assert!((exp.acceleration_scale_mean.unwrap() - 1.5).abs() < 1e-9);
    assert!(scale_mismatch(
        &recon,
        0,
        1_000_000,
        "rawaccel_linear",
        cfg,
        AnalysisConfig::v1().scale_mismatch_eps,
    ));
    let _ = QualityFlag::CameraInputMismatch;
}

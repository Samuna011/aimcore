use sense_input_win::{IntegrityTracker, MouseQueue};
use sense_types::MouseSample;

#[test]
fn drain_preserves_every_sample_order() {
    let queue = MouseQueue::new();
    queue.push_raw(10, -1, 0x01, 100);
    queue.push_raw(20, -2, 0x02, 200);
    queue.push_raw(30, -3, 0x04, 300);

    let samples = queue.drain_all();

    assert_eq!(samples.len(), 3);
    assert_eq!(
        samples
            .iter()
            .map(|sample| (
                sample.dx,
                sample.dy,
                sample.buttons,
                sample.timestamp_ns,
                sample.sequence_number,
            ))
            .collect::<Vec<_>>(),
        vec![
            (10, -1, 0x01, 100, 1),
            (20, -2, 0x02, 200, 2),
            (30, -3, 0x04, 300, 3),
        ]
    );
    assert!(queue.drain_all().is_empty());
}

#[test]
fn integrity_detects_gap_duplicate_ooo_regression() {
    let mut tracker = IntegrityTracker::default();
    for sample in [
        sample(1, 100),
        sample(3, 300),
        sample(2, 200),
        sample(3, 300),
    ] {
        tracker.observe(&sample);
    }

    let report = tracker.report();
    assert_eq!(report.samples_received, 4);
    assert_eq!(report.sequence_gaps, 1);
    assert_eq!(report.duplicate_sequences, 1);
    assert_eq!(report.out_of_order_samples, 1);
    assert_eq!(report.timestamp_regressions, 1);
}

fn sample(sequence_number: u64, timestamp_ns: u64) -> MouseSample {
    MouseSample {
        timestamp_ns,
        dx: 0,
        dy: 0,
        buttons: 0,
        sequence_number,
    }
}

use sense_types::{InputIntegrityReport, MouseSample};

#[derive(Default)]
pub struct IntegrityTracker {
    last_sequence: Option<u64>,
    last_timestamp_ns: Option<u64>,
    samples_received: u64,
    sequence_gaps: u64,
    duplicate_sequences: u64,
    out_of_order_samples: u64,
    timestamp_regressions: u64,
}

impl IntegrityTracker {
    pub fn observe(&mut self, sample: &MouseSample) {
        self.samples_received = self.samples_received.saturating_add(1);

        if let Some(last_sequence) = self.last_sequence {
            if sample.sequence_number == last_sequence {
                self.duplicate_sequences = self.duplicate_sequences.saturating_add(1);
            } else if sample.sequence_number < last_sequence {
                self.out_of_order_samples = self.out_of_order_samples.saturating_add(1);
            } else {
                self.sequence_gaps = self
                    .sequence_gaps
                    .saturating_add(sample.sequence_number - last_sequence - 1);
            }
        }

        if self
            .last_timestamp_ns
            .is_some_and(|last_timestamp_ns| sample.timestamp_ns < last_timestamp_ns)
        {
            self.timestamp_regressions = self.timestamp_regressions.saturating_add(1);
        }

        self.last_sequence = Some(sample.sequence_number);
        self.last_timestamp_ns = Some(sample.timestamp_ns);
    }

    pub fn report(&self) -> InputIntegrityReport {
        InputIntegrityReport {
            samples_received: self.samples_received,
            sequence_gaps: self.sequence_gaps,
            duplicate_sequences: self.duplicate_sequences,
            out_of_order_samples: self.out_of_order_samples,
            timestamp_regressions: self.timestamp_regressions,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

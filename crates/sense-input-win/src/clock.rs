use std::sync::OnceLock;

use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

const NANOS_PER_SECOND: u128 = 1_000_000_000;

pub fn monotonic_now_ns() -> u64 {
    let mut counter = 0_i64;
    unsafe {
        QueryPerformanceCounter(&mut counter)
            .expect("QueryPerformanceCounter is unavailable on this Windows system");
    }

    let frequency = *frequency();
    ((counter as u128 * NANOS_PER_SECOND) / frequency as u128)
        .try_into()
        .expect("QPC nanosecond timestamp overflowed u64")
}

fn frequency() -> &'static i64 {
    static FREQUENCY: OnceLock<i64> = OnceLock::new();
    FREQUENCY.get_or_init(|| {
        let mut frequency = 0_i64;
        unsafe {
            QueryPerformanceFrequency(&mut frequency)
                .expect("QueryPerformanceFrequency is unavailable on this Windows system");
        }
        assert!(frequency > 0, "QPC frequency must be positive");
        frequency
    })
}

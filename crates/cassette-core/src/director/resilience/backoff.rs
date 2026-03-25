use backoff::ExponentialBackoff;
use std::time::Duration;

pub fn director_backoff(max_elapsed_secs: u64) -> ExponentialBackoff {
    ExponentialBackoff {
        current_interval: Duration::from_secs(1),
        initial_interval: Duration::from_secs(1),
        max_interval: Duration::from_secs(60),
        max_elapsed_time: Some(Duration::from_secs(max_elapsed_secs.max(1))),
        randomization_factor: 0.2,
        multiplier: 2.0,
        ..Default::default()
    }
}

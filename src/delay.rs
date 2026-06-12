use chrono::{DateTime, Utc};
use std::time::Duration;

pub struct StatDelay {
    pub end_dt: DateTime<Utc>,
    pub duration: Duration,
}

impl StatDelay {
    pub fn get_delay_duration(&self) -> Duration {
        let now = Utc::now();
        let wasted_time = now - self.end_dt;
        let millis = if self.duration.as_millis() > (wasted_time.num_milliseconds() as u128) {
            self.duration.as_millis() - (wasted_time.num_milliseconds() as u128)
        } else {
            0u128
        };
        Duration::from_millis(millis as u64)
    }
}

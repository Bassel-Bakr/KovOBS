use chrono::{DateTime, Utc};
use std::time::Duration;

pub struct StatDelay {
    pub end_dt: DateTime<Utc>,
    pub duration: Duration,
}

impl StatDelay {
    pub fn get_delay_duration(&self) -> Duration {
        let target = self.end_dt + self.duration;
        let delta = target - Utc::now();
        Duration::from_millis(delta.num_milliseconds().max(0) as u64)
    }
}

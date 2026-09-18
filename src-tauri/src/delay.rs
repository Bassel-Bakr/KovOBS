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

#[cfg(test)]
mod tests {
    use super::StatDelay;
    use chrono::Utc;
    use std::time::Duration;

    fn ending(seconds_ago: i64, padding: u64) -> StatDelay {
        StatDelay {
            end_dt: Utc::now() - Duration::from_secs(seconds_ago as u64),
            duration: Duration::from_secs(padding),
        }
    }

    /// A run noticed at once waits the whole padding.
    #[test]
    fn the_delay_is_the_padding_when_the_run_just_ended() {
        let waited = ending(0, 10).get_delay_duration();

        assert!(waited <= Duration::from_secs(10), "{waited:?}");
        assert!(waited >= Duration::from_secs(9), "{waited:?}");
    }

    /// The padding is measured from the run, not from being noticed, so a late
    /// start waits out only what is left of it.
    #[test]
    fn a_late_pickup_waits_out_the_remainder() {
        let waited = ending(6, 10).get_delay_duration();

        assert!(waited <= Duration::from_secs(4), "{waited:?}");
        assert!(waited >= Duration::from_secs(3), "{waited:?}");
    }

    /// Later than the padding it is already too late to wait, and waiting more
    /// would only push the clip further past the run.
    #[test]
    fn a_pickup_after_the_padding_does_not_wait() {
        assert_eq!(ending(30, 10).get_delay_duration(), Duration::ZERO);
    }
}

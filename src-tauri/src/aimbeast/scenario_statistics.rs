use crate::stat::Stat;
use crate::stat::StatType::Aimbeast;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::time::Duration;

#[derive(Clone, Default, Debug, Deserialize)]
#[serde(default)]
pub struct ScenarioStatistics {
    pub scenario: String,
    #[serde(rename = "Score")]
    pub scores: Box<[f32]>,
}

impl ScenarioStatistics {
    pub fn last_score(&self) -> Option<&f32> {
        self.scores.last()
    }

    /// The best of every run before the last one, ignoring readings that are
    /// not a real number.
    ///
    /// The filter is what makes this safe rather than tidy: `partial_cmp`
    /// returns `None` against a NaN, and unwrapping that panicked the whole
    /// clipping session on a single bad line in a stats file. Filtering first
    /// means `total_cmp` is a total order over what remains, with nothing to
    /// unwrap.
    pub fn prev_highscore(&self) -> Option<&f32> {
        self.scores
            .iter()
            .rev()
            .skip(1)
            .filter(|score| score.is_finite())
            .max_by(|a, b| a.total_cmp(b))
    }

    /// Whether the last run beat everything before it.
    ///
    /// Strictly greater: matching a previous best is not beating it.
    ///
    /// The `Option` comparison gives the edge cases for free. A first run is
    /// `Some(_) > None`, which is true -- there is nothing to beat. A score
    /// that will not compare, such as a NaN, is false, so a broken reading is
    /// never mistaken for an improvement.
    pub fn is_pb(&self) -> bool {
        self.last_score() > self.prev_highscore()
    }

    /// Turns the finished run into a [`Stat`] covering the `length` that led up
    /// to `end_dt`.
    ///
    /// Aimbeast records no times of its own, so both ends come from the caller:
    /// `end_dt` from when the statistics file was written, and `length` from
    /// [`crate::aimbeast::scenario_length`]. Neither is read from the clock
    /// here, so whatever the watcher spends noticing the run does not move the
    /// window.
    pub fn into_stat(self, end_dt: DateTime<Utc>, length: Duration) -> Stat {
        let score = self.last_score().cloned().unwrap_or_default();

        Stat {
            scenario: self.scenario,
            score,
            end_dt,
            start_dt: end_dt - length,
            stat_type: Aimbeast,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ScenarioStatistics;
    use chrono::Utc;
    use std::time::Duration;

    fn with(scores: &[f32]) -> ScenarioStatistics {
        ScenarioStatistics {
            scenario: "test".into(),
            scores: scores.into(),
        }
    }

    /// The first run of a scenario has nothing to beat, so it counts.
    #[test]
    fn a_first_score_is_a_personal_best() {
        assert!(with(&[100.0]).is_pb());
    }

    /// No run happened, so there is nothing to celebrate or clip.
    #[test]
    fn no_scores_at_all_is_not_a_personal_best() {
        assert!(!with(&[]).is_pb());
    }

    #[test]
    fn beating_the_previous_best_is_a_personal_best() {
        assert!(with(&[100.0, 90.0, 120.0]).is_pb());
    }

    #[test]
    fn falling_short_is_not() {
        assert!(!with(&[100.0, 120.0, 110.0]).is_pb());
    }

    /// Matching is not beating.
    #[test]
    fn tying_the_previous_best_is_not_a_personal_best() {
        assert!(!with(&[100.0, 100.0]).is_pb());
    }

    /// The best is the highest of *all* earlier runs, not the one before.
    #[test]
    fn the_previous_best_is_the_maximum_not_the_last() {
        assert!(!with(&[150.0, 90.0, 100.0]).is_pb());
    }

    /// A NaN among the earlier scores used to panic here, taking the session
    /// with it. It needs two prior scores to reach the comparison.
    #[test]
    fn an_earlier_nan_does_not_panic() {
        assert!(with(&[f32::NAN, 1.0, 2.0]).is_pb());
        assert!(!with(&[100.0, f32::NAN, 50.0]).is_pb());
    }

    /// An infinite score is not a real reading, so it cannot be the bar to beat.
    #[test]
    fn an_infinite_earlier_score_is_ignored() {
        assert!(with(&[f32::INFINITY, 10.0, 20.0]).is_pb());
    }

    /// Nothing comparable came before, so the run stands on its own.
    #[test]
    fn only_unusable_earlier_scores_leaves_no_bar() {
        assert!(with(&[f32::NAN, f32::NAN, 10.0]).is_pb());
    }

    /// A NaN will not compare, so it cannot be an improvement either.
    #[test]
    fn an_incomparable_score_is_not_a_personal_best() {
        assert!(!with(&[100.0, f32::NAN]).is_pb());
    }

    /// The window is the scenario length, ending when the file was written.
    #[test]
    fn the_run_spans_the_scenario_length_up_to_the_end() {
        let end_dt = Utc::now();
        let stat = with(&[10.0, 20.0]).into_stat(end_dt, Duration::from_secs(15));

        assert_eq!(stat.end_dt, end_dt);
        assert_eq!(stat.start_dt, end_dt - Duration::from_secs(15));
        assert_eq!(stat.scenario, "test");
        assert_eq!(stat.score, 20.0);
    }

    /// Nothing was played, so the clock is all there is to go on.
    #[test]
    fn a_run_without_a_score_still_keeps_its_window() {
        let end_dt = Utc::now();
        let stat = with(&[]).into_stat(end_dt, Duration::from_secs(60));

        assert_eq!(stat.start_dt, end_dt - Duration::from_secs(60));
        assert_eq!(stat.score, 0.0);
    }
}

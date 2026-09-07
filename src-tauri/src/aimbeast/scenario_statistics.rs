use crate::stat::Stat;
use crate::stat::StatType::Aimbeast;
use chrono::Utc;
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

    pub fn prev_highscore(&self) -> Option<&f32> {
        self.scores
            .iter()
            .rev()
            .skip(1)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
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
}

impl From<ScenarioStatistics> for Stat {
    fn from(value: ScenarioStatistics) -> Self {
        let s = &value;
        let end_dt = Utc::now();
        Stat {
            scenario: s.scenario.clone(),
            score: s.last_score().cloned().unwrap_or_default(),
            end_dt,
            start_dt: end_dt - Duration::from_mins(1),
            stat_type: Aimbeast,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ScenarioStatistics;

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

    /// A NaN will not compare, so it cannot be an improvement either.
    #[test]
    fn an_incomparable_score_is_not_a_personal_best() {
        assert!(!with(&[100.0, f32::NAN]).is_pb());
    }
}

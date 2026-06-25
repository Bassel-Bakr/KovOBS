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

    pub fn is_pb(&self) -> bool {
        // Inverted condition because any comparison with None will return false, and we want to return true if there is no previous best score
        !(self.last_score() < self.prev_highscore())
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

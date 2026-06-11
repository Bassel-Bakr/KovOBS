use std::{collections::HashMap, fs, path};

use chrono::{DateTime, Utc};
use rayon::prelude::*;

use crate::stat::Stat;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CachedDataValue {
    high_score: f32,
    play_count: u32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CacheData {
    version: String,
    last_update: i64,
    scenarios: std::collections::HashMap<String, CachedDataValue>,
}

pub struct Cache {
    file_path: path::PathBuf,
    data: CacheData,
}

impl Cache {
    pub fn new(cache_file: &str) -> Self {
        Self {
            data: Self::default_data(),
            file_path: path::PathBuf::from(cache_file),
        }
    }

    pub fn get(&mut self, scenario: &str) -> &mut CachedDataValue {
        self.data
            .scenarios
            .entry(scenario.to_owned())
            .or_insert_with(Self::default_scenario_data)
    }

    pub fn push(&mut self, stat: &Stat) -> (bool, f32, f32) {
        let scenario_data = self.get(&stat.scenario);

        let old_high_score = scenario_data.high_score;
        let new_score = stat.score;

        let is_new_high_score = new_score > old_high_score;

        scenario_data.high_score = scenario_data.high_score.max(new_score);
        scenario_data.play_count += 1;

        (is_new_high_score, old_high_score, new_score)
    }

    pub fn load(&mut self) {
        if self.file_path.exists() {
            let json = std::fs::read_to_string(&self.file_path).unwrap();

            self.data = serde_json::from_str(&json).unwrap();
        } else {
            self.data = Self::default_data();
        }
    }

    pub fn update(&mut self, stats_folder: &str) {
        let last_update_timestamp = self.data.last_update;

        let last_update_time = DateTime::from_timestamp(last_update_timestamp, 0).unwrap();
        let current_update_time = Utc::now();

        let glob_path = format!("{}/*Stats.csv", stats_folder);

        let stat_files: Vec<_> = glob::glob(glob_path.as_str())
            .expect("Failed to read stats folder")
            .filter(|res| {
                res.as_ref()
                    .is_ok_and(|p| Stat::get_creation_or_modification_time(&p) > last_update_time)
            })
            .collect::<Result<_, _>>()
            .expect("Failed to collect stat files");

        let stats: Vec<Stat> = stat_files
            .par_iter()
            .map(|path| Stat::parse(&path).unwrap())
            .collect();

        for stat in stats {
            self.push(&stat);
        }

        self.save(current_update_time);
    }

    pub fn save(&mut self, update_time: DateTime<Utc>) {
        let previous_update = self.data.last_update;

        self.data.last_update = update_time.timestamp();

        let json = serde_json::to_string_pretty(&self.data).unwrap();

        fs::write(&self.file_path, json).unwrap();

        self.data.last_update = previous_update;
    }

    fn default_data() -> CacheData {
        CacheData {
            version: "1.0.0".into(),
            last_update: 0,
            scenarios: HashMap::new(),
        }
    }

    fn default_scenario_data() -> CachedDataValue {
        CachedDataValue {
            high_score: 0.0,
            play_count: 0,
        }
    }
}

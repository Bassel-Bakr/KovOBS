use std::{collections, fs, path};

use chrono::{DateTime, Utc};
use rayon::prelude::*;

use crate::stat::Stat;
use crate::utils;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CachedDataValue {
    high_score: f32,
    play_count: u32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CacheData {
    version: String,
    last_update: i64,
    scenarios: collections::HashMap<String, CachedDataValue>,
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

    pub fn load(&mut self) -> Result<(), std::io::Error> {
        self.data = if self.file_path.exists() {
            let json = fs::read_to_string(&self.file_path)?;
            serde_json::from_str(&json)?
        } else {
            Self::default_data()
        };

        Ok(())
    }

    pub fn update(&mut self, stats_folder: &str) -> Result<(), std::io::Error> {
        let last_update_timestamp = self.data.last_update;

        let last_update_time = DateTime::from_timestamp(last_update_timestamp, 0).unwrap();
        let current_update_time = Utc::now();

        let glob_path = format!("{}/*Stats.csv", stats_folder);

        let stat_files: Vec<_> = glob::glob(glob_path.as_str())
            .expect("Failed to read stats folder")
            .filter(|res| {
                res.as_ref().is_ok_and(|p| {
                    utils::get_creation_or_modification_time(p).unwrap() > last_update_time
                })
            })
            .collect::<Result<_, _>>()
            .expect("Failed to collect stat files");

        let stats: Vec<Stat> = stat_files
            .par_iter()
            .map(|path| Stat::parse(path).unwrap())
            .collect();

        for stat in stats {
            self.push(&stat);
        }

        self.save(current_update_time)?;

        Ok(())
    }

    pub fn save(&mut self, update_time: DateTime<Utc>) -> Result<(), std::io::Error> {
        self.data.last_update = update_time.timestamp();
        fs::write(&self.file_path, serde_json::to_string_pretty(&self.data)?)?;
        Ok(())
    }

    fn default_data() -> CacheData {
        CacheData {
            version: "1.0.0".into(),
            last_update: 0,
            scenarios: collections::HashMap::new(),
        }
    }

    fn default_scenario_data() -> CachedDataValue {
        CachedDataValue {
            high_score: 0.0,
            play_count: 0,
        }
    }
}

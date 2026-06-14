use crate::utils::Utils;
use chrono::{DateTime, Duration, Local, Timelike, Utc};
use num_traits::ToPrimitive;
use std::fmt::Display;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path,
};

use crate::consts;

#[derive(Debug, Clone)]
pub struct Stat {
    pub scenario: String,
    pub score: f32,

    pub start_dt: DateTime<Utc>,
    pub end_dt: DateTime<Utc>,
}

impl Display for Stat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{} - {} - {}",
            self.scenario,
            self.score
                .to_i32()
                .map_or_else(|| format!("{:.2}", self.score), |int| int.to_string(),),
            self.end_dt
                .with_timezone(&Local)
                .format(consts::STAT_DATE_TIME_FORMAT)
        )
    }
}

impl Stat {
    pub fn parse(stat_file: &path::Path) -> Result<Self, String> {
        let mut scenario = String::new();
        let mut score = 0.0f32;
        let mut challenge_duration = Duration::seconds(0);

        let file =
            File::open(stat_file).map_err(|e| format!("Failed to open stats file: {}", e))?;
        let mut reader = BufReader::new(file);

        let mut line = String::new();
        while reader.read_line(&mut line).unwrap_or(0) > 0 {
            let Some((key, value)) = line.split_once(',') else {
                continue;
            };

            match key {
                "Score:" => score = value.trim().parse().unwrap_or(0.0),
                "Scenario:" => scenario = value.trim().to_string(),
                "Challenge Start:" => {
                    if let Some(duration) = Self::parse_challenge_duration(value.trim()) {
                        challenge_duration = duration;
                    }
                }
                _ => (),
            }

            line.clear();
        }

        let end_dt = Self::get_end_dt(stat_file);
        let start_dt = Self::compute_start_time(end_dt, challenge_duration);
        println!("{end_dt}");

        Ok(Self {
            scenario,
            score,
            start_dt,
            end_dt,
        })
    }

    pub fn compute_start_time(end_dt: DateTime<Utc>, duration: Duration) -> DateTime<Utc> {
        let total_seconds = duration.num_seconds();

        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;

        let start_dt = end_dt
            .with_timezone(&Local)
            .with_hour(hours as u32)
            .unwrap()
            .with_minute(minutes as u32)
            .unwrap()
            .with_second(seconds as u32)
            .unwrap()
            .with_nanosecond(0)
            .unwrap()
            .with_timezone(&Utc);

        if start_dt > end_dt {
            end_dt - Duration::days(1)
        } else {
            start_dt
        }
    }
    /// Returns the end time of a Kovaaks stats file in UTC.
    ///
    /// Attempts to extract and parse the timestamp embedded in the file name,
    /// which is expected to have the form:
    ///
    /// `SCENARIO_NAME - Challenge - YYYY.MM.DD-HH.MM.SS Stats.csv`
    ///
    /// The parsed timestamp is interpreted as local time and converted to UTC.
    ///
    /// If the file name does not contain a valid timestamp, falls back to the
    /// file's creation time or, if unavailable, its last modification time.
    ///
    /// # Panics
    ///
    /// Panics if neither a valid timestamp can be parsed from the file name nor
    /// a creation or modification time can be obtained for the file.
    pub fn get_end_dt(stat_file: &path::Path) -> DateTime<Utc> {
        stat_file
            .to_string_lossy()
            .strip_suffix(consts::STAT_FILE_SUFFIX)
            .and_then(|s| s.get(s.len() - consts::STAT_DATE_TIME_LEN..))
            .and_then(Utils::parse_local_datetime)
            .unwrap_or_else(|| {
                Utils::get_creation_or_modification_time(stat_file)
                    .expect("Failed to get creation or modification time")
            })
    }

    /// Parses the challenge start time from the stats file and calculates the corresponding Duration.
    fn parse_challenge_duration(challenge_start: &str) -> Option<Duration> {
        let parts: Vec<&str> = challenge_start.split(":").collect();

        if parts.len() == 3 {
            let h: u32 = parts[0].parse().unwrap_or(0);
            let m: u32 = parts[1].parse().unwrap_or(0);
            let s: f32 = parts[2].parse().unwrap_or(0.0);
            let total_secs = h * 3600 + m * 60 + s as u32;
            Some(Duration::seconds(total_secs as i64))
        } else if parts.len() == 2 {
            let m: u32 = parts[0].parse().unwrap_or(0);
            let s: f32 = parts[1].parse().unwrap_or(0.0);
            let total_secs = m * 60 + s as u32;
            Some(Duration::seconds(total_secs as i64))
        } else {
            None
        }
    }
}

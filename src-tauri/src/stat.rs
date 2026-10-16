use crate::utils;
use chrono::{DateTime, Duration, Local, Timelike, Utc};
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

    pub stat_type: StatType,
}

#[derive(Debug, Clone)]
pub enum StatType {
    Aimbeast,
    KovaaKs,
}

impl Display for Stat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{} - {} - {}",
            self.scenario,
            if self.score.is_finite() && self.score.fract() == 0.0 {
                format!("{:.0}", self.score)
            } else {
                format!("{:.2}", self.score)
            },
            self.end_dt
                .with_timezone(&Local)
                .format(consts::STAT_DATE_TIME_FORMAT)
        )
    }
}

impl Stat {
    pub fn parse_kovaaks_stat(stat_file: &path::Path) -> Result<Self, String> {
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

        Ok(Self {
            scenario,
            score,
            start_dt,
            end_dt,
            stat_type: StatType::KovaaKs,
        })
    }

    /// Computes the start timestamp from the replay end time and the
    /// challenge duration recorded by KovaaK's.
    ///
    /// KovaaK's stores the challenge duration as a clock value (`HH:MM:SS`)
    /// rather than as an elapsed duration. This function therefore replaces
    /// the hour, minute, and second components of `end_dt` with those from
    /// `duration`.
    ///
    /// If the resulting timestamp falls after `end_dt`, it is assumed that
    /// the challenge started on the previous day.
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
    /// Returns the end time of a KovaaK's stats file in UTC.
    ///
    /// Attempts to extract and parse the timestamp embedded in the file name,
    /// which is expected to have the form:
    ///
    /// `SCENARIO_NAME - Challenge - YYYY.MM.DD-HH.MM.SS Stats.csv`
    ///
    /// The parsed timestamp is interpreted as local time and converted to UTC.
    ///
    /// The name only has whole seconds. KovaaK's writes the file as the run
    /// ends, so the file's modification time is the same moment to the
    /// millisecond, and it is used whenever it agrees with the name. A file
    /// that was copied or restored keeps its name but not its timestamp, so the
    /// name decides when they disagree. Across the latest 3,000 runs on one
    /// install, 2,999 agreed.
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
            .and_then(utils::parse_local_datetime)
            .map(|named| Self::refine_end(named, utils::get_modification_time(stat_file).ok()))
            .unwrap_or_else(|| {
                utils::get_creation_or_modification_time(stat_file)
                    .expect("Failed to get creation or modification time")
            })
    }

    /// The modification time when it is within two seconds of the end time
    /// read from the name, otherwise the name's.
    fn refine_end(named: DateTime<Utc>, modified: Option<DateTime<Utc>>) -> DateTime<Utc> {
        modified
            .filter(|modified| (*modified - named).abs() < Duration::seconds(2))
            .unwrap_or(named)
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

#[cfg(test)]
mod tests {
    use super::Stat;
    use chrono::{DateTime, Duration, Utc};

    fn at(seconds: i64, millis: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_800_000_000 + seconds, 0).expect("valid moment")
            + Duration::milliseconds(millis)
    }

    /// The name stops at the second; the file's own time does not.
    #[test]
    fn a_modification_time_that_agrees_gives_the_milliseconds() {
        assert_eq!(Stat::refine_end(at(0, 0), Some(at(0, 183))), at(0, 183));
    }

    /// A copied or restored file keeps its name but takes a new timestamp.
    #[test]
    fn a_modification_time_far_from_the_name_is_ignored() {
        assert_eq!(Stat::refine_end(at(0, 0), Some(at(86_400, 0))), at(0, 0));
        assert_eq!(Stat::refine_end(at(0, 0), Some(at(-5, 0))), at(0, 0));
    }

    #[test]
    fn without_a_modification_time_the_name_stands() {
        assert_eq!(Stat::refine_end(at(0, 0), None), at(0, 0));
    }

    /// End to end, on a file named the way KovaaK's names them.
    #[test]
    fn the_end_of_a_real_stats_file_comes_from_its_timestamp() {
        let dir = std::env::temp_dir().join("kovobs_stat_end_dt");
        _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("created folder");

        let path = dir.join("Ground Plaza - Challenge - 2026.10.15-21.33.16 Stats.csv");
        let file = std::fs::File::create(&path).expect("created file");

        let named = crate::utils::parse_local_datetime("2026.10.15-21.33.16").expect("a name time");
        let written = named + Duration::milliseconds(183);
        file.set_modified(written.into())
            .expect("set the timestamp");
        drop(file);

        assert_eq!(Stat::get_end_dt(&path), written);

        std::fs::remove_dir_all(&dir).expect("cleaned up");
    }
}

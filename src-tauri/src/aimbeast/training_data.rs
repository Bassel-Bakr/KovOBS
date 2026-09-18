//! Aimbeast's training log, and the scenario length hidden inside it.

use chrono::NaiveDate;
use encoding_rs_io::DecodeReaderBytesBuilder;
use serde::Deserialize;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Used when the training log cannot say how long the scenario runs. Most
/// Aimbeast scenarios are a minute long.
pub const DEFAULT_SCENARIO_LENGTH: Duration = Duration::from_mins(1);

const LOG_FOLDER: &str = "Training Data";
const LOG_PREFIX: &str = "training_data_";

/// The log as it is written: days, then the scenarios played on that day.
type TrainingLog = HashMap<String, HashMap<String, ScenarioDay>>;

/// One scenario's totals for one day.
///
/// `Total Time` is in the file too, and is deliberately not read here: it
/// counts abandoned runs as well, so it says nothing about the length of any
/// single one.
#[derive(Clone, Copy, Debug, Deserialize)]
struct ScenarioDay {
    #[serde(rename = "Completed Sessions Time")]
    completed_sessions_time: f64,
    #[serde(rename = "Completed Sessions")]
    completed_sessions: u32,
}

impl ScenarioDay {
    /// The length of a single completed run.
    fn length(&self) -> Option<Duration> {
        if self.completed_sessions == 0 {
            return None;
        }

        let seconds = self.completed_sessions_time / f64::from(self.completed_sessions);

        (seconds.is_finite() && seconds > 0.0).then(|| Duration::from_secs_f64(seconds))
    }
}

/// How long one completed run of `scenario` lasts.
///
/// The statistics file that triggers a clip carries no duration, so the length
/// comes from the sibling `Training Data` folder, where Aimbeast records, per
/// day and per scenario, how much time went into completed sessions and how
/// many of those there were. Their quotient is the scenario length: every
/// completed run of a scenario lasts exactly as long as the one before it.
///
/// Aimbeast writes the training log just before the statistics file, so the
/// run that is being clipped is already counted by the time this is read.
pub fn scenario_length(stats_folder: &Path, scenario: &str) -> Option<Duration> {
    // A run on New Year's Day is the first entry of its year, so the newest log
    // is not guaranteed to know the scenario and the older ones are worth a
    // look before giving up.
    log_files(&training_data_folder(stats_folder)?)
        .into_iter()
        .filter_map(|path| read_log(&path))
        .find_map(|log| length_of(&log, scenario))
}

/// The training log sits beside the statistics, both under Aimbeast's
/// `Trainer` folder.
fn training_data_folder(stats_folder: &Path) -> Option<PathBuf> {
    Some(stats_folder.parent()?.join(LOG_FOLDER))
}

/// Every yearly log in the folder, newest year first.
///
/// Ordered by the year in the name rather than by modification time: copying a
/// folder or letting Steam Cloud restore it rewrites the timestamps, and the
/// name is what actually says which year a log covers.
fn log_files(folder: &Path) -> Vec<PathBuf> {
    let pattern = folder.join(format!("{LOG_PREFIX}*.json"));

    let Ok(paths) = glob::glob(&pattern.to_string_lossy()) else {
        return Vec::new();
    };

    let mut logs: Vec<_> = paths
        .flatten()
        .filter_map(|path| Some((year_of(&path)?, path)))
        .collect();

    logs.sort_unstable_by_key(|(year, _)| Reverse(*year));

    logs.into_iter().map(|(_, path)| path).collect()
}

fn year_of(path: &Path) -> Option<i32> {
    path.file_stem()?
        .to_str()?
        .strip_prefix(LOG_PREFIX)?
        .parse()
        .ok()
}

fn read_log(path: &Path) -> Option<TrainingLog> {
    parse_log(std::fs::File::open(path).ok()?)
}

/// The logs are UTF-16 with a BOM, so the bytes are decoded rather than read
/// as UTF-8.
fn parse_log<R: Read>(reader: R) -> Option<TrainingLog> {
    let mut reader = DecodeReaderBytesBuilder::new()
        .encoding(None) // Auto-detect from BOM, otherwise UTF-8
        .build(reader);

    serde_json::from_reader(&mut reader).ok()
}

/// The latest day the scenario was completed on.
///
/// The log's own newest day decides, rather than today's date: the run being
/// clipped was just written, so it is that day, and reading the system clock
/// would only add ways to disagree with the file -- a wrong date, or a run
/// finishing either side of midnight.
///
/// Settling for an older day is safe anyway. A scenario's length is fixed;
/// only the number of runs changes from one day to the next.
fn length_of(log: &TrainingLog, scenario: &str) -> Option<Duration> {
    log.iter()
        .filter_map(|(day, scenarios)| Some((day_date(day), scenarios.get(scenario)?.length()?)))
        // `None` sorts below any date, so an unreadable key answers only once
        // no dated day can.
        .max_by_key(|(date, _)| *date)
        .map(|(_, length)| length)
}

/// Aimbeast keys its days as `d/m/yyyy`, without padding.
fn day_date(key: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(key, "%d/%m/%Y").ok()
}

#[cfg(test)]
mod tests {
    use super::{
        TrainingLog, day_date, length_of, log_files, parse_log, scenario_length,
        training_data_folder, year_of,
    };
    use chrono::NaiveDate;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn log(json: &str) -> TrainingLog {
        parse_log(json.as_bytes()).expect("parsable log")
    }

    fn temp_folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("kovobs_training_data_{name}"));

        _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).expect("created folder");

        folder
    }

    #[test]
    fn a_day_key_is_unpadded_day_month_year() {
        assert_eq!(day_date("5/9/2026"), Some(date(2026, 9, 5)));
        assert_eq!(day_date("19/12/2026"), Some(date(2026, 12, 19)));
    }

    #[test]
    fn a_key_that_is_not_a_day_has_no_date() {
        assert_eq!(day_date("19/12"), None);
        assert_eq!(day_date("19/12/2026/1"), None);
        assert_eq!(day_date("31/2/2026"), None);
        assert_eq!(day_date("yesterday"), None);
    }

    #[test]
    fn the_training_log_sits_beside_the_statistics() {
        assert_eq!(
            training_data_folder(Path::new("/Aimbeast/Trainer/Statistics")),
            Some(Path::new("/Aimbeast/Trainer/Training Data").to_path_buf())
        );
    }

    #[test]
    fn the_year_comes_from_the_file_name() {
        assert_eq!(
            year_of(Path::new("/logs/training_data_2026.json")),
            Some(2026)
        );
        assert_eq!(year_of(Path::new("/logs/training_data_backup.json")), None);
        assert_eq!(year_of(Path::new("/logs/something_else.json")), None);
    }

    /// A restored or copied folder carries misleading timestamps, so the year
    /// in the name decides the order.
    #[test]
    fn the_logs_are_ordered_by_year_newest_first() {
        let folder = temp_folder("ordered_by_year");

        // Created oldest-year-last, so a modification time order would differ.
        for year in ["2025", "2027", "2026"] {
            std::fs::write(folder.join(format!("training_data_{year}.json")), "{}")
                .expect("written log");
        }

        std::fs::write(folder.join("training_data_backup.json"), "{}").expect("written log");
        std::fs::write(folder.join("notes.txt"), "").expect("written file");

        let found: Vec<_> = log_files(&folder)
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();

        assert_eq!(
            found,
            [
                "training_data_2027.json",
                "training_data_2026.json",
                "training_data_2025.json"
            ]
        );

        std::fs::remove_dir_all(&folder).expect("cleaned up");
    }

    #[test]
    fn a_folder_without_logs_has_none() {
        assert!(log_files(Path::new("/does/not/exist")).is_empty());
    }

    /// Nothing to derive a sibling folder from.
    #[test]
    fn a_stats_folder_without_a_parent_has_no_training_log() {
        assert_eq!(training_data_folder(Path::new("")), None);
        assert_eq!(scenario_length(Path::new(""), "TEST"), None);
    }

    /// The length of a run is the completed time shared between the completed
    /// runs, not the total time, which also counts the abandoned ones.
    #[test]
    fn the_length_is_the_completed_time_per_completed_session() {
        let log = log(
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":120,"Completed Sessions":2,"Total Time":167}}}"#,
        );

        assert_eq!(length_of(&log, "TEST"), Some(Duration::from_secs(60)));
    }

    /// The run being clipped was just written, so the newest day is the one
    /// that describes it. Days are compared as dates, not as the strings they
    /// are stored as, where "5/9" would outrank "19/9".
    #[test]
    fn the_newest_day_wins() {
        let log = log(r#"{
                "5/9/2026":{"TEST":{"Completed Sessions Time":30,"Completed Sessions":2,"Total Time":30}},
                "19/9/2026":{"TEST":{"Completed Sessions Time":120,"Completed Sessions":2,"Total Time":120}}
            }"#);

        assert_eq!(length_of(&log, "TEST"), Some(Duration::from_secs(60)));
    }

    /// A scenario length is fixed, so an older day answers just as well.
    #[test]
    fn an_older_day_answers_when_it_is_all_there_is() {
        let log = log(
            r#"{"18/9/2026":{"TEST":{"Completed Sessions Time":30,"Completed Sessions":2,"Total Time":49}}}"#,
        );

        assert_eq!(length_of(&log, "TEST"), Some(Duration::from_secs(15)));
    }

    /// Time spent without finishing a run says nothing about its length, so
    /// the newest day that did finish one answers instead.
    #[test]
    fn a_day_with_no_completed_session_is_skipped() {
        let log = log(r#"{
                "19/9/2026":{"TEST":{"Completed Sessions Time":0,"Completed Sessions":0,"Total Time":12}},
                "18/9/2026":{"TEST":{"Completed Sessions Time":60,"Completed Sessions":1,"Total Time":60}}
            }"#);

        assert_eq!(length_of(&log, "TEST"), Some(Duration::from_secs(60)));
    }

    /// A key that is not a date is the last thing to fall back on, never the
    /// newest day.
    #[test]
    fn a_dated_day_beats_an_undated_one() {
        let log = log(r#"{
                "whenever":{"TEST":{"Completed Sessions Time":30,"Completed Sessions":2,"Total Time":30}},
                "18/9/2026":{"TEST":{"Completed Sessions Time":120,"Completed Sessions":2,"Total Time":120}}
            }"#);

        assert_eq!(length_of(&log, "TEST"), Some(Duration::from_secs(60)));
    }

    #[test]
    fn an_undated_day_is_still_better_than_nothing() {
        let log = log(
            r#"{"whenever":{"TEST":{"Completed Sessions Time":30,"Completed Sessions":2,"Total Time":30}}}"#,
        );

        assert_eq!(length_of(&log, "TEST"), Some(Duration::from_secs(15)));
    }

    /// Only ever played, never finished: the caller falls back instead.
    #[test]
    fn a_scenario_that_was_never_completed_has_no_length() {
        let log = log(
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":0,"Completed Sessions":0,"Total Time":12}}}"#,
        );

        assert_eq!(length_of(&log, "TEST"), None);
    }

    #[test]
    fn an_unplayed_scenario_has_no_length() {
        let log = log(
            r#"{"19/9/2026":{"OTHER":{"Completed Sessions Time":60,"Completed Sessions":1,"Total Time":60}}}"#,
        );

        assert_eq!(length_of(&log, "TEST"), None);
    }

    /// Aimbeast writes its logs as UTF-16, which is not valid UTF-8.
    #[test]
    fn a_utf_16_log_is_decoded() {
        let json = r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":60,"Completed Sessions":1,"Total Time":60}}}"#;

        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(json.encode_utf16().flat_map(u16::to_le_bytes));

        let log = parse_log(bytes.as_slice()).expect("decoded log");

        assert_eq!(length_of(&log, "TEST"), Some(Duration::from_secs(60)));
    }

    #[test]
    fn a_log_that_is_not_json_is_ignored() {
        assert!(parse_log(b"not json".as_slice()).is_none());
    }
}

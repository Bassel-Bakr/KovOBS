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
    /// The average length of the day's completed runs.
    ///
    /// Only as good as its assumption, that the scenario always runs the same
    /// length. Prefer [`ScenarioDay::length_since`], which measures the run
    /// itself.
    fn average_length(&self) -> Option<Duration> {
        self.length_over(self.completed_sessions_time, self.completed_sessions)
    }

    /// The length of the runs completed since `earlier`, which is one run when
    /// this is read once per run.
    ///
    /// Scenarios that end early -- on a miss, say -- last a different time
    /// every run, so the day's average describes none of them. The totals only
    /// move when a run completes, and by exactly that run's duration, so the
    /// difference between two readings is the truth about the runs between
    /// them.
    fn length_since(&self, earlier: &Self) -> Option<Duration> {
        let runs = self
            .completed_sessions
            .checked_sub(earlier.completed_sessions)?;

        self.length_over(
            self.completed_sessions_time - earlier.completed_sessions_time,
            runs,
        )
    }

    fn length_over(&self, seconds: f64, runs: u32) -> Option<Duration> {
        if runs == 0 {
            return None;
        }

        let seconds = seconds / f64::from(runs);

        (seconds.is_finite() && seconds > 0.0).then(|| Duration::from_secs_f64(seconds))
    }
}

/// How long each run of a scenario lasted, measured rather than assumed.
///
/// The statistics file that triggers a clip carries no duration, so the length
/// comes from the sibling `Training Data` folder, where Aimbeast records, per
/// day and per scenario, how much time went into completed sessions and how
/// many of those there were. Keeping the totals from the previous run turns
/// that into the length of the run in hand, which is what scenarios that can
/// end early need.
///
/// Aimbeast writes the training log just before the statistics file, so the run
/// being clipped is already counted by the time this is read.
#[derive(Debug, Default)]
pub struct ScenarioLengths(HashMap<String, (String, ScenarioDay)>);

impl ScenarioLengths {
    /// The length of the run just written, and remembers the totals behind it.
    ///
    /// Falls back to the day's average when there is nothing to compare
    /// against: the first run of a scenario since the app started, or since the
    /// day rolled over. When runs were missed -- the app was closed for some of
    /// them -- the difference covers all of them at once, and their average is
    /// still closer than the whole day's.
    pub fn last_run_length(&mut self, stats_folder: &Path, scenario: &str) -> Option<Duration> {
        let (day, totals) = latest_totals(stats_folder, scenario)?;

        let length = match self.0.get(scenario) {
            Some((seen_day, seen)) if *seen_day == day => totals.length_since(seen),
            _ => None,
        }
        .or_else(|| totals.average_length());

        self.0.insert(scenario.to_owned(), (day, totals));

        length
    }
}

/// The scenario's totals for the newest day the logs have them on.
fn latest_totals(stats_folder: &Path, scenario: &str) -> Option<(String, ScenarioDay)> {
    // A run on New Year's Day is the first entry of its year, so the newest log
    // is not guaranteed to know the scenario and the older ones are worth a
    // look before giving up.
    log_files(&training_data_folder(stats_folder)?)
        .into_iter()
        .filter_map(|path| read_log(&path))
        .find_map(|log| totals_in(&log, scenario))
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

/// The scenario's totals on the latest day it was completed on, with that day's
/// key.
///
/// The log's own newest day decides, rather than today's date: the run being
/// clipped was just written, so it is that day, and reading the system clock
/// would only add ways to disagree with the file -- a wrong date, or a run
/// finishing either side of midnight.
///
/// The key comes back because the totals restart each day, so a reading is only
/// comparable with another from the same day.
fn totals_in(log: &TrainingLog, scenario: &str) -> Option<(String, ScenarioDay)> {
    log.iter()
        .filter(|(_, scenarios)| {
            scenarios
                .get(scenario)
                .is_some_and(|day| day.average_length().is_some())
        })
        // `None` sorts below any date, so an unreadable key answers only once
        // no dated day can.
        .max_by_key(|(day, _)| day_date(day))
        .map(|(day, scenarios)| (day.clone(), scenarios[scenario]))
}

/// Aimbeast keys its days as `d/m/yyyy`, without padding.
fn day_date(key: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(key, "%d/%m/%Y").ok()
}

#[cfg(test)]
mod tests {
    use super::{
        ScenarioDay, ScenarioLengths, TrainingLog, day_date, log_files, parse_log, totals_in,
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

    /// The day's average, which is what the caller falls back on when it has no
    /// earlier reading to measure against.
    fn average(log: &TrainingLog, scenario: &str) -> Option<Duration> {
        totals_in(log, scenario)?.1.average_length()
    }

    fn totals(seconds: f64, runs: u32) -> ScenarioDay {
        ScenarioDay {
            completed_sessions_time: seconds,
            completed_sessions: runs,
        }
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
        assert_eq!(
            ScenarioLengths::default().last_run_length(Path::new(""), "TEST"),
            None
        );
    }

    /// The length of a run is the completed time shared between the completed
    /// runs, not the total time, which also counts the abandoned ones.
    #[test]
    fn the_length_is_the_completed_time_per_completed_session() {
        let log = log(
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":120,"Completed Sessions":2,"Total Time":167}}}"#,
        );

        assert_eq!(average(&log, "TEST"), Some(Duration::from_secs(60)));
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

        assert_eq!(average(&log, "TEST"), Some(Duration::from_secs(60)));
    }

    /// What the averaging misses. Aimbeast's `SPHERE FRENZY BAZ 10S 1SHOT` ends
    /// on a miss, so 14 completed runs came to 93 seconds of a nominally 10
    /// second scenario -- an average of 6.64 that describes none of them.
    #[test]
    fn a_run_is_measured_against_the_reading_before_it() {
        let before = totals(93.0, 14);
        let after = totals(97.0, 15);

        assert_eq!(after.length_since(&before), Some(Duration::from_secs(4)));
        assert_ne!(after.average_length(), after.length_since(&before));
    }

    /// The app was closed for some of them, so they come back as their own
    /// average rather than the day's.
    #[test]
    fn runs_missed_together_average_only_among_themselves() {
        let before = totals(30.0, 2);
        let after = totals(60.0, 4);

        assert_eq!(after.length_since(&before), Some(Duration::from_secs(15)));
    }

    /// Nothing completed since, so there is no run to measure.
    #[test]
    fn an_unchanged_reading_measures_nothing() {
        let totals = totals(60.0, 4);

        assert_eq!(totals.length_since(&totals), None);
    }

    /// Totals only ever climb within a day. Going backwards means the reading
    /// is from another day or another install, not a run.
    #[test]
    fn a_reading_that_went_backwards_measures_nothing() {
        assert_eq!(totals(30.0, 2).length_since(&totals(60.0, 4)), None);
        assert_eq!(totals(0.0, 3).length_since(&totals(60.0, 2)), None);
    }

    /// Only the fallback, and a weak one on an older day, but better than
    /// assuming a minute.
    #[test]
    fn an_older_day_answers_when_it_is_all_there_is() {
        let log = log(
            r#"{"18/9/2026":{"TEST":{"Completed Sessions Time":30,"Completed Sessions":2,"Total Time":49}}}"#,
        );

        assert_eq!(average(&log, "TEST"), Some(Duration::from_secs(15)));
    }

    /// Time spent without finishing a run says nothing about its length, so
    /// the newest day that did finish one answers instead.
    #[test]
    fn a_day_with_no_completed_session_is_skipped() {
        let log = log(r#"{
                "19/9/2026":{"TEST":{"Completed Sessions Time":0,"Completed Sessions":0,"Total Time":12}},
                "18/9/2026":{"TEST":{"Completed Sessions Time":60,"Completed Sessions":1,"Total Time":60}}
            }"#);

        assert_eq!(average(&log, "TEST"), Some(Duration::from_secs(60)));
    }

    /// A key that is not a date is the last thing to fall back on, never the
    /// newest day.
    #[test]
    fn a_dated_day_beats_an_undated_one() {
        let log = log(r#"{
                "whenever":{"TEST":{"Completed Sessions Time":30,"Completed Sessions":2,"Total Time":30}},
                "18/9/2026":{"TEST":{"Completed Sessions Time":120,"Completed Sessions":2,"Total Time":120}}
            }"#);

        assert_eq!(average(&log, "TEST"), Some(Duration::from_secs(60)));
    }

    #[test]
    fn an_undated_day_is_still_better_than_nothing() {
        let log = log(
            r#"{"whenever":{"TEST":{"Completed Sessions Time":30,"Completed Sessions":2,"Total Time":30}}}"#,
        );

        assert_eq!(average(&log, "TEST"), Some(Duration::from_secs(15)));
    }

    /// Only ever played, never finished: the caller falls back instead.
    #[test]
    fn a_scenario_that_was_never_completed_has_no_length() {
        let log = log(
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":0,"Completed Sessions":0,"Total Time":12}}}"#,
        );

        assert_eq!(average(&log, "TEST"), None);
    }

    #[test]
    fn an_unplayed_scenario_has_no_length() {
        let log = log(
            r#"{"19/9/2026":{"OTHER":{"Completed Sessions Time":60,"Completed Sessions":1,"Total Time":60}}}"#,
        );

        assert_eq!(average(&log, "TEST"), None);
    }

    /// Aimbeast writes its logs as UTF-16, which is not valid UTF-8.
    #[test]
    fn a_utf_16_log_is_decoded() {
        let json = r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":60,"Completed Sessions":1,"Total Time":60}}}"#;

        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(json.encode_utf16().flat_map(u16::to_le_bytes));

        let log = parse_log(bytes.as_slice()).expect("decoded log");

        assert_eq!(average(&log, "TEST"), Some(Duration::from_secs(60)));
    }

    /// Writes a log where a real install keeps one, and gives back the stats
    /// folder beside it that the caller passes in.
    fn install_with_log(name: &str, json: &str) -> PathBuf {
        let root = temp_folder(name);
        let logs = root.join("Training Data");

        std::fs::create_dir_all(&logs).expect("created folder");
        std::fs::write(logs.join("training_data_2026.json"), json).expect("written log");

        root.join("Statistics")
    }

    fn rewrite_log(stats_folder: &Path, json: &str) {
        let log = stats_folder
            .parent()
            .expect("a parent")
            .join("Training Data")
            .join("training_data_2026.json");

        std::fs::write(log, json).expect("rewritten log");
    }

    /// The first run of a scenario has nothing before it, so the day's average
    /// stands in. Every run after it is measured.
    #[test]
    fn the_first_run_falls_back_and_the_next_is_measured() {
        let stats_folder = install_with_log(
            "measured",
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":93,"Completed Sessions":14,"Total Time":93}}}"#,
        );

        let mut lengths = ScenarioLengths::default();

        // 93 / 14, the average of runs that each ended early.
        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST"),
            Some(Duration::from_secs_f64(93.0 / 14.0))
        );

        rewrite_log(
            &stats_folder,
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":97,"Completed Sessions":15,"Total Time":97}}}"#,
        );

        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST"),
            Some(Duration::from_secs(4))
        );

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    /// The totals restart each day, so yesterday's reading would measure a run
    /// as the whole of today. The average stands in instead.
    #[test]
    fn a_reading_from_another_day_is_not_measured_against() {
        let stats_folder = install_with_log(
            "day_rollover",
            r#"{"18/9/2026":{"TEST":{"Completed Sessions Time":120,"Completed Sessions":2,"Total Time":120}}}"#,
        );

        let mut lengths = ScenarioLengths::default();

        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST"),
            Some(Duration::from_secs(60))
        );

        rewrite_log(
            &stats_folder,
            r#"{
                "18/9/2026":{"TEST":{"Completed Sessions Time":120,"Completed Sessions":2,"Total Time":120}},
                "19/9/2026":{"TEST":{"Completed Sessions Time":15,"Completed Sessions":1,"Total Time":15}}
            }"#,
        );

        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST"),
            Some(Duration::from_secs(15))
        );

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    /// One scenario's runs must not be measured against another's totals.
    #[test]
    fn scenarios_are_remembered_apart() {
        let stats_folder = install_with_log(
            "per_scenario",
            r#"{"19/9/2026":{
                "TEST":{"Completed Sessions Time":60,"Completed Sessions":1,"Total Time":60},
                "OTHER":{"Completed Sessions Time":15,"Completed Sessions":1,"Total Time":15}
            }}"#,
        );

        let mut lengths = ScenarioLengths::default();

        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST"),
            Some(Duration::from_secs(60))
        );
        assert_eq!(
            lengths.last_run_length(&stats_folder, "OTHER"),
            Some(Duration::from_secs(15))
        );

        rewrite_log(
            &stats_folder,
            r#"{"19/9/2026":{
                "TEST":{"Completed Sessions Time":60,"Completed Sessions":1,"Total Time":60},
                "OTHER":{"Completed Sessions Time":25,"Completed Sessions":2,"Total Time":25}
            }}"#,
        );

        assert_eq!(
            lengths.last_run_length(&stats_folder, "OTHER"),
            Some(Duration::from_secs(10))
        );

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    #[test]
    fn a_log_that_is_not_json_is_ignored() {
        assert!(parse_log(b"not json".as_slice()).is_none());
    }
}

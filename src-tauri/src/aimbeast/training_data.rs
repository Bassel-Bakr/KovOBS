//! Aimbeast's training log, and the scenario length hidden inside it.

use chrono::{DateTime, NaiveDate, Utc};
use encoding_rs_io::DecodeReaderBytesBuilder;
use serde::Deserialize;
use std::cmp::Reverse;
use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Used when the training log cannot say how long the scenario runs. Most
/// Aimbeast scenarios are a minute long.
pub const DEFAULT_SCENARIO_LENGTH: Duration = Duration::from_mins(1);

/// The longest run Aimbeast leaves out of the training log.
///
/// It saves a run's length rounded to whole seconds, and only when that is
/// over 3 (`Save Scenario To Training Streak` checks `Time > 3.0`). So a run
/// that ended without a write to the log was under 3.5 seconds, not counting
/// pauses.
pub const UNCOUNTED_RUN_MAX: Duration = Duration::from_millis(3500);

/// How close to a run's end its own training-log write lands. Aimbeast writes
/// the log a few milliseconds before the statistics file.
const OWN_WRITE_SLACK: Duration = Duration::from_millis(250);

/// What is known about a run as it ends, beyond the training log.
#[derive(Clone, Copy, Debug)]
pub struct RunEnd {
    /// When the statistics file was written.
    pub at: DateTime<Utc>,
    /// First kill to last, from the statistics file. See
    /// [`crate::aimbeast::ScenarioStatistics::kill_span`].
    pub kill_span: Option<Duration>,
    /// The training-log write before this run's own. See [`TrainingWrites`].
    pub previous_write: Option<DateTime<Utc>>,
    /// Whether the training log was written as the run ended, when the log is
    /// being watched. Asked of the write rather than the totals: a ranked run
    /// is logged as `<name> - RANKED` while its statistics file drops the
    /// suffix, so the totals looked up for it may be another scenario's.
    pub counted: Option<bool>,
}

impl From<DateTime<Utc>> for RunEnd {
    fn from(at: DateTime<Utc>) -> Self {
        Self {
            at,
            kill_span: None,
            previous_write: None,
            counted: None,
        }
    }
}

/// When Aimbeast wrote its training log, timed as the writes happen.
///
/// The log is written when a run of over 3 seconds ends, and when one is
/// restarted from inside -- fast restart, the pause menu's Restart, or Smart
/// Restart. A run therefore started after the write before its own, whichever
/// scenario that write was for.
///
/// The times have to be taken as the writes happen. By the time a run is
/// handled, the file's own timestamp is that run's, and every earlier write
/// is gone from it.
#[derive(Clone, Debug, Default)]
pub struct TrainingWrites(Arc<Mutex<VecDeque<DateTime<Utc>>>>);

impl TrainingWrites {
    const KEPT: usize = 32;

    pub fn record(&self, at: DateTime<Utc>) {
        let mut writes = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        // One write arrives as several file events.
        if writes.back() == Some(&at) {
            return;
        }

        writes.push_back(at);

        if writes.len() > Self::KEPT {
            writes.pop_front();
        }
    }

    /// Whether the run that ended at `ended` wrote the log as it ended, which
    /// it does when it lasted over 3 seconds.
    pub fn wrote_at(&self, ended: DateTime<Utc>) -> bool {
        let writes = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        writes.iter().any(|&at| {
            (ended - at)
                .abs()
                .to_std()
                .is_ok_and(|gap| gap < OWN_WRITE_SLACK)
        })
    }

    /// The latest write before the run that ended at `ended`, leaving out the
    /// run's own.
    pub fn last_before(&self, ended: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let writes = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        writes
            .iter()
            .copied()
            .filter(|&at| at + OWN_WRITE_SLACK < ended)
            .max()
    }
}

const LOG_FOLDER: &str = "Training Data";
const LOG_PREFIX: &str = "training_data_";

/// What a ranked run's name carries in the log and loses in its statistics
/// file name. See [`logged_name`].
const RANKED_SUFFIX: &str = " - RANKED";

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
    /// Counted runs of one scenario do not all last the same time, so the day's
    /// average can describe none of them. The totals only move when a run is
    /// counted, and by exactly that run's duration, so the difference between
    /// two readings is the truth about the runs between them.
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
pub struct ScenarioLengths(HashMap<String, Seen>);

/// What the last look at a scenario found.
#[derive(Clone, Debug)]
struct Seen {
    day: String,
    totals: ScenarioDay,
    /// When its previous run ended, once one has been watched. Priming leaves
    /// this empty: it reads totals, not runs.
    ended: Option<DateTime<Utc>>,
}

impl ScenarioLengths {
    /// Takes a reading of every scenario before any run is watched.
    ///
    /// Without it the first run of each scenario after the app starts has
    /// nothing to measure against and settles for the day's average, which is
    /// worst exactly where measuring matters: a scenario whose runs vary was
    /// seen at 6.4 seconds averaged over the day while the run in hand lasted
    /// 12.
    ///
    /// Only reruns of a day already underway need this. The first run of a new
    /// day writes `1` completed session, so the average of that day is the run
    /// itself.
    pub fn prime(&mut self, stats_folder: &Path) {
        let Some(folder) = training_data_folder(stats_folder) else {
            return;
        };

        for log in log_files(&folder).iter().filter_map(|path| read_log(path)) {
            for (day, scenarios) in &log {
                for (scenario, totals) in scenarios {
                    if totals.average_length().is_none() {
                        continue;
                    }

                    let newer_than_seen = self
                        .0
                        .get(scenario)
                        .is_none_or(|seen| day_date(&seen.day) < day_date(day));

                    if newer_than_seen {
                        self.0.insert(
                            scenario.clone(),
                            Seen {
                                day: day.clone(),
                                totals: *totals,
                                ended: None,
                            },
                        );
                    }
                }
            }
        }
    }

    /// The length of the run that ended at `end`, and remembers what it was
    /// worked out from.
    ///
    /// Every run of over 3 seconds is counted, however it ended -- the timer,
    /// a kill or miss limit, dying -- so the difference from the previous
    /// reading is the run itself. When runs were missed (the app was closed for
    /// some of them), the difference covers all of them at once, and their
    /// average is still closer than the whole day's. A run that ended without
    /// writing the log was left out of it, which Aimbeast does only below
    /// [`UNCOUNTED_RUN_MAX`].
    ///
    /// With nothing to compare against -- the first run of a scenario since the
    /// day rolled over, or one the log has never counted -- the answer is the
    /// longer of the day's average and the run's kill span. The average is
    /// exact for a scenario that always runs its timer out and wrong for one
    /// that can end early. The kill span is a floor under the run. Taking the
    /// longer keeps the clip from losing the start of the run.
    ///
    /// Whatever the source, the answer is capped by the time since this
    /// scenario's previous run ended, and by the time since the training-log
    /// write before this run's own. The player started the run after both, so
    /// it cannot have outlasted either. A cap is only ever an upper bound, so it
    /// can shorten the answer toward the truth but never past it -- unless
    /// Aimbeast writes the same run twice, further apart than the watcher's
    /// debounce, which would read as two runs a moment apart.
    pub fn last_run_length(
        &mut self,
        stats_folder: &Path,
        scenario: &str,
        end: impl Into<RunEnd>,
    ) -> Option<Duration> {
        let end = end.into();
        let latest = latest_totals(stats_folder, scenario);
        let seen = self.0.get(scenario);

        let measured = match (seen, &latest) {
            (Some(seen), Some((day, totals))) if seen.day == *day => {
                totals.length_since(&seen.totals)
            }
            _ => None,
        };

        let uncounted = (end.counted == Some(false)).then_some(UNCOUNTED_RUN_MAX);

        let fallback = || {
            latest
                .as_ref()
                .and_then(|(_, totals)| totals.average_length())
                .into_iter()
                .chain(end.kill_span)
                .max()
        };

        let since_previous_run = seen
            .and_then(|seen| seen.ended)
            .and_then(|before| (end.at - before).to_std().ok());

        let since_previous_write = end
            .previous_write
            .and_then(|before| (end.at - before).to_std().ok());

        let length = measured.or(uncounted).or_else(fallback).map(|length| {
            since_previous_run
                .into_iter()
                .chain(since_previous_write)
                .fold(length, Duration::min)
        });

        if let Some((day, totals)) = latest {
            self.0.insert(
                scenario.to_owned(),
                Seen {
                    day,
                    totals,
                    ended: Some(end.at),
                },
            );
        }

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

/// The name a run of `scenario` is logged under in the training log.
///
/// A ranked run is logged as `<name> - RANKED`, while its statistics file
/// under `Ranked/` drops the suffix: Aimbeast saves the log under the
/// scenario's full name and builds the statistics path with
/// `Replace(ScenarioName, " - RANKED", "")`. Looked up by the file name, a
/// ranked run would be measured against the Normal scenario's totals, or none.
///
/// Not every scenario filed under `Ranked/` is logged with the suffix, so the
/// suffixed name is taken only when the log has it, and only when it was
/// played no earlier than the plain name. A run that was just counted was
/// written today, so its own name is the newer of the two.
pub fn logged_name(stats_folder: &Path, scenario: &str, ranked: bool) -> String {
    if !ranked {
        return scenario.to_owned();
    }

    let suffixed = format!("{scenario}{RANKED_SUFFIX}");
    let newest_day =
        |name: &str| latest_totals(stats_folder, name).and_then(|(day, _)| day_date(&day));

    match (newest_day(&suffixed), newest_day(scenario)) {
        (Some(ranked_day), plain_day) if plain_day <= Some(ranked_day) => suffixed,
        _ => scenario.to_owned(),
    }
}

/// The training log sits beside the statistics, both under Aimbeast's
/// `Trainer` folder.
pub fn training_data_folder(stats_folder: &Path) -> Option<PathBuf> {
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

/// Whether the file is one of the yearly training logs, rather than anything
/// else that shares their folder, such as Steam Cloud's bookkeeping.
pub fn is_training_log(path: &Path) -> bool {
    year_of(path).is_some()
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
        RunEnd, ScenarioDay, ScenarioLengths, TrainingLog, TrainingWrites, UNCOUNTED_RUN_MAX,
        day_date, log_files, logged_name, parse_log, totals_in, training_data_folder, year_of,
    };
    use chrono::{DateTime, NaiveDate, Utc};
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

    /// A moment far enough after the previous one that the cap never applies,
    /// for the tests that are not about the cap.
    fn much_later() -> DateTime<Utc> {
        static NEXT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
        let step = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        DateTime::from_timestamp(1_700_000_000 + step * 3600, 0).expect("valid moment")
    }

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_800_000_000 + seconds, 0).expect("valid moment")
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
            ScenarioLengths::default().last_run_length(Path::new(""), "TEST", much_later()),
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

    /// What the averaging misses. Counted runs of a scenario do not all last
    /// the same time -- `SPHERE FRENZY BAZ 10S 1SHOT` has 14 of them totalling
    /// 93 seconds, an average of 6.64 that describes none of them -- so the run
    /// in hand is worth measuring rather than assuming.
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
            lengths.last_run_length(&stats_folder, "TEST", much_later()),
            Some(Duration::from_secs_f64(93.0 / 14.0))
        );

        rewrite_log(
            &stats_folder,
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":97,"Completed Sessions":15,"Total Time":97}}}"#,
        );

        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST", much_later()),
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
            lengths.last_run_length(&stats_folder, "TEST", much_later()),
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
            lengths.last_run_length(&stats_folder, "TEST", much_later()),
            Some(Duration::from_secs(15))
        );

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    /// The case that sent a 12 second run out as 6.4: the app restarts partway
    /// through a day, and without a reading taken up front the first run of
    /// each scenario settles for the day's average.
    #[test]
    fn priming_measures_the_first_run_after_a_restart() {
        let stats_folder = install_with_log(
            "primed",
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":20,"Completed Sessions":4,"Total Time":20}}}"#,
        );

        let mut lengths = ScenarioLengths::default();
        lengths.prime(&stats_folder);

        rewrite_log(
            &stats_folder,
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":32,"Completed Sessions":5,"Total Time":32}}}"#,
        );

        // 32/5 is 6.4; the run itself was 12.
        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST", much_later()),
            Some(Duration::from_secs(12))
        );

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    /// Priming reads whatever is there, so a day that has not started yet
    /// leaves the first run of it to the average -- which is that run itself.
    #[test]
    fn priming_an_older_day_still_leaves_the_new_day_to_itself() {
        let stats_folder = install_with_log(
            "primed_older_day",
            r#"{"18/9/2026":{"TEST":{"Completed Sessions Time":20,"Completed Sessions":4,"Total Time":20}}}"#,
        );

        let mut lengths = ScenarioLengths::default();
        lengths.prime(&stats_folder);

        rewrite_log(
            &stats_folder,
            r#"{
                "18/9/2026":{"TEST":{"Completed Sessions Time":20,"Completed Sessions":4,"Total Time":20}},
                "19/9/2026":{"TEST":{"Completed Sessions Time":12,"Completed Sessions":1,"Total Time":12}}
            }"#,
        );

        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST", much_later()),
            Some(Duration::from_secs(12))
        );

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    /// Each scenario is primed from its own newest day, not one shared date.
    #[test]
    fn priming_takes_each_scenario_from_its_own_newest_day() {
        let stats_folder = install_with_log(
            "primed_per_scenario",
            r#"{
                "18/9/2026":{
                    "TEST":{"Completed Sessions Time":20,"Completed Sessions":4,"Total Time":20},
                    "OTHER":{"Completed Sessions Time":30,"Completed Sessions":2,"Total Time":30}
                },
                "19/9/2026":{"TEST":{"Completed Sessions Time":99,"Completed Sessions":9,"Total Time":99}}
            }"#,
        );

        let mut lengths = ScenarioLengths::default();
        lengths.prime(&stats_folder);

        rewrite_log(
            &stats_folder,
            r#"{
                "18/9/2026":{
                    "TEST":{"Completed Sessions Time":20,"Completed Sessions":4,"Total Time":20},
                    "OTHER":{"Completed Sessions Time":45,"Completed Sessions":3,"Total Time":45}
                },
                "19/9/2026":{"TEST":{"Completed Sessions Time":109,"Completed Sessions":10,"Total Time":109}}
            }"#,
        );

        // TEST was primed from 19/9 at 99/9, not from 18/9.
        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST", much_later()),
            Some(Duration::from_secs(10))
        );
        assert_eq!(
            lengths.last_run_length(&stats_folder, "OTHER", much_later()),
            Some(Duration::from_secs(15))
        );

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    /// Aimbeast leaves a run out of the log only when it lasted 3 seconds or
    /// less, rounded, so a run that ended without a write was under 3.5.
    #[test]
    fn a_run_the_log_left_out_was_under_three_and_a_half_seconds() {
        let stats_folder = install_with_log(
            "uncounted",
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":60,"Completed Sessions":2,"Total Time":60}}}"#,
        );

        let mut lengths = ScenarioLengths::default();

        // Nothing before it, so the average stands: a 30 second timer.
        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST", at(0)),
            Some(Duration::from_secs(30))
        );

        // Cleared a minute later, and the log was not written.
        let end = RunEnd {
            counted: Some(false),
            ..RunEnd::from(at(60))
        };
        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST", end),
            Some(UNCOUNTED_RUN_MAX)
        );

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    /// The log was written, but not under this name -- a ranked run is logged
    /// as `<name> - RANKED` -- so the unmoved totals say nothing about the run.
    #[test]
    fn unmoved_totals_after_a_write_are_not_taken_for_a_short_run() {
        let stats_folder = install_with_log(
            "counted_elsewhere",
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":120,"Completed Sessions":2,"Total Time":120}}}"#,
        );

        let mut lengths = ScenarioLengths::default();
        lengths.prime(&stats_folder);

        let end = RunEnd {
            counted: Some(true),
            ..RunEnd::from(much_later())
        };
        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST", end),
            Some(Duration::from_secs(60))
        );

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    /// The run cannot have outlasted the gap since the previous one ended.
    #[test]
    fn a_run_cannot_be_longer_than_the_gap_since_the_previous_one() {
        let stats_folder = install_with_log(
            "capped",
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":60,"Completed Sessions":2,"Total Time":60}}}"#,
        );

        let mut lengths = ScenarioLengths::default();

        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST", at(0)),
            Some(Duration::from_secs(30))
        );

        // Two seconds later, and the totals did not move, so the average would
        // say 30 again.
        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST", at(2)),
            Some(Duration::from_secs(2))
        );

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    /// The first run a log never counted -- no reading before it, no day of
    /// its own -- has only its kill span to go on, rather than a minute.
    #[test]
    fn a_run_the_log_never_counted_takes_its_kill_span() {
        let stats_folder = install_with_log("kill_span_only", r#"{}"#);

        let mut lengths = ScenarioLengths::default();
        let end = RunEnd {
            kill_span: Some(Duration::from_secs(42)),
            ..RunEnd::from(at(0))
        };

        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST", end),
            Some(Duration::from_secs(42))
        );

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    /// Without a reading to measure against, the longer of the average and the
    /// kill span stands: the span is a floor under the run, the average is
    /// right only for a run that went the distance.
    #[test]
    fn the_fallback_is_the_longer_of_the_average_and_the_kill_span() {
        let stats_folder = install_with_log(
            "fallback_longer",
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":93,"Completed Sessions":14,"Total Time":93}}}"#,
        );

        let longer = RunEnd {
            kill_span: Some(Duration::from_secs(9)),
            ..RunEnd::from(much_later())
        };
        assert_eq!(
            ScenarioLengths::default().last_run_length(&stats_folder, "TEST", longer),
            Some(Duration::from_secs(9))
        );

        let shorter = RunEnd {
            kill_span: Some(Duration::from_secs(2)),
            ..RunEnd::from(much_later())
        };
        assert_eq!(
            ScenarioLengths::default().last_run_length(&stats_folder, "TEST", shorter),
            Some(Duration::from_secs_f64(93.0 / 14.0))
        );

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    /// A measured run is exact, so a kill span does not move it.
    #[test]
    fn a_measured_run_ignores_the_kill_span() {
        let stats_folder = install_with_log(
            "measured_ignores_span",
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":30,"Completed Sessions":3,"Total Time":30}}}"#,
        );

        let mut lengths = ScenarioLengths::default();
        lengths.prime(&stats_folder);

        rewrite_log(
            &stats_folder,
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":36,"Completed Sessions":4,"Total Time":36}}}"#,
        );

        let end = RunEnd {
            kill_span: Some(Duration::from_secs(20)),
            ..RunEnd::from(much_later())
        };
        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST", end),
            Some(Duration::from_secs(6))
        );

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    /// A restart writes the log too, so the run that followed it cannot be
    /// longer than the time since.
    #[test]
    fn a_run_cannot_be_longer_than_the_time_since_the_previous_log_write() {
        let stats_folder = install_with_log(
            "previous_write",
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":120,"Completed Sessions":2,"Total Time":150}}}"#,
        );

        let end = RunEnd {
            previous_write: Some(at(-7)),
            ..RunEnd::from(at(0))
        };

        // The average says a minute; the restart was seven seconds ago.
        assert_eq!(
            ScenarioLengths::default().last_run_length(&stats_folder, "TEST", end),
            Some(Duration::from_secs(7))
        );

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    #[test]
    fn the_last_write_before_a_run_leaves_out_the_runs_own() {
        let writes = TrainingWrites::default();
        writes.record(at(-30));
        writes.record(at(-7));
        // The run's own write, a few milliseconds before its statistics file.
        writes.record(at(0) - Duration::from_millis(3));

        assert_eq!(writes.last_before(at(0)), Some(at(-7)));
    }

    /// `1 WALL 4 TARGETS` as played ranked: its statistics file drops the
    /// suffix, the log keeps it.
    #[test]
    fn a_ranked_run_is_looked_up_with_its_suffix() {
        let stats_folder = install_with_log(
            "ranked_suffix",
            r#"{
                "21/4/2026":{"1 WALL 4 TARGETS - RANKED":{"Completed Sessions Time":1800,"Completed Sessions":30,"Total Time":1990}},
                "19/4/2026":{"1 WALL 4 TARGETS":{"Completed Sessions Time":60,"Completed Sessions":1,"Total Time":60}}
            }"#,
        );

        assert_eq!(
            logged_name(&stats_folder, "1 WALL 4 TARGETS", true),
            "1 WALL 4 TARGETS - RANKED"
        );
        assert_eq!(
            logged_name(&stats_folder, "1 WALL 4 TARGETS", false),
            "1 WALL 4 TARGETS"
        );

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    /// Filed under `Ranked/` but logged without the suffix, as
    /// `PASU 5TGS SMOOTH V2 (2EZ)` is.
    #[test]
    fn a_ranked_scenario_logged_plainly_keeps_its_plain_name() {
        let stats_folder = install_with_log(
            "ranked_plain",
            r#"{"26/2/2026":{"PASU 5TGS SMOOTH V2 (2EZ)":{"Completed Sessions Time":300,"Completed Sessions":5,"Total Time":344}}}"#,
        );

        assert_eq!(
            logged_name(&stats_folder, "PASU 5TGS SMOOTH V2 (2EZ)", true),
            "PASU 5TGS SMOOTH V2 (2EZ)"
        );

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    /// Both names in the log: the one written more recently is the run's own.
    /// On the same day the suffix, which is where ranked runs go.
    #[test]
    fn the_more_recently_logged_name_wins() {
        let stats_folder = install_with_log(
            "ranked_newest",
            r#"{
                "23/10/2025":{"X - RANKED":{"Completed Sessions Time":60,"Completed Sessions":1,"Total Time":60}},
                "24/10/2025":{"X":{"Completed Sessions Time":60,"Completed Sessions":1,"Total Time":60}}
            }"#,
        );
        assert_eq!(logged_name(&stats_folder, "X", true), "X");

        rewrite_log(
            &stats_folder,
            r#"{"24/10/2025":{
                "X":{"Completed Sessions Time":60,"Completed Sessions":1,"Total Time":60},
                "X - RANKED":{"Completed Sessions Time":60,"Completed Sessions":1,"Total Time":60}
            }}"#,
        );
        assert_eq!(logged_name(&stats_folder, "X", true), "X - RANKED");

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    #[test]
    fn a_name_the_log_has_never_seen_stays_as_it_is() {
        let stats_folder = install_with_log("ranked_unknown", r#"{}"#);

        assert_eq!(logged_name(&stats_folder, "X", true), "X");

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    /// The run's own write lands a few milliseconds before its statistics file.
    #[test]
    fn a_run_over_three_seconds_wrote_as_it_ended() {
        let writes = TrainingWrites::default();
        writes.record(at(-30));
        writes.record(at(0) - Duration::from_millis(10));

        assert!(writes.wrote_at(at(0)));
        assert!(!writes.wrote_at(at(-10)));
    }

    #[test]
    fn no_write_before_a_run_leaves_nothing_to_cap_with() {
        let writes = TrainingWrites::default();
        writes.record(at(5));

        assert_eq!(writes.last_before(at(0)), None);
    }

    /// One write arrives as several file events, and only recent writes matter.
    #[test]
    fn writes_are_kept_once_and_only_the_latest() {
        let writes = TrainingWrites::default();
        writes.record(at(0));
        writes.record(at(0));

        assert_eq!(writes.0.lock().expect("unpoisoned").len(), 1);

        for second in 1..=100 {
            writes.record(at(second));
        }

        assert_eq!(
            writes.0.lock().expect("unpoisoned").len(),
            TrainingWrites::KEPT
        );
        assert_eq!(writes.last_before(at(1000)), Some(at(100)));
    }

    /// The cap is a ceiling, not an answer: a measured run below it is left
    /// alone.
    #[test]
    fn the_cap_never_lengthens_a_measured_run() {
        let stats_folder = install_with_log(
            "cap_leaves_measured",
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":30,"Completed Sessions":3,"Total Time":30}}}"#,
        );

        let mut lengths = ScenarioLengths::default();
        lengths.prime(&stats_folder);

        rewrite_log(
            &stats_folder,
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":36,"Completed Sessions":4,"Total Time":36}}}"#,
        );

        // Measured at 6 seconds, and 50 seconds have passed, so nothing is cut.
        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST", at(50)),
            Some(Duration::from_secs(6))
        );

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    /// Leaving the scenario and coming back later says nothing, so a wide gap
    /// leaves the fallback as it was.
    #[test]
    fn a_wide_gap_caps_nothing() {
        let stats_folder = install_with_log(
            "wide_gap",
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":60,"Completed Sessions":2,"Total Time":60}}}"#,
        );

        let mut lengths = ScenarioLengths::default();

        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST", at(0)),
            Some(Duration::from_secs(30))
        );
        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST", at(600)),
            Some(Duration::from_secs(30))
        );

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    /// Priming reads totals, not runs, so it leaves no moment to cap against.
    #[test]
    fn priming_alone_caps_nothing() {
        let stats_folder = install_with_log(
            "prime_no_cap",
            r#"{"19/9/2026":{"TEST":{"Completed Sessions Time":60,"Completed Sessions":2,"Total Time":60}}}"#,
        );

        let mut lengths = ScenarioLengths::default();
        lengths.prime(&stats_folder);

        assert_eq!(
            lengths.last_run_length(&stats_folder, "TEST", at(1)),
            Some(Duration::from_secs(30))
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
            lengths.last_run_length(&stats_folder, "TEST", much_later()),
            Some(Duration::from_secs(60))
        );
        assert_eq!(
            lengths.last_run_length(&stats_folder, "OTHER", much_later()),
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
            lengths.last_run_length(&stats_folder, "OTHER", much_later()),
            Some(Duration::from_secs(10))
        );

        std::fs::remove_dir_all(stats_folder.parent().expect("a parent")).expect("cleaned up");
    }

    #[test]
    fn a_log_that_is_not_json_is_ignored() {
        assert!(parse_log(b"not json".as_slice()).is_none());
    }
}

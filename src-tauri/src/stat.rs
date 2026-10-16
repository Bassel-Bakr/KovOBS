use crate::utils;
use chrono::{DateTime, Duration, Local, LocalResult, NaiveTime, Offset, TimeZone, Utc};
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

    /// The moment a run started, from when it ended and the local clock time
    /// KovaaK's wrote as its `Challenge Start`.
    ///
    /// The clock time has no date. The stats file is named for the run's end,
    /// so a run that crossed midnight carries the next day's date and started
    /// on the day before. Every instant showing that clock time on the end's
    /// day or the day before is a candidate, and the start is the latest one
    /// not after the end. That one rule covers a run on one day, a run across
    /// midnight, and the hour a change from summer time shows twice.
    pub fn compute_start_time(end_dt: DateTime<Utc>, time_of_day: Duration) -> DateTime<Utc> {
        Self::start_in(&Local, end_dt, time_of_day)
    }

    /// How far after the end a start may land and still be the same day.
    /// KovaaK's writes the start to the millisecond, but the file's time can
    /// trail it by a second on a run that ends as it begins: "01:31:45.253"
    /// in a file written at 01:31:44. A run that crossed midnight is a whole
    /// day out instead.
    const START_AFTER_END_SLACK: Duration = Duration::seconds(60);

    fn start_in<Tz: TimeZone>(
        tz: &Tz,
        end_dt: DateTime<Utc>,
        time_of_day: Duration,
    ) -> DateTime<Utc> {
        let end = end_dt.with_timezone(tz);
        let offset_at_end = end.offset().fix();
        let day = end.date_naive();

        std::iter::once(day)
            .chain(day.pred_opt())
            .flat_map(|date| {
                let clock = date.and_time(NaiveTime::MIN) + time_of_day;

                match tz.from_local_datetime(&clock) {
                    LocalResult::Single(start) => vec![start.with_timezone(&Utc)],
                    LocalResult::Ambiguous(first, second) => {
                        vec![first.with_timezone(&Utc), second.with_timezone(&Utc)]
                    }
                    // A clock time the change to summer time skips. KovaaK's
                    // cannot write one, but a wrong file must not take the
                    // session down, so read it with the offset at the end.
                    LocalResult::None => vec![
                        (clock - Duration::seconds(offset_at_end.local_minus_utc().into()))
                            .and_utc(),
                    ],
                }
            })
            .filter(|start| *start <= end_dt + Self::START_AFTER_END_SLACK)
            .max()
            .unwrap_or(end_dt)
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

    /// Reads a `Challenge Start` clock time, `HH:MM:SS.mmm` or `MM:SS.mmm`, as
    /// the time since midnight, milliseconds included. The fraction is read as
    /// digits rather than through a float, so "49.059" is exactly 49,059 ms.
    fn parse_challenge_duration(challenge_start: &str) -> Option<Duration> {
        let parts: Vec<&str> = challenge_start.split(':').collect();

        let (hours, minutes, seconds) = match parts.as_slice() {
            [h, m, s] => (h.parse::<i64>().ok()?, m.parse::<i64>().ok()?, *s),
            [m, s] => (0, m.parse::<i64>().ok()?, *s),
            _ => return None,
        };

        let (whole, fraction) = seconds.split_once('.').unwrap_or((seconds, ""));
        let whole: i64 = whole.parse().ok()?;
        if !fraction.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        // Milliseconds are the first three digits, padded: ".5" is 500.
        let millis: i64 = format!("{:0<3}", &fraction[..fraction.len().min(3)])
            .parse()
            .ok()?;

        Some(
            Duration::hours(hours)
                + Duration::minutes(minutes)
                + Duration::seconds(whole)
                + Duration::milliseconds(millis),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Stat, StatType};
    use chrono::{
        DateTime, Duration, FixedOffset, Local, MappedLocalTime, NaiveDate, NaiveDateTime,
        NaiveTime, TimeZone, Utc,
    };

    /// Berlin's clocks in 2026, written out so the summer-time tests do not
    /// depend on the machine's time zone: UTC+1, and UTC+2 from 29 March to
    /// 25 October, each change at 01:00 UTC.
    #[derive(Clone, Copy, Debug)]
    struct Berlin2026;

    impl Berlin2026 {
        fn summer(utc: &NaiveDateTime) -> bool {
            let start = NaiveDate::from_ymd_opt(2026, 3, 29)
                .expect("a date")
                .and_hms_opt(1, 0, 0);
            let end = NaiveDate::from_ymd_opt(2026, 10, 25)
                .expect("a date")
                .and_hms_opt(1, 0, 0);
            Some(*utc) >= start && Some(*utc) < end
        }

        fn hours(h: i32) -> FixedOffset {
            FixedOffset::east_opt(h * 3600).expect("an offset")
        }
    }

    impl TimeZone for Berlin2026 {
        type Offset = FixedOffset;

        fn from_offset(_: &FixedOffset) -> Self {
            Berlin2026
        }

        fn offset_from_local_date(&self, local: &NaiveDate) -> MappedLocalTime<FixedOffset> {
            self.offset_from_local_datetime(&local.and_time(NaiveTime::MIN))
        }

        fn offset_from_local_datetime(
            &self,
            local: &NaiveDateTime,
        ) -> MappedLocalTime<FixedOffset> {
            // Summer first, so a clock time shown twice lists its earlier instant first.
            let fits: Vec<FixedOffset> = [Self::hours(2), Self::hours(1)]
                .into_iter()
                .filter(|offset| {
                    let utc = *local - Duration::seconds(offset.local_minus_utc().into());
                    self.offset_from_utc_datetime(&utc) == *offset
                })
                .collect();

            match fits.as_slice() {
                [] => MappedLocalTime::None,
                [one] => MappedLocalTime::Single(*one),
                [first, second, ..] => MappedLocalTime::Ambiguous(*first, *second),
            }
        }

        fn offset_from_utc_date(&self, utc: &NaiveDate) -> FixedOffset {
            self.offset_from_utc_datetime(&utc.and_time(NaiveTime::MIN))
        }

        fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
            Self::hours(if Self::summer(utc) { 2 } else { 1 })
        }
    }

    fn utc(year: i32, month: u32, day: u32, h: u32, m: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, h, m, s)
            .single()
            .expect("a UTC time")
    }

    fn at(seconds: i64, millis: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_800_000_000 + seconds, 0).expect("valid moment")
            + Duration::milliseconds(millis)
    }

    /// A wall-clock time here, the way KovaaK's writes its times.
    fn local(year: i32, month: u32, day: u32, h: u32, m: u32, s: u32) -> DateTime<Utc> {
        Local
            .with_ymd_and_hms(year, month, day, h, m, s)
            .single()
            .expect("an unambiguous local time")
            .with_timezone(&Utc)
    }

    fn clock(h: i64, m: i64, s: i64) -> Duration {
        Duration::hours(h) + Duration::minutes(m) + Duration::seconds(s)
    }

    /// The tail of a stats file as KovaaK's 3.9.10 writes it.
    const STATS: &str = "Kill #,Timestamp,Bot,Weapon,TTK,Shots,Hits\n\
        1,21:24:11.069,fast strafes blink,LG,21.755001s,2177,1000\n\
        \n\
        Kills:,6\n\
        Fight Time:,113.575996\n\
        Score:,885.119141\n\
        Scenario:,Ground Plaza Sparky v3 OW\n\
        Hash:,7d1081a21910a705055afdb9d35dca79\n\
        Challenge Start:,21:23:49.059\n\
        Pause Count:,0\n\
        \n\
        Sens Scale:,cm/360\n";

    #[test]
    fn a_challenge_start_is_read_as_a_clock_time() {
        assert_eq!(
            Stat::parse_challenge_duration("21:23:49"),
            Some(clock(21, 23, 49))
        );
        assert_eq!(
            Stat::parse_challenge_duration("23:49"),
            Some(clock(0, 23, 49))
        );
    }

    /// KovaaK's writes milliseconds, and they are kept exactly.
    #[test]
    fn a_challenge_start_keeps_its_milliseconds() {
        assert_eq!(
            Stat::parse_challenge_duration("21:23:49.059"),
            Some(clock(21, 23, 49) + Duration::milliseconds(59))
        );
        assert_eq!(
            Stat::parse_challenge_duration("23:49.5"),
            Some(clock(0, 23, 49) + Duration::milliseconds(500))
        );
        // Past the millisecond is dropped, not rounded.
        assert_eq!(
            Stat::parse_challenge_duration("0:0:1.0009"),
            Some(clock(0, 0, 1))
        );
    }

    #[test]
    fn a_challenge_start_that_is_not_a_clock_time_is_none() {
        assert_eq!(Stat::parse_challenge_duration(""), None);
        assert_eq!(Stat::parse_challenge_duration("soon"), None);
        assert_eq!(Stat::parse_challenge_duration("1:2:3:4"), None);
        assert_eq!(Stat::parse_challenge_duration("a:b:c"), None);
        assert_eq!(Stat::parse_challenge_duration("21:23:4x"), None);
        assert_eq!(Stat::parse_challenge_duration("21:23:49.05x"), None);
    }

    #[test]
    fn a_run_starts_on_the_day_it_ended() {
        let end = local(2026, 10, 15, 21, 25, 43);

        assert_eq!(
            Stat::compute_start_time(end, clock(21, 23, 49)),
            local(2026, 10, 15, 21, 23, 49)
        );
    }

    /// Started before midnight, ended after: the clock time of the start is
    /// later than the end's, so the start was the day before.
    #[test]
    fn a_run_across_midnight_starts_the_day_before() {
        let end = local(2026, 10, 16, 0, 1, 10);

        assert_eq!(
            Stat::compute_start_time(end, clock(23, 59, 20)),
            local(2026, 10, 15, 23, 59, 20)
        );
    }

    /// A run that starts and ends in the same second is not mistaken for one
    /// that crossed midnight.
    #[test]
    fn a_start_in_the_same_second_as_the_end_is_that_day() {
        let end = local(2026, 10, 15, 21, 25, 43);

        assert_eq!(Stat::compute_start_time(end, clock(21, 25, 43)), end);
    }

    /// `1w2ts Perfected`, played on this install: named for 00:00:16 on 23
    /// March, written then, Challenge Start 23:59:16.403.
    #[test]
    fn a_real_run_across_midnight_starts_the_evening_before() {
        let end = local(2026, 3, 23, 0, 0, 16);

        assert_eq!(
            Stat::compute_start_time(end, clock(23, 59, 16) + Duration::milliseconds(403)),
            local(2026, 3, 22, 23, 59, 16) + Duration::milliseconds(403)
        );
    }

    /// A run that ends as it begins can show a start a moment after its
    /// file's time: "01:31:45.253" in a file written at 01:31:44. That is the
    /// same day, not a run that started the day before.
    #[test]
    fn a_start_a_moment_after_the_end_is_that_day() {
        let end = local(2026, 8, 2, 1, 31, 44) + Duration::milliseconds(900);
        let start = clock(1, 31, 45) + Duration::milliseconds(253);

        assert_eq!(
            Stat::compute_start_time(end, start),
            local(2026, 8, 2, 1, 31, 45) + Duration::milliseconds(253)
        );
    }

    #[test]
    fn the_start_keeps_its_milliseconds() {
        let end = local(2026, 10, 15, 21, 25, 43) + Duration::milliseconds(781);

        assert_eq!(
            Stat::compute_start_time(end, clock(21, 23, 49) + Duration::milliseconds(59)),
            local(2026, 10, 15, 21, 23, 49) + Duration::milliseconds(59)
        );
    }

    /// When the clocks go back, 02:00 to 03:00 happens twice. A run that ends
    /// in the second pass started in it too, not an hour earlier. The old
    /// version panicked here: setting the hour on an ambiguous time gives none.
    #[test]
    fn a_run_in_the_repeated_hour_is_not_an_hour_long() {
        // 02:40 CET, the second 02:40 of the night.
        let end = utc(2026, 10, 25, 1, 40, 0);

        assert_eq!(
            Stat::start_in(&Berlin2026, end, clock(2, 35, 0)),
            utc(2026, 10, 25, 1, 35, 0)
        );
    }

    /// Started in the first pass of the repeated hour, ended in the second.
    #[test]
    fn a_run_across_the_clocks_going_back_starts_in_the_first_pass() {
        // 02:10 CET, after the clocks went back from 03:00 CEST.
        let end = utc(2026, 10, 25, 1, 10, 0);

        assert_eq!(
            Stat::start_in(&Berlin2026, end, clock(2, 55, 0)),
            utc(2026, 10, 25, 0, 55, 0)
        );
    }

    /// Started before the clocks went forward, ended after.
    #[test]
    fn a_run_across_the_clocks_going_forward_keeps_its_length() {
        // 03:00:30 CEST, a minute of wall clock after 01:59:50 CET.
        let end = utc(2026, 3, 29, 1, 0, 30);

        assert_eq!(
            Stat::start_in(&Berlin2026, end, clock(1, 59, 50)),
            utc(2026, 3, 29, 0, 59, 50)
        );
    }

    /// 02:30 never shows on 29 March: the clocks jump from 02:00 to 03:00.
    /// KovaaK's cannot write it, but a wrong file must not panic, and is read
    /// with the offset the run ended in.
    #[test]
    fn a_clock_time_the_clocks_skip_does_not_panic() {
        // 03:10 CEST.
        let end = utc(2026, 3, 29, 1, 10, 0);

        assert_eq!(
            Stat::start_in(&Berlin2026, end, clock(2, 30, 0)),
            utc(2026, 3, 29, 0, 30, 0)
        );
    }

    #[test]
    fn a_stats_file_gives_the_run() {
        let dir = std::env::temp_dir().join("kovobs_stat_parse");
        _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("created folder");

        let path =
            dir.join("Ground Plaza Sparky v3 OW - Challenge - 2026.10.15-21.25.43 Stats.csv");
        std::fs::write(&path, STATS).expect("written stats");

        let stat = Stat::parse_kovaaks_stat(&path).expect("parsed");

        assert_eq!(stat.scenario, "Ground Plaza Sparky v3 OW");
        // The same f32 the file's text parses to; the literal has more digits than an f32 holds.
        assert_eq!(stat.score, "885.119141".parse::<f32>().expect("a number"));
        assert_eq!(
            stat.start_dt,
            local(2026, 10, 15, 21, 23, 49) + Duration::milliseconds(59)
        );
        assert!(matches!(stat.stat_type, StatType::KovaaKs));
        // Just written, so its timestamp is far from the name's and the name decides.
        assert_eq!(stat.end_dt, local(2026, 10, 15, 21, 25, 43));

        std::fs::remove_dir_all(&dir).expect("cleaned up");
    }

    #[test]
    fn a_missing_stats_file_is_an_error() {
        let path = std::env::temp_dir()
            .join("kovobs_stat_missing - Challenge - 2026.10.15-21.25.43 Stats.csv");

        assert!(Stat::parse_kovaaks_stat(&path).is_err());
    }

    /// Clip and screenshot names are built from this.
    #[test]
    fn a_run_reads_as_scenario_score_and_end_time() {
        let mut stat = Stat {
            scenario: "Ground Plaza".into(),
            score: 885.119,
            start_dt: local(2026, 10, 15, 21, 23, 49),
            end_dt: local(2026, 10, 15, 21, 25, 43),
            stat_type: StatType::KovaaKs,
        };
        assert_eq!(
            stat.to_string(),
            "Ground Plaza - 885.12 - 2026.10.15-21.25.43"
        );

        // A whole score drops its decimals.
        stat.score = 1314.0;
        assert_eq!(
            stat.to_string(),
            "Ground Plaza - 1314 - 2026.10.15-21.25.43"
        );
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

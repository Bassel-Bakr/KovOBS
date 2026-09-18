use crate::consts;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use std::time::Duration;
use std::{fs, path};

pub fn get_creation_or_modification_time(
    path: &path::Path,
) -> Result<DateTime<Utc>, std::io::Error> {
    let metadata = fs::metadata(path)?;
    let time = metadata.created().or(metadata.modified())?;
    Ok(DateTime::<Utc>::from(time))
}

/// When the file was last written.
///
/// Distinct from [`get_creation_or_modification_time`], which prefers the
/// creation time: that is the right answer for a file written once per run,
/// and the wrong one for a file that is rewritten every run, where creation
/// dates from whenever the scenario was first played.
pub fn get_modification_time(path: &path::Path) -> Result<DateTime<Utc>, std::io::Error> {
    Ok(DateTime::<Utc>::from(fs::metadata(path)?.modified()?))
}

pub fn parse_local_datetime(date_time_str: &str) -> Option<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(date_time_str, consts::STAT_DATE_TIME_FORMAT)
        .ok()
        .and_then(|naive| Local.from_local_datetime(&naive).earliest())
        .map(|d| d.to_utc())
}

/// Waits until the file is stable (fully written to disk)
pub async fn wait_for_file(path: &path::Path) -> Result<(), std::io::Error> {
    let mut prev_len = 0;
    let mut stable_checks = 0;

    while stable_checks < 3 {
        let len = tokio::fs::metadata(path).await?.len();

        if len == prev_len {
            stable_checks += 1;
        } else {
            stable_checks = 0;
            prev_len = len;
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    Ok(())
}

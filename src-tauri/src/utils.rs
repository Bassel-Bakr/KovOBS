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

#[cfg(test)]
mod tests {
    use super::{get_modification_time, wait_for_file};
    use chrono::Utc;
    use std::time::Duration;

    fn temp_file(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("kovobs_utils_{name}"))
    }

    /// Aimbeast rewrites one file per scenario, so the run's time is the latest
    /// write, not the first.
    #[tokio::test]
    async fn the_modification_time_follows_the_latest_write() {
        let path = temp_file("rewritten.json");

        std::fs::write(&path, "first").expect("written");
        let first = get_modification_time(&path).expect("read");

        tokio::time::sleep(Duration::from_millis(50)).await;

        std::fs::write(&path, "second").expect("rewritten");
        let second = get_modification_time(&path).expect("read");

        assert!(second > first, "{second} is not after {first}");
        assert!(
            (Utc::now() - second).num_seconds().abs() < 5,
            "{second} is not around now"
        );

        std::fs::remove_file(&path).expect("cleaned up");
    }

    #[tokio::test]
    async fn a_file_that_stops_growing_is_stable() {
        let path = temp_file("stable.json");

        std::fs::write(&path, "done").expect("written");

        wait_for_file(&path).await.expect("stable");

        std::fs::remove_file(&path).expect("cleaned up");
    }

    #[tokio::test]
    async fn a_file_that_is_not_there_is_an_error() {
        assert!(get_modification_time(&temp_file("missing.json")).is_err());
        assert!(wait_for_file(&temp_file("missing.json")).await.is_err());
    }
}

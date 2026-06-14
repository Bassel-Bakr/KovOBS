use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use std::{fs, path};

use crate::consts;

pub struct Utils {}

impl Utils {
    pub fn get_creation_or_modification_time(
        path: &path::Path,
    ) -> Result<DateTime<Utc>, std::io::Error> {
        let metadata = fs::metadata(path)?;
        let time = metadata.created().or(metadata.modified())?;
        Ok(DateTime::<Utc>::from(time))
    }

    pub fn parse_local_datetime(date_time_str: &str) -> Option<DateTime<Utc>> {
        NaiveDateTime::parse_from_str(date_time_str, consts::STAT_DATE_TIME_FORMAT)
            .ok()
            .and_then(|naive| Local.from_local_datetime(&naive).earliest())
            .map(|d| d.to_utc())
    }
}

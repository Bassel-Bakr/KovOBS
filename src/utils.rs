use chrono::{DateTime, Utc};
use std::{fs, path};

pub struct Utils {}

impl Utils {
    pub fn get_creation_or_modification_time(
        path: &path::Path,
    ) -> Result<DateTime<Utc>, std::io::Error> {
        let metadata = fs::metadata(path)?;
        let time = metadata.created().or(metadata.modified())?;
        Ok(DateTime::<Utc>::from(time))
    }
}

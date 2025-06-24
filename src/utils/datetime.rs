use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub(crate) enum DateTimeError {
    DateTimeInvalid(String),
}

impl std::fmt::Display for DateTimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DateTimeError::DateTimeInvalid(e) => write!(f, "DateTimeInvalid({})", e),
        }
    }
}

// /// Converts +02:00 DateTime to DateTime<FixedOffset> (takes DateTime String in format: %d.%m.%Y %H:%M:%S)
//pub(crate) fn datetime_string_to_datetime(
//    datetime: &str,
//) -> Result<DateTime<FixedOffset>, DateTimeError> {
//    let date_time =
//        DateTime::parse_from_str(&format!("{} +02:00", datetime), "%d.%m.%Y %H:%M:%S %z").map_err(
//            |e| {
//                DateTimeError::DateTimeInvalid(format!(
//                    "converting '{} +02:00' failed with error '{}'",
//                    datetime, e
//                ))
//            },
//        )?;
//    Ok(date_time)
//}

/// Converts +02:00 DateTime to DateTime<FixedOffset> (takes DateTime String in format: %Y-%m-%d %H:%M:%S)
pub(crate) fn datetime_string_stupid_to_datetime(
    datetime: &str,
) -> Result<DateTime<FixedOffset>, DateTimeError> {
    let date_time =
        DateTime::parse_from_str(&format!("{} +02:00", datetime), "%Y-%m-%d %H:%M:%S %z").map_err(
            |e| {
                DateTimeError::DateTimeInvalid(format!(
                    "converting '{} +02:00' failed with error '{}'",
                    datetime, e
                ))
            },
        )?;
    Ok(date_time)
}

/// Converts +02:00 Date & Time to DateTime<FixedOffset> (takes DateTime String in format: %d.%m.%Y %H:%M:%S)
pub(crate) fn date_time_string_to_datetime(
    date: &str,
    time: &str,
) -> Result<DateTime<FixedOffset>, DateTimeError> {
    let date_time =
        DateTime::parse_from_str(&format!("{} {} +02:00", date, time), "%d.%m.%Y %H:%M:%S %z")
            .map_err(|e| {
                DateTimeError::DateTimeInvalid(format!(
                    "converting '{} {}' +02:00 failed with error '{}'",
                    date, time, e
                ))
            })
            .unwrap();
    Ok(date_time)
}

/// Converts Date to NaiveDate (%d.%m.%Y)
pub(crate) fn date_string_to_naivedate(date: &str) -> Result<NaiveDate, DateTimeError> {
    let date = NaiveDate::parse_from_str(date, "%d.%m.%Y").map_err(|e| {
        DateTimeError::DateTimeInvalid(format!("converting '{}' failed with error '{}'", date, e))
    })?;

    Ok(date)
}

/// Merges a [NaiveDate] with a [NaiveTime] to a [DateTime] (CEST)
pub(crate) fn merge_naive_date_time_to_datetime(
    date: &NaiveDate,
    time: &NaiveTime,
) -> Result<DateTime<FixedOffset>, DateTimeError> {
    let date_time =
        DateTime::parse_from_str(&format!("{} {} +02:00", date, time), "%Y-%m-%d %H:%M:%S %z")
            .map_err(|e| {
                DateTimeError::DateTimeInvalid(format!(
                    "merging '{} {}' +02:00 failed with error '{}'",
                    date, time, e
                ))
            })?;
    Ok(date_time)
}

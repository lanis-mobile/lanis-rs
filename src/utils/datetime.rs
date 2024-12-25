use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub(crate) enum DateTimeError {
    DateInvalid(String),
    TimeInvalid(String),
}

/// Converts CEST DateTime to NaiveDateTime (takes DateTime String in format: %d.%m.%Y %H:%M:%S)
pub(crate) async fn date_time_string_to_datetime(date: &String, time: &String) -> Result<DateTime<FixedOffset>, DateTimeError> {
    let date_time = DateTime::parse_from_str(&format!("{} {} +02:00", date, time), "%d.%m.%Y %H:%M:%S %z").map_err(|e| DateTimeError::TimeInvalid(e.to_string()))?;
    Ok(date_time)
}

/// Merges a [NaiveDate] with a [NaiveTime] to a [DateTime] (CEST)
pub(crate) fn merge_naive_date_time_to_datetime(date: &NaiveDate, time: &NaiveTime) -> Result<DateTime<FixedOffset>, DateTimeError> {
    let date_time = DateTime::parse_from_str(&format!("{} {} +02:00", date, time), "%Y-%m-%d %H:%M:%S %z").map_err(|e| DateTimeError::DateInvalid(e.to_string()))?;
    Ok(date_time)
}
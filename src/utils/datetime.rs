use chrono::{DateTime, FixedOffset};

/// Converts CEST DateTime to NaiveDateTime (takes DateTime String in format: %d.%m.%Y %H:%M:%S)
pub(crate) async fn date_time_string_to_date_time(date: &String, time: &String) -> Result<DateTime<FixedOffset>, String> {
    let date_time = DateTime::parse_from_str(&format!("{} {} +02:00", date, time), "%d.%m.%Y %H:%M:%S %z").map_err(|e| format!("Invalid date: {}", e))?;
    Ok(date_time)
}
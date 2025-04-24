use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub(crate) enum ConversionError {
    /// Happens if the function doesn't know the provided unit (the [String] contains the unknown unit)
    UnknownUnit(String),
    /// Happens if a wrong format for [String] conversion is used
    InvalidFormat(String),
    /// Happens if something goes wrong when parsing a type into another type
    Parsing(String),
}

impl Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConversionError::UnknownUnit(e) => write!(f, "Unknown unit! ({e})"),
            ConversionError::InvalidFormat(e) => write!(f, "Invalid format! ({e})"),
            ConversionError::Parsing(e) => write!(f, "Parsing failed! ({e})"),
        }
    }
}

pub(crate) async fn string_to_byte_size(string: String) -> Result<u64, ConversionError> {
    let parts = string.trim().split_whitespace().collect::<Vec<&str>>();
    if parts.len() != 2 {
        return Err(ConversionError::InvalidFormat(String::from(
            "expected a number and the unit seperated by spaces",
        )));
    }

    let number = parts
        .get(0)
        .unwrap()
        .replace(",", ".")
        .parse::<f64>()
        .map_err(|e| ConversionError::Parsing(format!("failed to parse size to f64 '{}'", e)))?;

    let bytes = match *parts.get(1).unwrap() {
        "B" => number as u64,
        "KB" => (number * 1_024.0) as u64,
        "MB" => (number * 1_024.0 * 1_024.0) as u64,
        "GB" => (number * 1_024.0 * 1_024.0 * 1_024.0) as u64,
        "TB" => (number * 1_024.0 * 1_024.0 * 1_024.0 * 1_024.0) as u64,
        "PB" => (number * 1_024.0 * 1_024.0 * 1_024.0 * 1_024.0 * 1_024.0) as u64,
        unknown_unit => return Err(ConversionError::UnknownUnit(unknown_unit.to_owned())),
    };

    Ok(bytes)
}


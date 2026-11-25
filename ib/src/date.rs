use crate::error::Error;
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use std::str::FromStr;

type Result<T> = std::result::Result<T, Error>;

pub fn convert_timestamp_to_date_string(timestamp: i64) -> Result<String> {
    let datetime: DateTime<Utc> = Utc
        .timestamp_opt(timestamp, 0)
        .single()
        .ok_or(Error::FailedToConvertDate)?;
    Ok(datetime.format("%Y-%m-%d").to_string())
}

/// Konvertiere Datum in Sekunden seit UNIX Epoch
pub fn convert_date(date: &str) -> Result<i64> {
    let date = NaiveDate::from_str(&date[0..10]).map_err(|_| Error::FailedToParseDate)?;
    let timestamp = date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
    Ok(timestamp)
}

pub fn get_remaining_months(timestamp: i64) -> Result<u8> {
    let datetime: DateTime<Utc> = Utc
        .timestamp_opt(timestamp, 0)
        .single()
        .ok_or(Error::FailedToConvertDate)?;
    Ok((13 - datetime.month()) as u8)
}

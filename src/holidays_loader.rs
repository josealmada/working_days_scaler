use chrono::{Date, FixedOffset, NaiveDate, ParseError, TimeZone};
use csv::StringRecord;
use thiserror::Error;
use tracing::error;

use crate::holidays_loader::HolidaysLoaderError::{ErrorOpeningFile, InvalidDateFormat};

#[derive(Error, Debug)]
pub enum HolidaysLoaderError {
    #[error("Error opening file {0}.")]
    ErrorOpeningFile(String, #[source] csv::Error),
    #[error("Invalid date format at line {0}.")]
    InvalidDateFormat(u64, #[source] ParseError),
}

pub fn load(
    time_offset: FixedOffset,
    holidays_file: &str,
) -> Result<Vec<Date<FixedOffset>>, HolidaysLoaderError> {
    let mut holidays = Vec::new();

    let mut reader = csv::Reader::from_path(holidays_file)
        .map_err(|err| ErrorOpeningFile(holidays_file.to_string(), err))?;

    for result in reader.records() {
        match result {
            Ok(record) => {
                if let Some(date_string) = record.get(0) {
                    let date = NaiveDate::parse_from_str(date_string, "%Y-%m-%d")
                        .map_err(|err| InvalidDateFormat(line_number(record), err))?;
                    holidays.push(time_offset.from_utc_date(&date));
                }
            }
            Err(err) => error!(
                "Error {} loading holidays from {}",
                err.to_string(),
                holidays_file
            ),
        }
    }

    Ok(holidays)
}

fn line_number(record: StringRecord) -> u64 {
    match record.position() {
        None => 0,
        Some(pos) => pos.line() + 1,
    }
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, TimeZone};

    use crate::holidays_loader::load;

    #[tokio::test]
    async fn should_return_error_if_holidays_file_not_found() {
        let offset = FixedOffset::west(3 * 3600);

        let result = load(offset, "unknown_file.csv");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Error opening file unknown_file.csv."
        );
    }

    #[tokio::test]
    async fn should_return_error_if_any_invalid_date() {
        let offset = FixedOffset::west(3 * 3600);

        let result = load(offset, "tests_resources/invalid_date_holidays.csv");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Invalid date format at line 5."
        );
    }

    #[tokio::test]
    async fn should_load_holidays_ignoring_offset() {
        let offset = FixedOffset::west(3 * 3600);

        let result = load(offset, "tests_resources/small_holidays.csv");
        assert!(result.is_ok());

        let holidays = result.unwrap();
        assert_eq!(holidays.len(), 12);

        println!("{}", holidays.get(0).unwrap());

        assert_eq!(*holidays.get(0).unwrap(), offset.ymd(2020, 1, 1));
        assert_eq!(*holidays.get(1).unwrap(), offset.ymd(2020, 2, 24));
        assert_eq!(*holidays.get(2).unwrap(), offset.ymd(2020, 2, 25));
        assert_eq!(*holidays.get(3).unwrap(), offset.ymd(2020, 4, 10));
        assert_eq!(*holidays.get(4).unwrap(), offset.ymd(2020, 4, 21));
        assert_eq!(*holidays.get(5).unwrap(), offset.ymd(2020, 5, 1));
        assert_eq!(*holidays.get(6).unwrap(), offset.ymd(2020, 6, 11));
        assert_eq!(*holidays.get(7).unwrap(), offset.ymd(2020, 9, 7));
        assert_eq!(*holidays.get(8).unwrap(), offset.ymd(2020, 10, 12));
        assert_eq!(*holidays.get(9).unwrap(), offset.ymd(2020, 11, 2));
        assert_eq!(*holidays.get(10).unwrap(), offset.ymd(2020, 11, 15));
        assert_eq!(*holidays.get(11).unwrap(), offset.ymd(2020, 12, 25));
    }
}

use chrono::{Date, Datelike, Duration, FixedOffset, Weekday};
use thiserror::Error;

use WorkingDaysError::DateOutOfRange;

use crate::working_days::WorkingDaysError::EmptyHolidayList;

#[derive(Debug)]
pub struct WorkingDays {
    pub time_offset: FixedOffset,
    pub start_date: Date<FixedOffset>,
    pub end_date: Date<FixedOffset>,
    data_offset: usize,
    data: Vec<u8>,
}

#[derive(Error, Debug, PartialEq)]
pub enum WorkingDaysError {
    #[error("The holiday list is empty. Its also used to infer witch years to process.")]
    EmptyHolidayList,
    #[error(
        "The requested date was not calculated. Table processed for dates between {0} and {1}."
    )]
    DateOutOfRange(Date<FixedOffset>, Date<FixedOffset>),
}

impl WorkingDays {
    pub fn build(
        time_offset: FixedOffset,
        holidays: Vec<Date<FixedOffset>>,
    ) -> Result<WorkingDays, WorkingDaysError> {
        if holidays.is_empty() {
            Err(EmptyHolidayList)
        } else {
            let start_date = at_start_of_year(holidays.first().unwrap());
            let end_date = at_end_of_year(holidays.last().unwrap());
            Ok(Self::build_with_range(
                time_offset,
                start_date,
                end_date,
                holidays,
            ))
        }
    }

    pub fn build_with_range(
        time_offset: FixedOffset,
        start_date: Date<FixedOffset>,
        end_date: Date<FixedOffset>,
        mut holidays: Vec<Date<FixedOffset>>,
    ) -> Self {
        holidays.sort();

        let data_offset = start_date.num_days_from_ce() as usize;
        let data = process_working_days(&start_date, &end_date, holidays);

        WorkingDays {
            time_offset,
            start_date,
            end_date,
            data_offset,
            data,
        }
    }

    pub fn working_days_mtd(&self, date: Date<FixedOffset>) -> Result<u8, WorkingDaysError> {
        let date_days = date.num_days_from_ce() as usize;
        if date_days >= self.data_offset && date_days < self.data_offset + self.data.len() {
            let index = date.num_days_from_ce() as usize - self.data_offset;
            Ok(*self.data.get(index).unwrap())
        } else {
            Err(DateOutOfRange(self.start_date, self.end_date))
        }
    }
}

fn at_start_of_year(date: &Date<FixedOffset>) -> Date<FixedOffset> {
    date.with_month(1).unwrap().with_day(1).unwrap()
}

fn at_end_of_year(date: &Date<FixedOffset>) -> Date<FixedOffset> {
    date.with_month(12).unwrap().with_day(31).unwrap()
}

fn process_working_days(
    start_date: &Date<FixedOffset>,
    end_date: &Date<FixedOffset>,
    holidays: Vec<Date<FixedOffset>>,
) -> Vec<u8> {
    let data_size = end_date.num_days_from_ce() - start_date.num_days_from_ce();
    let mut data = Vec::with_capacity(data_size as usize);

    let mut current_date = *start_date;
    let mut current_month = start_date.month();
    let mut wd_count = 0;
    let mut holiday_iter = holidays.into_iter().filter(|date| date >= start_date);
    let mut next_holiday = holiday_iter.next();

    while current_date <= *end_date {
        if !is_weekend(&current_date) && Some(current_date) != next_holiday {
            wd_count += 1;
        }

        data.push(wd_count);

        if Some(current_date) == next_holiday {
            next_holiday = holiday_iter.next()
        }

        current_date += Duration::days(1);
        if current_date.month() != current_month {
            current_month = current_date.month();
            wd_count = 0
        }
    }

    data
}

fn is_weekend(date: &Date<FixedOffset>) -> bool {
    matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn should_require_a_holiday_list_not_empty() {
        let offset = FixedOffset::west(3 * 3600);
        let result = WorkingDays::build(offset, Vec::new());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), EmptyHolidayList)
    }

    #[test]
    fn should_calculate_start_date_and_end_date_correctly() {
        let mut holidays = Vec::new();
        let offset = FixedOffset::west(3 * 3600);

        holidays.push(offset.ymd(2020, 6, 5));
        holidays.push(offset.ymd(2021, 6, 5));

        let working_days = WorkingDays::build(offset, holidays).unwrap();

        assert_eq!(working_days.start_date, offset.ymd(2020, 1, 1));
        assert_eq!(working_days.end_date, offset.ymd(2021, 12, 31));
    }

    #[test]
    fn should_return_error_if_date_out_of_range() {
        let mut holidays = Vec::new();
        let offset = FixedOffset::west(3 * 3600);

        holidays.push(offset.ymd(2020, 6, 5));
        holidays.push(offset.ymd(2021, 6, 5));

        let working_days = WorkingDays::build(offset, holidays).unwrap();

        let before = working_days.working_days_mtd(offset.ymd(2019, 12, 31));
        assert!(before.is_err());

        let after = working_days.working_days_mtd(offset.ymd(2022, 1, 1));
        assert!(after.is_err());
    }

    #[test]
    fn should_working_days_calculation() {
        let mut holidays = Vec::new();
        let offset = FixedOffset::west(3 * 3600);

        holidays.push(offset.ymd(2022, 1, 1));
        holidays.push(offset.ymd(2022, 2, 28));
        holidays.push(offset.ymd(2022, 3, 1));
        holidays.push(offset.ymd(2022, 4, 15));
        holidays.push(offset.ymd(2022, 4, 21));
        holidays.push(offset.ymd(2022, 5, 1));
        holidays.push(offset.ymd(2022, 6, 16));
        holidays.push(offset.ymd(2022, 9, 7));
        holidays.push(offset.ymd(2022, 10, 12));
        holidays.push(offset.ymd(2022, 11, 2));
        holidays.push(offset.ymd(2022, 11, 15));
        holidays.push(offset.ymd(2022, 12, 25));

        let working_days = WorkingDays::build(offset, holidays).unwrap();

        let june: Vec<u8> = vec![
            1, 2, 3, 3, 3, 4, 5, 6, 7, 8, 8, 8, 9, 10, 11, 11, 12, 12, 12, 13, 14, 15, 16, 17, 17,
            17, 18, 19, 20, 21,
        ];
        let mut current_date = offset.ymd(2022, 6, 1);
        for wds in june {
            assert_eq!(working_days.working_days_mtd(current_date).unwrap(), wds);
            current_date += Duration::days(1);
        }

        let november = vec![
            1, 1, 2, 3, 3, 3, 4, 5, 6, 7, 8, 8, 8, 9, 9, 10, 11, 12, 12, 12, 13, 14, 15, 16, 17,
            17, 17, 18, 19, 20,
        ];
        let mut current_date = offset.ymd(2022, 11, 1);
        for wds in november {
            assert_eq!(working_days.working_days_mtd(current_date).unwrap(), wds);
            current_date += Duration::days(1);
        }
    }
}

use colored::Colorize;
use std::collections::HashMap;
use std::fmt;

use chrono::{DateTime, Datelike, NaiveDate, Utc};

use crate::{fileindex::IndexFile, fileindex::MetaValue, summarystats::FileIndexSummarizer};

/// Captures summary for a set of dates that is to be displayed
/// in a grid
pub struct DateSummary {
    dates: HashMap<i32, usize>,
    count: usize,
    months_per_row: Option<usize>,
    exif_dates: u64,
    file_dates: u64,
}

impl DateSummary {
    pub fn new() -> DateSummary {
        let dates: HashMap<i32, usize> = HashMap::new();
        DateSummary {
            dates,
            count: 0,
            months_per_row: None,
            exif_dates: 0,
            file_dates: 0,
        }
    }

    pub fn new_wrapping(months_per_row: usize) -> DateSummary {
        let dates: HashMap<i32, usize> = HashMap::new();
        DateSummary {
            dates,
            count: 0,
            months_per_row: Some(months_per_row),
            exif_dates: 0,
            file_dates: 0,
        }
    }

    /// Add a new date to the summary. Date will be
    /// binned and then displayed as part of the summary
    pub fn add_date(&mut self, date: &DateTime<Utc>) {
        self.count += 1;
        let year = date.year();
        let month = date.month();
        let day = date.day();
        let key = year * 10000 + (month * 100) as i32 + day as i32;
        *self.dates.entry(key).or_insert(0) += 1;
    }

    pub fn add_fileindex(&mut self, f: &IndexFile) {
        let mut date: DateTime<Utc> = f.created_at.clone();
        let mut has_exif_date = false;
        for ref v in &f.meta {
            if v.name == "DateTaken" {
                if let MetaValue::Date(ref value) = v.value {
                    date = value.clone();
                    self.exif_dates += 1;
                    has_exif_date = true;
                }
            }
        }
        if !has_exif_date {
            self.file_dates += 1;
        }
        self.add_date(&date);
    }
}

impl FileIndexSummarizer for DateSummary {
    fn add(&mut self, f: &crate::fileindex::IndexFile) {
        self.add_fileindex(&f);
    }
}

impl fmt::Display for DateSummary {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let summary = format!(
            "{}:{}",
            "#total".bold(),
            format!("{}", self.count).color("red")
        );
        write!(f, "{}", &summary)?;

        let (min_date_key, max_date_key) = {
            let keys: Vec<&i32> = self.dates.keys().collect();
            let min_key = keys.iter().min().unwrap();
            let max_key = keys.iter().max().unwrap();
            (**min_key, **max_key)
        };

        let min_year = min_date_key / 10000;
        let min_month = ((min_date_key % 10000) / 100) as u32;
        let max_year = max_date_key / 10000;
        let max_month = ((max_date_key % 10000) / 100) as u32;

        let mut date = NaiveDate::from_ymd_opt(min_year, min_month, 1).unwrap();
        let mut prev_month = date.month();
        loop {
            let mut grid: HashMap<(u32, u32), usize> = HashMap::new();
            let mut grid_width: u32 = 0;
            let mut month_breaks: Vec<u32> = Vec::new();
            let mut months_this_row = 0;
            let start_month = date.month0();
            let start_year = date.year();

            loop {
                let year = date.year();
                let weekday = date.weekday().num_days_from_sunday();
                let month = date.month();
                let day = date.day();
                let key = year * 10000 + (month * 100) as i32 + day as i32;
                let count_for_day = self.dates.get(&key);
                if let Some(count) = count_for_day {
                    grid.insert((grid_width, weekday), *count);
                } else {
                    grid.insert((grid_width, weekday), 0);
                }
                if weekday == 6 {
                    grid_width += 1;
                }
                date = date.succ_opt().unwrap();
                if month != prev_month {
                    month_breaks.push(grid_width);
                    prev_month = month;
                    months_this_row += 1;
                    if let Some(months_wrap) = self.months_per_row {
                        if months_this_row > months_wrap {
                            break;
                        }
                    }
                }
                if date.year() > max_year || (date.year() == max_year && date.month() > max_month) {
                    break;
                }
            }
            grid_width += 1;

            write!(f, "\n{} ", format!("{}", start_year).bold())?;
            const MONTHS: &[&str] = &[
                "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
            ];
            const DAYS: &[&str] = &["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

            let mut pos: usize = 5;
            let mut m: usize = start_month as usize;
            m += 1;
            if m >= 12 {
                m = 0;
            }
            write!(f, "\u{250C}{} ", MONTHS[start_month as usize])?;
            let mut heading_year = start_year;
            for target_pos in month_breaks.iter() {
                if pos <= *target_pos as usize {
                    write!(
                        f,
                        "{}\u{250C}{} ",
                        String::from(" ").repeat(*target_pos as usize - pos),
                        MONTHS[m]
                    )?;
                    pos = *target_pos as usize + 5;
                }
                m += 1;
                if m >= 12 {
                    m = 0;
                    heading_year += 1;
                    let year_str = format!("\u{250C}{} ", heading_year);
                    pos += year_str.len();
                    write!(f, "{}", year_str.bold())?;
                }
            }
            for d in 0..7 {
                write!(f, "\n {} ", DAYS[d as usize].italic().bright_black())?;
                for i in 0..(grid_width + 1) {
                    let v = grid.get(&(i, d));

                    if let Some(v) = v {
                        let output;
                        if *v > 10 {
                            output = String::from('\u{2589}').bright_green();
                        } else if *v > 5 {
                            output = String::from('\u{2593}').green();
                        } else if *v > 0 {
                            output = String::from('\u{2592}').green();
                        } else {
                            output = String::from('\u{2591}').bright_black();
                        }
                        write!(f, "{}", output)?
                    } else {
                        write!(f, " ")?
                    }
                }
            }
            write!(f, "\n")?;
            if date.year() > max_year || (date.year() == max_year && date.month() > max_month) {
                break;
            }
        }
        if self.file_dates > 0 {
            write!(
                f,
                "\nSome dates did not come from EXIF - exif:{} file:{}",
                self.exif_dates, self.file_dates
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;

    #[test]
    fn test_date_summary() {
        let mut summary = DateSummary::new();
        let date1 = DateTime::<Utc>::from_naive_utc_and_offset(
            NaiveDateTime::parse_from_str("2022-09-05 23:56:04", "%Y-%m-%d %H:%M:%S").unwrap(),
            Utc,
        );
        let date2 = DateTime::<Utc>::from_naive_utc_and_offset(
            NaiveDateTime::parse_from_str("2022-01-01 23:56:04", "%Y-%m-%d %H:%M:%S").unwrap(),
            Utc,
        );
        let date3 = DateTime::<Utc>::from_naive_utc_and_offset(
            NaiveDateTime::parse_from_str("2022-09-05 00:56:04", "%Y-%m-%d %H:%M:%S").unwrap(),
            Utc,
        );
        let date4 = DateTime::<Utc>::from_naive_utc_and_offset(
            NaiveDateTime::parse_from_str("2022-03-03 03:56:04", "%Y-%m-%d %H:%M:%S").unwrap(),
            Utc,
        );
        let date5 = DateTime::<Utc>::from_naive_utc_and_offset(
            NaiveDateTime::parse_from_str("2022-12-31 12:56:04", "%Y-%m-%d %H:%M:%S").unwrap(),
            Utc,
        );
        summary.add_date(&date1);
        for _ in 1..10 {
            summary.add_date(&date2);
        }
        summary.add_date(&date2);
        summary.add_date(&date3);
        summary.add_date(&date4);
        summary.add_date(&date5);

        let expected = "\u{1b}[1m#total\u{1b}[0m:\u{1b}[31m14\u{1b}[0m\n\u{1b}[1m2022\u{1b}[0m ┌JAN ┌FEB    ┌APR ┌MAY    ┌JUL ┌AUG     ┌OCT    ┌DEC \u{1b}[1m┌2023 \u{1b}[0m\n \u{1b}[3;90mSun\u{1b}[0m  \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m  \n \u{1b}[3;90mMon\u{1b}[0m  \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[32m▒\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m  \n \u{1b}[3;90mTue\u{1b}[0m  \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m  \n \u{1b}[3;90mWed\u{1b}[0m  \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m  \n \u{1b}[3;90mThu\u{1b}[0m  \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[32m▒\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m  \n \u{1b}[3;90mFri\u{1b}[0m  \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m  \n \u{1b}[3;90mSat\u{1b}[0m \u{1b}[32m▓\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[32m▒\u{1b}[0m  \n";

        let observed = format!("{}", summary);
        assert_eq!(
            expected, observed,
            "Expected: {} / Observed: {}",
            expected, observed
        );
    }

    #[test]
    fn test_date_summary_wrapping() {
        let mut summary = DateSummary::new_wrapping(4);
        let date1 = DateTime::<Utc>::from_naive_utc_and_offset(
            NaiveDateTime::parse_from_str("2022-09-05 23:56:04", "%Y-%m-%d %H:%M:%S").unwrap(),
            Utc,
        );
        let date2 = DateTime::<Utc>::from_naive_utc_and_offset(
            NaiveDateTime::parse_from_str("2022-01-01 23:56:04", "%Y-%m-%d %H:%M:%S").unwrap(),
            Utc,
        );
        let date3 = DateTime::<Utc>::from_naive_utc_and_offset(
            NaiveDateTime::parse_from_str("2022-09-05 00:56:04", "%Y-%m-%d %H:%M:%S").unwrap(),
            Utc,
        );
        let date4 = DateTime::<Utc>::from_naive_utc_and_offset(
            NaiveDateTime::parse_from_str("2022-03-03 03:56:04", "%Y-%m-%d %H:%M:%S").unwrap(),
            Utc,
        );
        let date5 = DateTime::<Utc>::from_naive_utc_and_offset(
            NaiveDateTime::parse_from_str("2022-12-31 12:56:04", "%Y-%m-%d %H:%M:%S").unwrap(),
            Utc,
        );
        summary.add_date(&date1);
        for _ in 1..10 {
            summary.add_date(&date2);
        }
        summary.add_date(&date2);
        summary.add_date(&date3);
        summary.add_date(&date4);
        summary.add_date(&date5);

        let expected = "\u{1b}[1m#total\u{1b}[0m:\u{1b}[31m14\u{1b}[0m\n\u{1b}[1m2022\u{1b}[0m ┌JAN ┌FEB    ┌APR ┌MAY \n \u{1b}[3;90mSun\u{1b}[0m  \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m \n \u{1b}[3;90mMon\u{1b}[0m  \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m \n \u{1b}[3;90mTue\u{1b}[0m  \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m \n \u{1b}[3;90mWed\u{1b}[0m  \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m \n \u{1b}[3;90mThu\u{1b}[0m  \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[32m▒\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m  \n \u{1b}[3;90mFri\u{1b}[0m  \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m  \n \u{1b}[3;90mSat\u{1b}[0m \u{1b}[32m▓\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m  \n\n\u{1b}[1m2022\u{1b}[0m ┌JUN     ┌AUG     ┌OCT \n \u{1b}[3;90mSun\u{1b}[0m  \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m \n \u{1b}[3;90mMon\u{1b}[0m  \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[32m▒\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m \n \u{1b}[3;90mTue\u{1b}[0m  \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m \n \u{1b}[3;90mWed\u{1b}[0m  \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m  \n \u{1b}[3;90mThu\u{1b}[0m \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m  \n \u{1b}[3;90mFri\u{1b}[0m \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m  \n \u{1b}[3;90mSat\u{1b}[0m \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m  \n\n\u{1b}[1m2022\u{1b}[0m ┌NOV \u{1b}[1m┌2023 \u{1b}[0m\n \u{1b}[3;90mSun\u{1b}[0m  \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m  \n \u{1b}[3;90mMon\u{1b}[0m  \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m  \n \u{1b}[3;90mTue\u{1b}[0m  \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m  \n \u{1b}[3;90mWed\u{1b}[0m \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m  \n \u{1b}[3;90mThu\u{1b}[0m \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m  \n \u{1b}[3;90mFri\u{1b}[0m \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m  \n \u{1b}[3;90mSat\u{1b}[0m \u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[90m░\u{1b}[0m\u{1b}[32m▒\u{1b}[0m  \n";

        let observed = format!("{}", summary);
        assert_eq!(
            expected, observed,
            "Expected: {} / Observed: {}",
            expected, observed
        );
    }
}

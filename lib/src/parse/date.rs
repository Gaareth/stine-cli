use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};

use anyhow::Context;
use chrono::{Date, DateTime, NaiveDate,
             NaiveDateTime, NaiveTime, ParseError, ParseResult, TimeZone, Utc, };
use chrono_tz::Europe::Berlin;
use lazy_static::lazy_static;

use crate::{Period, PeriodParseError};

lazy_static! {
    static ref WEEKDAY_MAP: HashMap<String, String> = HashMap::from([
        (String::from("Mo"), String::from("Mon")),
        (String::from("Di"), String::from("Tue")),
        (String::from("Mi"), String::from("Wed")),
        (String::from("Do"), String::from("Thu")),
        (String::from("Fr"), String::from("Fri")),
        (String::from("Sa"), String::from("Sat")),
        (String::from("So"), String::from("Sun")),
    ]);
}

#[derive(Debug)]
pub struct DateTimeParseError;


impl Display for DateTimeParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed parsing datetime")
    }
}

impl Error for DateTimeParseError {}

/// Replaces all german weekday abbreviations with the english alternative
// Reason: I could not find a localized function in the chrono library which can parse strings.
// only found the localized formatter fn.
pub fn ger_weekday_to_eng_str(date_str: &str) -> String {
    let mut eng_str: String = String::from(date_str);
    for (ger_weekday, eng_weekday) in WEEKDAY_MAP.iter() {
        eng_str = eng_str.replace(ger_weekday, eng_weekday);
    }

    eng_str
}

pub fn pre_process_date_string(date_str: &str) -> Result<String, DateTimeParseError> {
    let mut fixed_str = date_str.to_string();

    let weekday_map: HashMap<&str, &str> = HashMap::from([
        ("Mo", "Mon"),
        ("Di", "Tue"),
        ("Mi", "Wed"),
        ("Do", "Thu"),
        ("Fr", "Fri"),
        ("Sa", "Sat"),
        ("So", "Sun"),
        ("Mo", "Mon"),
        ("Tu", "Tue"),
        ("We", "Wed"),
        ("Th", "Thu"),
        ("Fr", "Fri"),
        ("Sa", "Sat"),
        ("Su", "Sun"),
    ]);

    let month_map: HashMap<&str, &str> = HashMap::from([
        ("Jan", "Jan"),
        ("Feb", "Feb"),
        ("Mär", "Mar"),
        ("Apr", "Apr"),
        ("Mai", "May"),
        ("Jun", "Jun"),
        ("Jul", "Jul"),
        ("Aug", "Aug"),
        ("Sep", "Sep"),
        ("Okt", "Oct"),
        ("Nov", "Nov"),
        ("Dez", "Dec"),
    ]);

    let weekday = date_str.split(',').next().unwrap();
    let fixed_weekday = weekday_map.get(weekday);
    if let Some(fixed_weekday) = fixed_weekday {
        fixed_str = fixed_str.replace(weekday, fixed_weekday);
    }

    let dot_split: Vec<&str> = date_str.split('.').collect();
    let month = match dot_split.len() {
        2 => { dot_split[1].chars().take(4).collect::<String>() }
        3 => { dot_split[1].to_string() }
        _ => { return Err(DateTimeParseError); }
    }.trim().to_string();

    //Mai 2022 14:15
    let fixed_month = month_map.get(month.as_str());
    if let Some(fixed_month) = fixed_month {
        if dot_split.len() == 2 {
            fixed_str = fixed_str.replace(month.as_str(), &format!("{}.", fixed_month));
        } else {
            fixed_str = fixed_str.replace(month.as_str(), fixed_month);
        }
    }

    Ok(fixed_str)
}


fn try_format(string: &str, format: &str) -> Result<DateTime<Utc>, ParseError> {
    Ok(stine_naive_to_utc(NaiveDateTime::parse_from_str(string, format)?))
}

// TODO: wait until chrono has a parse_from_localized_str function
// TODO: refactor using regex
// Mon, 20 June 2022, 9:00 am
fn parse_long_month_datetime(to_parse: &str) -> Result<DateTime<Utc>, PeriodParseError> {
    // format string needs to specific for chrono
    let s = to_parse.replace(" am", ":00 am");
    let s = s.replace(" pm", ":00 pm");
    // fix weird formatting of stine dates :(
    let s = s.replace(" ,", ",");

    let s = s.trim();

    let format_eng = "%a,%e %B %Y,%l:%M %P";
    let format_ger = "%a, %d.%m.%y, %H:%M Uhr";
    let format_ger2 = "%a, %d.%m.%y %H:%M Uhr";

    let dt = try_format(s, format_eng);
    if dt.is_ok() {
        return Ok(dt.unwrap());
    }

    let s = ger_weekday_to_eng_str(s);
    let dt = try_format(s.as_str(), format_ger);
    if dt.is_ok() {
        return Ok(dt.unwrap());
    }

    Ok(try_format(s.as_str(), format_ger2).with_context(|| format!("Failed parsing date: {to_parse}|{s}"))?)

}

pub fn stine_naive_to_utc(naive: NaiveDateTime) -> DateTime<Utc> {
    let tz_aware = Berlin.from_local_datetime(&naive).unwrap(); // TODO: can panic. look into fix this
    tz_aware.with_timezone(&Utc)
}

pub fn stine_naive_date_to_utc(naive: NaiveDate) -> Date<Utc> {
    let tz_aware = Berlin.from_local_date(&naive).unwrap(); // TODO: can panic. look into fix this
    tz_aware.with_timezone(&Utc)
}

pub fn parse_stine_dmy_date(s: &str) -> ParseResult<Date<Utc>> {
    Ok(stine_naive_date_to_utc(parse_dmy_date(s)?))
}

pub fn parse_dmy_date(s: &str) -> ParseResult<NaiveDate> {
    NaiveDate::parse_from_str(s, "%d.%m.%y")
}

pub fn parse_time(s: &str) -> ParseResult<NaiveTime> {
    NaiveTime::parse_from_str(s, "%H:%M")
}


/// Parses: Mon, 20 June 2022, 9:00 am to Mon, 20 June 2022, 9:00 am
pub fn parse_period(s: &str) -> Result<Period, PeriodParseError> {
    let mut split = if s.contains(" - ") {
        s.split(" - ")
    } else if s.contains(" to ") {
        s.split(" to ")
    } else {
        return Err(PeriodParseError::MissingSplitSymbol);
    };

    Ok(Period {
        start: parse_long_month_datetime(split.next().unwrap())?,
        end: parse_long_month_datetime(split.next().unwrap())?,
    })
}

/// Parses simple date strings in the following format: "%a,%e. %b. %Y" or "%a,%e. %b %Y"
/// Example: Fri, 8. Apr. 2022
pub fn parse_simple_stine_date(date_str: &str) -> Result<DateTime<Utc>, Box<dyn Error>> {
    // format: Fri, 8. Apr. 2022
    let format = "%a,%e. %b. %Y";

    // format: Fri, 8. May 2022
    let format2 = "%a,%e. %B %Y";

    let date_str = &pre_process_date_string(date_str)?;

    try_parse_datetime(date_str, format, format2)
}


/// Parses stine date string
/// # Arguments
/// * `date_str` - Date string in the format: "%a,%e. %b. %Y %H:%M"
///                or if this fails: "%a,%e. %B %Y %H:%M"
pub fn parse_stine_datetime(date_str: &str) -> Result<DateTime<Utc>, Box<dyn Error>> {
    // format: Fri, 8. Apr. 2022 10:15
    let format = "%a,%e. %b. %Y %H:%M";

    // format: Fri, 8. May 2022 10:15
    let format2 = "%a,%e. %B %Y %H:%M";

    let date_str = &pre_process_date_string(date_str)?;

    try_parse_datetime(date_str, format, format2)
}

pub fn try_parse_datetime(date_str: &str, format_1: &str, fallback_format: &str)
                          -> Result<DateTime<Utc>, Box<dyn Error>> {
    match NaiveDateTime::parse_from_str(date_str, format_1) {
        Ok(dt) => Ok(stine_naive_to_utc(dt)),
        Err(_) => {
            match NaiveDateTime::parse_from_str(date_str, fallback_format) {
                Ok(dt) => Ok(stine_naive_to_utc(dt)),
                Err(err) => Err(Box::new(err))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};

    use crate::parse::date::{parse_long_month_datetime, stine_naive_to_utc};
    use crate::parse::parse_appointment_datetime;

    #[test]
    fn test_long_month_date_parsing() {
        let ndt = NaiveDate::from_ymd(2022, 6, 20)
            .and_hms(9, 0, 0);
        let dt: DateTime<Utc> = stine_naive_to_utc(ndt);

        assert_eq!(dt, parse_long_month_datetime("Mon, 20 June 2022, 9 am").unwrap());
        assert_eq!(dt, parse_long_month_datetime("Mo, 20.06.22, 09:00 Uhr").unwrap());


        let dt = stine_naive_to_utc(NaiveDate::from_ymd(2022, 1, 1)
            .and_hms(13, 0, 0));
        assert_eq!(dt, parse_long_month_datetime("Sat, 1 Jan 2022, 1 pm").unwrap());
        assert_eq!(dt, parse_long_month_datetime("Sa, 01.01.22, 13:00 Uhr").unwrap());
    }

    #[test]
    fn test_date_parsing() {
        let d = NaiveDate::from_ymd(2022, 5, 4);
        let t = NaiveTime::from_hms(14, 15, 0);

        assert_eq!(NaiveDateTime::new(d, t), parse_appointment_datetime("Wed, 4. Mai 2022 14:15").unwrap());

        let d = NaiveDate::from_ymd(2022, 4, 6);
        let t = NaiveTime::from_hms(14, 15, 0);

        assert_eq!(NaiveDateTime::new(d, t), parse_appointment_datetime("Wed, 6. Apr. 2022 14:15").unwrap());
    }
}
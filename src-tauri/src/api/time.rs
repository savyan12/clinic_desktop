use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use chrono_tz::Africa::Tripoli;

pub fn today() -> NaiveDate {
    Utc::now().with_timezone(&Tripoli).date_naive()
}

pub fn iso(value: DateTime<Utc>) -> String {
    value
        .with_timezone(&Tripoli)
        .to_rfc3339_opts(SecondsFormat::Secs, false)
}

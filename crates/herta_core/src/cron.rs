//! Six numeric cron fields with AND semantics and explicit IANA calendar handling.
use crate::{HbError, HbResult, extension::Registration, jsvm::JsCronConfig};
use chrono::{DateTime, Datelike, LocalResult, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use serde::Deserialize;

#[derive(Clone, Debug)]
pub struct Schedule {
    fields: [u64; 6],
    timezone: Tz,
}
impl Schedule {
    pub fn parse(expression: &str, timezone: &str) -> HbResult<Self> {
        let parts: Vec<_> = expression.split_whitespace().collect();
        if parts.len() != 6 || expression.len() > 1024 {
            return Err(invalid());
        }
        let limits = [(0, 59), (0, 59), (0, 23), (1, 31), (1, 12), (0, 7)];
        let mut fields = [0; 6];
        for (index, (part, (min, max))) in parts.iter().zip(limits).enumerate() {
            fields[index] = field(part, min, max)?;
        }
        if fields[5] & (1 << 7) != 0 {
            fields[5] |= 1;
        }
        Ok(Self {
            fields,
            timezone: timezone.parse().map_err(|_| invalid())?,
        })
    }
    pub fn matches(&self, instant: DateTime<Utc>) -> bool {
        let local = instant.with_timezone(&self.timezone);
        let values = [
            local.second(),
            local.minute(),
            local.hour(),
            local.day(),
            local.month(),
            local.weekday().num_days_from_sunday(),
        ];
        if !self
            .fields
            .iter()
            .zip(values)
            .all(|(field, value)| field & (1 << value) != 0)
        {
            return false;
        }
        // Walking UTC skips nonexistent local times. An ambiguous wall time only
        // belongs to the earlier occurrence, including non-hour DST transitions.
        match self.timezone.from_local_datetime(&local.naive_local()) {
            LocalResult::Single(_) => true,
            LocalResult::Ambiguous(first, second) => {
                instant == first.min(second).with_timezone(&Utc)
            }
            LocalResult::None => false,
        }
    }
}
fn invalid() -> HbError {
    HbError::validation("invalid six-field cron schedule or options")
}
fn number(value: &str, min: u32, max: u32) -> HbResult<u32> {
    if value.is_empty() || !value.bytes().all(|ch| ch.is_ascii_digit()) {
        return Err(invalid());
    }
    let value = value.parse::<u32>().map_err(|_| invalid())?;
    if !(min..=max).contains(&value) {
        return Err(invalid());
    }
    Ok(value)
}
fn field(input: &str, min: u32, max: u32) -> HbResult<u64> {
    let mut bits = 0;
    for item in input.split(',') {
        let (range, step) = match item.split_once('/') {
            Some((range, step)) => (range, number(step, 1, max - min + 1)?),
            None => (item, 1),
        };
        let (start, end) = if range == "*" {
            (min, max)
        } else if let Some((start, end)) = range.split_once('-') {
            (number(start, min, max)?, number(end, min, max)?)
        } else {
            let start = number(range, min, max)?;
            (start, if item.contains('/') { max } else { start })
        };
        if start > end {
            return Err(invalid());
        }
        for value in (start..=end).step_by(step as usize) {
            bits |= 1 << value;
        }
    }
    Ok(bits)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Options {
    expression: String,
    timezone: Option<String>,
    max_runtime_ms: Option<u64>,
    retries: Option<usize>,
    #[serde(default)]
    idempotent: bool,
    auth_mode: String,
}
#[derive(Clone, Debug)]
pub struct CronTask {
    pub name: String,
    pub registration: usize,
    pub schedule: Schedule,
    pub max_runtime_ms: u64,
    pub retries: usize,
}
impl CronTask {
    pub fn parse(registration: &Registration, config: &JsCronConfig) -> HbResult<Self> {
        let name = &registration.name;
        if name.is_empty()
            || name.len() > 128
            || !name.as_bytes()[0].is_ascii_alphanumeric()
            || !name
                .bytes()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, b'.' | b'_' | b'-'))
        {
            return Err(invalid());
        }
        let options: Options =
            serde_json::from_value(registration.options.clone()).map_err(|_| invalid())?;
        let retries = options.retries.unwrap_or(config.retries);
        let max_runtime_ms = options.max_runtime_ms.unwrap_or(config.max_runtime_ms);
        if options.auth_mode != "system"
            || retries > config.max_retries
            || retries > 3
            || (retries != 0 && !options.idempotent)
            || max_runtime_ms == 0
            || max_runtime_ms > config.max_runtime_ms
        {
            return Err(invalid());
        }
        Ok(Self {
            name: name.clone(),
            registration: registration.id,
            schedule: Schedule::parse(
                &options.expression,
                options.timezone.as_deref().unwrap_or(&config.timezone),
            )?,
            retries,
            max_runtime_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn instant(value: &str) -> DateTime<Utc> {
        value.parse().unwrap()
    }
    #[test]
    fn six_fields_ranges_steps_and_day_weekday_are_all_and() {
        let schedule = Schedule::parse("0,30 */15 8-9 28 9 1", "UTC").unwrap();
        assert!(schedule.matches(instant("2026-09-28T08:15:30Z")));
        for time in [
            "2026-09-28T08:16:30Z",
            "2026-09-28T10:15:30Z",
            "2026-09-21T08:15:30Z",
            "2027-09-28T08:15:30Z",
        ] {
            assert!(!schedule.matches(instant(time)), "{time}");
        }
        assert!(
            Schedule::parse("0 0 0 * * 7", "UTC")
                .unwrap()
                .matches(instant("2026-09-27T00:00:00Z"))
        );
        for expression in [
            "* * * * *",
            "* * * * * * *",
            "60 * * * * *",
            "*/0 * * * * *",
            "0 0 0 ? * *",
            "0 0 0 * * MON",
            "0 0 0 * * 7-1",
            "0 0 0 0 * *",
            "0 0 0 * * 1,",
            "@daily",
        ] {
            assert!(Schedule::parse(expression, "UTC").is_err(), "{expression}");
        }
    }
    #[test]
    fn dst_gaps_are_skipped_and_only_the_first_repeated_time_matches() {
        let spring = Schedule::parse("0 30 2 * * *", "America/New_York").unwrap();
        for hour in 0..24 {
            assert!(!spring.matches(instant(&format!("2026-03-08T{hour:02}:30:00Z"))));
        }
        let autumn = Schedule::parse("0 30 1 * * *", "America/New_York").unwrap();
        assert!(autumn.matches(instant("2026-11-01T05:30:00Z")));
        assert!(!autumn.matches(instant("2026-11-01T06:30:00Z")));
        let half_hour = Schedule::parse("0 45 1 * * *", "Australia/Lord_Howe").unwrap();
        assert!(half_hour.matches(instant("2026-04-04T14:45:00Z")));
        assert!(!half_hour.matches(instant("2026-04-04T15:15:00Z")));
    }
}

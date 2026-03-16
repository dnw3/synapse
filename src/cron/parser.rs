//! Cron expression parser for standard 5-field cron syntax.
//!
//! Supports:
//! - `*` wildcard
//! - Specific values (`5`, `12`)
//! - Ranges (`1-5`, `MON-FRI`)
//! - Lists (`1,3,5`, `MON,WED,FRI`)
//! - Step values (`*/2`, `0-12/3`)
//! - Day-of-week names (`MON`, `TUE`, `WED`, `THU`, `FRI`, `SAT`, `SUN`)
//! - Month names (`JAN`–`DEC`)
//!
//! Field order: `minute hour day-of-month month day-of-week`

use chrono::{DateTime, Datelike, Duration, Timelike, Utc};

/// Parsed representation of a single cron field.
#[derive(Debug, Clone)]
enum Field {
    /// Match any value.
    Any,
    /// Match a specific set of values.
    Values(Vec<u32>),
}

impl Field {
    fn matches(&self, value: u32) -> bool {
        match self {
            Field::Any => true,
            Field::Values(v) => v.contains(&value),
        }
    }
}

/// Parsed 5-field cron schedule.
#[derive(Debug, Clone)]
struct CronSchedule {
    minutes: Field,
    hours: Field,
    days_of_month: Field,
    months: Field,
    days_of_week: Field,
}

/// Parse a named day-of-week abbreviation to 0-based Sunday index (0=Sun..6=Sat).
fn parse_dow_name(s: &str) -> Option<u32> {
    match s.to_uppercase().as_str() {
        "SUN" => Some(0),
        "MON" => Some(1),
        "TUE" => Some(2),
        "WED" => Some(3),
        "THU" => Some(4),
        "FRI" => Some(5),
        "SAT" => Some(6),
        _ => None,
    }
}

/// Parse a named month abbreviation to 1-based month index (1=Jan..12=Dec).
fn parse_month_name(s: &str) -> Option<u32> {
    match s.to_uppercase().as_str() {
        "JAN" => Some(1),
        "FEB" => Some(2),
        "MAR" => Some(3),
        "APR" => Some(4),
        "MAY" => Some(5),
        "JUN" => Some(6),
        "JUL" => Some(7),
        "AUG" => Some(8),
        "SEP" => Some(9),
        "OCT" => Some(10),
        "NOV" => Some(11),
        "DEC" => Some(12),
        _ => None,
    }
}

/// Parse a single token as an integer, optionally resolving named aliases.
/// `is_dow` selects day-of-week name resolution, otherwise month name resolution.
fn parse_value(token: &str, is_dow: bool, is_month: bool) -> Option<u32> {
    if let Ok(n) = token.parse::<u32>() {
        return Some(n);
    }
    if is_dow {
        return parse_dow_name(token);
    }
    if is_month {
        return parse_month_name(token);
    }
    None
}

/// Parse a single cron field string into a [`Field`].
///
/// `min`/`max` define the legal range for the field.
/// `is_dow` / `is_month` enable named-alias resolution.
fn parse_field(expr: &str, min: u32, max: u32, is_dow: bool, is_month: bool) -> Option<Field> {
    if expr == "*" {
        return Some(Field::Any);
    }

    let mut values: Vec<u32> = Vec::new();

    for part in expr.split(',') {
        // Handle step syntax: `*/N` or `R-R/N` or `N/N`
        let (range_part, step) = if let Some(slash) = part.find('/') {
            let step_str = &part[slash + 1..];
            let step: u32 = step_str.parse().ok()?;
            if step == 0 {
                return None;
            }
            (&part[..slash], Some(step))
        } else {
            (part, None)
        };

        if range_part == "*" {
            // `*/N` — full range with step
            let s = step.unwrap_or(1);
            let mut v = min;
            while v <= max {
                values.push(v);
                v += s;
            }
        } else if let Some(dash) = range_part.find('-') {
            // Range: `low-high` or `MON-FRI`
            let low_str = &range_part[..dash];
            let high_str = &range_part[dash + 1..];
            let low = parse_value(low_str, is_dow, is_month)?;
            let high = parse_value(high_str, is_dow, is_month)?;
            if low > high || low < min || high > max {
                return None;
            }
            let s = step.unwrap_or(1);
            let mut v = low;
            while v <= high {
                values.push(v);
                v += s;
            }
        } else {
            // Single value, possibly with step (e.g. `5/2` means start at 5, step 2 to max)
            let val = parse_value(range_part, is_dow, is_month)?;
            if val < min || val > max {
                return None;
            }
            if let Some(s) = step {
                let mut v = val;
                while v <= max {
                    values.push(v);
                    v += s;
                }
            } else {
                values.push(val);
            }
        }
    }

    values.sort_unstable();
    values.dedup();
    Some(Field::Values(values))
}

/// Parse a 5-field cron expression string.
fn parse(expr: &str) -> Option<CronSchedule> {
    let fields: Vec<&str> = expr.split_whitespace().collect();
    if fields.len() != 5 {
        return None;
    }

    let minutes = parse_field(fields[0], 0, 59, false, false)?;
    let hours = parse_field(fields[1], 0, 23, false, false)?;
    let days_of_month = parse_field(fields[2], 1, 31, false, false)?;
    let months = parse_field(fields[3], 1, 12, false, true)?;
    let days_of_week = parse_field(fields[4], 0, 6, true, false)?;

    Some(CronSchedule {
        minutes,
        hours,
        days_of_month,
        months,
        days_of_week,
    })
}

impl CronSchedule {
    /// Check whether a given UTC datetime matches this schedule.
    ///
    /// Follows the standard cron convention: if both `day-of-month` and
    /// `day-of-week` are restricted (non-`*`), the datetime matches when
    /// *either* condition is true (OR semantics).  When only one is
    /// restricted, only that condition is tested.
    fn matches(&self, dt: &DateTime<Utc>) -> bool {
        let minute = dt.minute();
        let hour = dt.hour();
        let dom = dt.day();
        let month = dt.month();
        // chrono weekday: Mon=0 .. Sun=6; cron convention: Sun=0 .. Sat=6
        let dow = dt.weekday().num_days_from_sunday();

        if !self.minutes.matches(minute) {
            return false;
        }
        if !self.hours.matches(hour) {
            return false;
        }
        if !self.months.matches(month) {
            return false;
        }

        // Day matching: OR semantics when both are restricted
        let dom_restricted = !matches!(self.days_of_month, Field::Any);
        let dow_restricted = !matches!(self.days_of_week, Field::Any);

        match (dom_restricted, dow_restricted) {
            (true, true) => {
                // Either day-of-month OR day-of-week must match
                self.days_of_month.matches(dom) || self.days_of_week.matches(dow)
            }
            (true, false) => self.days_of_month.matches(dom),
            (false, true) => self.days_of_week.matches(dow),
            (false, false) => true,
        }
    }

    /// Compute the next firing time strictly after `after`.
    ///
    /// Iterates minute-by-minute up to a maximum of 366 days to handle edge
    /// cases like Feb 29.  Returns `None` if no match is found within that
    /// window (should not happen for well-formed schedules).
    fn next_after(&self, after: &DateTime<Utc>) -> Option<DateTime<Utc>> {
        // Start from the next whole minute after `after`
        let mut candidate = *after + Duration::minutes(1);
        // Zero out seconds/nanoseconds
        candidate = candidate
            .with_second(0)
            .and_then(|d| d.with_nanosecond(0))?;

        let limit = *after + Duration::days(366);

        while candidate <= limit {
            if self.matches(&candidate) {
                return Some(candidate);
            }
            candidate += Duration::minutes(1);
        }
        None
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Stateless cron expression parser and evaluator.
pub struct CronParser;

impl CronParser {
    /// Return `true` if `expr` is a syntactically valid 5-field cron expression.
    pub fn is_valid(expr: &str) -> bool {
        parse(expr).is_some()
    }

    /// Compute the next firing time strictly after `after`.
    ///
    /// Returns `None` if the expression is invalid or no candidate is found
    /// within a 366-day window.
    pub fn next_after(expr: &str, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        parse(expr)?.next_after(&after)
    }

    /// Return `true` if the given datetime matches the cron expression.
    ///
    /// Only minute-resolution is considered (seconds are ignored).
    pub fn matches(expr: &str, dt: &DateTime<Utc>) -> bool {
        parse(expr).map(|s| s.matches(dt)).unwrap_or(false)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn utc(year: i32, month: u32, day: u32, hour: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, min, 0)
            .unwrap()
    }

    // ── is_valid ─────────────────────────────────────────────────────────────

    #[test]
    fn test_valid_wildcard_all() {
        assert!(CronParser::is_valid("* * * * *"));
    }

    #[test]
    fn test_valid_specific() {
        assert!(CronParser::is_valid("0 9 * * *"));
        assert!(CronParser::is_valid("30 14 1 1 *"));
    }

    #[test]
    fn test_valid_range() {
        assert!(CronParser::is_valid("0 9 * * MON-FRI"));
        assert!(CronParser::is_valid("0 0 1-15 * *"));
    }

    #[test]
    fn test_valid_list() {
        assert!(CronParser::is_valid("0 9 * * 1,3,5"));
        assert!(CronParser::is_valid("0,30 * * * *"));
    }

    #[test]
    fn test_valid_step() {
        assert!(CronParser::is_valid("30 */2 * * *"));
        assert!(CronParser::is_valid("*/15 * * * *"));
    }

    #[test]
    fn test_invalid_too_few_fields() {
        assert!(!CronParser::is_valid("* * * *"));
    }

    #[test]
    fn test_invalid_too_many_fields() {
        assert!(!CronParser::is_valid("* * * * * *"));
    }

    #[test]
    fn test_invalid_out_of_range() {
        assert!(!CronParser::is_valid("60 * * * *")); // minute 60 invalid
        assert!(!CronParser::is_valid("* 24 * * *")); // hour 24 invalid
    }

    // ── matches ──────────────────────────────────────────────────────────────

    #[test]
    fn test_matches_wildcard() {
        let dt = utc(2024, 6, 15, 12, 30);
        assert!(CronParser::matches("* * * * *", &dt));
    }

    #[test]
    fn test_matches_specific_minute_hour() {
        let dt = utc(2024, 6, 15, 9, 0);
        assert!(CronParser::matches("0 9 * * *", &dt));
        assert!(!CronParser::matches("0 9 * * *", &utc(2024, 6, 15, 9, 1)));
    }

    #[test]
    fn test_matches_mon_fri() {
        // 2024-06-17 is a Monday (weekday index 1 in Sun=0 convention)
        let monday = utc(2024, 6, 17, 9, 0);
        assert!(CronParser::matches("0 9 * * MON-FRI", &monday));

        // 2024-06-15 is a Saturday
        let saturday = utc(2024, 6, 15, 9, 0);
        assert!(!CronParser::matches("0 9 * * MON-FRI", &saturday));
    }

    #[test]
    fn test_matches_step() {
        // "30 */2 * * *" fires at minute 30 of every even hour (0,2,4,...,22)
        assert!(CronParser::matches("30 */2 * * *", &utc(2024, 1, 1, 0, 30)));
        assert!(CronParser::matches("30 */2 * * *", &utc(2024, 1, 1, 4, 30)));
        assert!(!CronParser::matches(
            "30 */2 * * *",
            &utc(2024, 1, 1, 1, 30)
        ));
        assert!(!CronParser::matches("30 */2 * * *", &utc(2024, 1, 1, 4, 0)));
    }

    #[test]
    fn test_matches_month_name() {
        let jan = utc(2024, 1, 5, 0, 0);
        assert!(CronParser::matches("0 0 * JAN *", &jan));
        assert!(!CronParser::matches("0 0 * FEB *", &jan));
    }

    // ── next_after ───────────────────────────────────────────────────────────

    #[test]
    fn test_next_after_wildcard() {
        let after = utc(2024, 6, 15, 12, 30);
        let next = CronParser::next_after("* * * * *", after).unwrap();
        assert_eq!(next, utc(2024, 6, 15, 12, 31));
    }

    #[test]
    fn test_next_after_hourly() {
        // "0 9 * * *" — after 09:00 today, fires at 09:00 tomorrow
        let after = utc(2024, 6, 15, 9, 0);
        let next = CronParser::next_after("0 9 * * *", after).unwrap();
        assert_eq!(next, utc(2024, 6, 16, 9, 0));
    }

    #[test]
    fn test_next_after_mon_fri() {
        // After Friday 2024-06-14 09:00, next MON-FRI 09:00 is Monday 2024-06-17 09:00
        let after = utc(2024, 6, 14, 9, 0);
        let next = CronParser::next_after("0 9 * * MON-FRI", after).unwrap();
        assert_eq!(next, utc(2024, 6, 17, 9, 0));
    }

    #[test]
    fn test_next_after_step_every_2_hours() {
        // "30 */2 * * *" — after 04:30, next is 06:30
        let after = utc(2024, 1, 1, 4, 30);
        let next = CronParser::next_after("30 */2 * * *", after).unwrap();
        assert_eq!(next, utc(2024, 1, 1, 6, 30));
    }

    #[test]
    fn test_next_after_list() {
        // "0 9 * * 1,3,5" (Mon/Wed/Fri at 09:00)
        // 2024-06-16 is Sunday; next should be Mon 2024-06-17
        let after = utc(2024, 6, 16, 9, 0);
        let next = CronParser::next_after("0 9 * * 1,3,5", after).unwrap();
        assert_eq!(next, utc(2024, 6, 17, 9, 0));
    }

    #[test]
    fn test_next_after_invalid_returns_none() {
        let after = utc(2024, 1, 1, 0, 0);
        assert!(CronParser::next_after("not a cron", after).is_none());
    }
}

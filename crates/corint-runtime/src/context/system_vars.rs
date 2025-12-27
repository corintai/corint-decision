//! System Variables Builder
//!
//! Builds system-injected metadata including timestamps, date/time components,
//! and environment information.

use chrono::{Datelike, Timelike};
use corint_core::Value;
use std::collections::HashMap;

/// Build system variables (sys namespace)
///
/// Generates a comprehensive set of system variables including:
/// - Request identification
/// - Time information (timestamps, date/time components)
/// - Business context (business hours, weekday/weekend)
/// - Environment metadata
pub(super) fn build_system_vars() -> HashMap<String, Value> {
    let mut sys = HashMap::new();
    let now = chrono::Utc::now();

    // Request identification
    sys.insert(
        "request_id".to_string(),
        Value::String(uuid::Uuid::new_v4().to_string()),
    );

    // Time information - ISO formats
    sys.insert("timestamp".to_string(), Value::String(now.to_rfc3339()));
    sys.insert(
        "timestamp_ms".to_string(),
        Value::Number(now.timestamp_millis() as f64),
    );
    sys.insert(
        "timestamp_sec".to_string(),
        Value::Number(now.timestamp() as f64),
    );

    // Date components
    sys.insert(
        "date".to_string(),
        Value::String(now.format("%Y-%m-%d").to_string()),
    );
    sys.insert("year".to_string(), Value::Number(now.year() as f64));
    sys.insert("month".to_string(), Value::Number(now.month() as f64));
    sys.insert("day".to_string(), Value::Number(now.day() as f64));

    // Month name
    let month_name = match now.month() {
        1 => "january",
        2 => "february",
        3 => "march",
        4 => "april",
        5 => "may",
        6 => "june",
        7 => "july",
        8 => "august",
        9 => "september",
        10 => "october",
        11 => "november",
        12 => "december",
        _ => "unknown",
    };
    sys.insert("month_name".to_string(), Value::String(month_name.to_string()));

    // Quarter
    let quarter = ((now.month() - 1) / 3) + 1;
    sys.insert("quarter".to_string(), Value::Number(quarter as f64));

    // Time components
    sys.insert(
        "time".to_string(),
        Value::String(now.format("%H:%M:%S").to_string()),
    );
    sys.insert("hour".to_string(), Value::Number(now.hour() as f64));
    sys.insert("minute".to_string(), Value::Number(now.minute() as f64));
    sys.insert("second".to_string(), Value::Number(now.second() as f64));

    // Time of day periods
    let time_of_day = match now.hour() {
        0..=5 => "night",
        6..=11 => "morning",
        12..=17 => "afternoon",
        18..=21 => "evening",
        _ => "night",
    };
    sys.insert("time_of_day".to_string(), Value::String(time_of_day.to_string()));

    // Business hours (9 AM - 5 PM)
    let is_business_hours = now.hour() >= 9 && now.hour() < 17;
    sys.insert("is_business_hours".to_string(), Value::Bool(is_business_hours));

    // Day of week
    let day_of_week = match now.weekday() {
        chrono::Weekday::Mon => "monday",
        chrono::Weekday::Tue => "tuesday",
        chrono::Weekday::Wed => "wednesday",
        chrono::Weekday::Thu => "thursday",
        chrono::Weekday::Fri => "friday",
        chrono::Weekday::Sat => "saturday",
        chrono::Weekday::Sun => "sunday",
    };
    sys.insert(
        "day_of_week".to_string(),
        Value::String(day_of_week.to_string()),
    );

    // Day of week as number (1=Monday, 7=Sunday)
    let day_of_week_num = match now.weekday() {
        chrono::Weekday::Mon => 1,
        chrono::Weekday::Tue => 2,
        chrono::Weekday::Wed => 3,
        chrono::Weekday::Thu => 4,
        chrono::Weekday::Fri => 5,
        chrono::Weekday::Sat => 6,
        chrono::Weekday::Sun => 7,
    };
    sys.insert("day_of_week_num".to_string(), Value::Number(day_of_week_num as f64));

    // Weekend and weekday flags
    let is_weekend = matches!(now.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun);
    sys.insert("is_weekend".to_string(), Value::Bool(is_weekend));
    sys.insert("is_weekday".to_string(), Value::Bool(!is_weekend));

    // Day of year
    sys.insert("day_of_year".to_string(), Value::Number(now.ordinal() as f64));

    // Environment information
    sys.insert(
        "environment".to_string(),
        Value::String(
            std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
        ),
    );

    // Version information
    sys.insert(
        "corint_version".to_string(),
        Value::String(env!("CARGO_PKG_VERSION").to_string()),
    );

    sys
}

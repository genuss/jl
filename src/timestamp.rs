use chrono::{DateTime, FixedOffset, Local, TimeZone, Utc};
use chrono_tz::Tz;
use serde_json::Value;

use crate::error::JlError;

/// Attempt to parse a JSON value as a timestamp.
///
/// Supports:
/// - ISO 8601 strings (e.g. "2024-01-15T10:30:00Z", "2024-01-15T10:30:00+05:30")
/// - Epoch seconds as f64 or i64
/// - Epoch milliseconds as i64 (values >= 1e12)
pub fn parse_timestamp(value: &Value) -> Option<DateTime<FixedOffset>> {
    match value {
        Value::String(s) => parse_iso8601(s),
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                parse_epoch(f)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Parse an ISO 8601 timestamp string.
fn parse_iso8601(s: &str) -> Option<DateTime<FixedOffset>> {
    // Try parsing with timezone info first
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt);
    }
    // Try common ISO 8601 variants without timezone (assume UTC)
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(dt.and_utc().fixed_offset());
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(dt.and_utc().fixed_offset());
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
        return Some(dt.and_utc().fixed_offset());
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.and_utc().fixed_offset());
    }
    None
}

/// Parse an epoch-based numeric timestamp.
/// Values >= 1e12 are treated as milliseconds, otherwise as seconds.
fn parse_epoch(value: f64) -> Option<DateTime<FixedOffset>> {
    if !value.is_finite() {
        return None;
    }
    let (secs, nanos) = if value.abs() >= 1e12 {
        // Epoch milliseconds - use Euclidean division for correct negative handling
        let millis = value as i64;
        let secs = millis.div_euclid(1000);
        let remaining_millis = millis.rem_euclid(1000) as u32;
        (secs, remaining_millis * 1_000_000)
    } else {
        // Epoch seconds - use floor for correct negative value handling
        let secs = value.floor() as i64;
        let frac = value - value.floor();
        let nanos = (frac * 1e9) as u32;
        (secs, nanos)
    };

    Utc.timestamp_opt(secs, nanos)
        .single()
        .map(|dt| dt.fixed_offset())
}

/// Format a parsed timestamp for display, converting to the requested timezone.
///
/// `tz` can be:
/// - "local" - use the system local timezone
/// - "utc" or "UTC" - use UTC
/// - An IANA timezone name (e.g. "America/New_York", "Europe/London")
pub fn format_timestamp(ts: &DateTime<FixedOffset>, tz: &str) -> Result<String, JlError> {
    let formatted = match tz.to_ascii_lowercase().as_str() {
        "local" => {
            let local_dt = ts.with_timezone(&Local);
            local_dt.format("%Y-%m-%dT%H:%M:%S%.3f%:z").to_string()
        }
        "utc" => {
            let utc_dt = ts.with_timezone(&Utc);
            utc_dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
        }
        _ => {
            let named_tz: Tz = tz
                .parse()
                .map_err(|_| JlError::Tz(format!("unknown timezone: {tz}")))?;
            let converted = ts.with_timezone(&named_tz);
            converted.format("%Y-%m-%dT%H:%M:%S%.3f%:z").to_string()
        }
    };
    Ok(formatted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- parse_timestamp tests ---

    #[test]
    fn parse_iso8601_rfc3339_utc() {
        let val = json!("2024-01-15T10:30:00Z");
        let ts = parse_timestamp(&val).unwrap();
        assert_eq!(ts.to_rfc3339(), "2024-01-15T10:30:00+00:00");
    }

    #[test]
    fn parse_iso8601_with_offset() {
        let val = json!("2024-01-15T10:30:00+05:30");
        let ts = parse_timestamp(&val).unwrap();
        assert_eq!(ts.to_rfc3339(), "2024-01-15T10:30:00+05:30");
    }

    #[test]
    fn parse_iso8601_with_fractional_seconds() {
        let val = json!("2024-01-15T10:30:00.123Z");
        let ts = parse_timestamp(&val).unwrap();
        assert_eq!(ts.timestamp(), 1705314600);
        assert_eq!(ts.timestamp_subsec_millis(), 123);
    }

    #[test]
    fn parse_iso8601_no_timezone_assumes_utc() {
        let val = json!("2024-01-15T10:30:00");
        let ts = parse_timestamp(&val).unwrap();
        assert_eq!(ts.offset().local_minus_utc(), 0);
    }

    #[test]
    fn parse_iso8601_space_separator() {
        let val = json!("2024-01-15 10:30:00");
        let ts = parse_timestamp(&val).unwrap();
        assert_eq!(ts.timestamp(), 1705314600);
    }

    #[test]
    fn parse_iso8601_space_with_fractional() {
        let val = json!("2024-01-15 10:30:00.456");
        let ts = parse_timestamp(&val).unwrap();
        assert_eq!(ts.timestamp_subsec_millis(), 456);
    }

    #[test]
    fn parse_epoch_seconds_integer() {
        let val = json!(1705314600);
        let ts = parse_timestamp(&val).unwrap();
        assert_eq!(ts.to_rfc3339(), "2024-01-15T10:30:00+00:00");
    }

    #[test]
    fn parse_epoch_seconds_float() {
        let val = json!(1705314600.5);
        let ts = parse_timestamp(&val).unwrap();
        assert_eq!(ts.timestamp(), 1705314600);
        assert!(ts.timestamp_subsec_millis() >= 499 && ts.timestamp_subsec_millis() <= 501);
    }

    #[test]
    fn parse_epoch_millis() {
        let val = json!(1705314600000_i64);
        let ts = parse_timestamp(&val).unwrap();
        assert_eq!(ts.to_rfc3339(), "2024-01-15T10:30:00+00:00");
    }

    #[test]
    fn parse_epoch_millis_with_remainder() {
        let val = json!(1705314600123_i64);
        let ts = parse_timestamp(&val).unwrap();
        assert_eq!(ts.timestamp_subsec_millis(), 123);
    }

    #[test]
    fn parse_invalid_string() {
        let val = json!("not a timestamp");
        assert!(parse_timestamp(&val).is_none());
    }

    #[test]
    fn parse_null() {
        let val = json!(null);
        assert!(parse_timestamp(&val).is_none());
    }

    #[test]
    fn parse_boolean() {
        let val = json!(true);
        assert!(parse_timestamp(&val).is_none());
    }

    #[test]
    fn parse_array() {
        let val = json!([1, 2, 3]);
        assert!(parse_timestamp(&val).is_none());
    }

    #[test]
    fn parse_object() {
        let val = json!({"time": 123});
        assert!(parse_timestamp(&val).is_none());
    }

    #[test]
    fn parse_empty_string() {
        let val = json!("");
        assert!(parse_timestamp(&val).is_none());
    }

    // --- format_timestamp / timezone conversion tests ---

    #[test]
    fn format_utc() {
        let ts = DateTime::parse_from_rfc3339("2024-01-15T10:30:00+00:00").unwrap();
        let formatted = format_timestamp(&ts, "utc").unwrap();
        assert_eq!(formatted, "2024-01-15T10:30:00.000Z");
    }

    #[test]
    fn format_utc_uppercase() {
        let ts = DateTime::parse_from_rfc3339("2024-01-15T10:30:00+00:00").unwrap();
        let formatted = format_timestamp(&ts, "UTC").unwrap();
        assert_eq!(formatted, "2024-01-15T10:30:00.000Z");
    }

    #[test]
    fn format_named_timezone() {
        let ts = DateTime::parse_from_rfc3339("2024-01-15T10:30:00+00:00").unwrap();
        let formatted = format_timestamp(&ts, "America/New_York").unwrap();
        // EST is UTC-5
        assert_eq!(formatted, "2024-01-15T05:30:00.000-05:00");
    }

    #[test]
    fn format_named_timezone_positive_offset() {
        let ts = DateTime::parse_from_rfc3339("2024-01-15T10:30:00+00:00").unwrap();
        let formatted = format_timestamp(&ts, "Asia/Tokyo").unwrap();
        // JST is UTC+9
        assert_eq!(formatted, "2024-01-15T19:30:00.000+09:00");
    }

    #[test]
    fn format_local_timezone() {
        let ts = DateTime::parse_from_rfc3339("2024-01-15T10:30:00+00:00").unwrap();
        let formatted = format_timestamp(&ts, "local").unwrap();
        // Can't assert exact value since it depends on the system timezone,
        // but it should be a valid timestamp string
        assert!(formatted.contains("2024-01-15"));
        assert!(formatted.contains(":"));
    }

    #[test]
    fn format_invalid_timezone() {
        let ts = DateTime::parse_from_rfc3339("2024-01-15T10:30:00+00:00").unwrap();
        let result = format_timestamp(&ts, "Invalid/Timezone");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(format!("{err}").contains("unknown timezone"));
    }

    #[test]
    fn format_preserves_subsecond_precision() {
        let ts = DateTime::parse_from_rfc3339("2024-01-15T10:30:00.123+00:00").unwrap();
        let formatted = format_timestamp(&ts, "utc").unwrap();
        assert!(formatted.contains(".123"));
    }

    #[test]
    fn format_with_source_offset() {
        // Timestamp with +05:30 offset, display in UTC
        let ts = DateTime::parse_from_rfc3339("2024-01-15T16:00:00+05:30").unwrap();
        let formatted = format_timestamp(&ts, "utc").unwrap();
        assert_eq!(formatted, "2024-01-15T10:30:00.000Z");
    }
}

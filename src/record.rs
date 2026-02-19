use std::collections::BTreeMap;

use serde_json::Value;

use crate::cli::TsFormat;
use crate::error::JlError;
use crate::level::Level;
use crate::schema::FieldMapping;
use crate::timestamp;

/// A structured log record extracted from a JSON log line.
#[derive(Debug, Clone)]
pub struct LogRecord {
    pub level: Option<Level>,
    pub timestamp: Option<String>,
    pub logger: Option<String>,
    pub message: Option<String>,
    pub stack_trace: Option<String>,
    pub extras: BTreeMap<String, Value>,
    pub raw: Value,
}

impl LogRecord {
    /// Extract a structured log record from a JSON value using the given field mapping.
    ///
    /// Pulls canonical fields (level, timestamp, logger, message, stack_trace)
    /// based on the mapping, parses level (string or Bunyan numeric), formats
    /// the timestamp using the given timezone, and collects remaining fields as extras.
    pub fn extract(
        value: Value,
        mapping: &FieldMapping,
        tz: &str,
        ts_format: TsFormat,
    ) -> Result<LogRecord, JlError> {
        let obj = match value.as_object() {
            Some(o) => o,
            None => {
                return Ok(LogRecord {
                    level: None,
                    timestamp: None,
                    logger: None,
                    message: Some(value.to_string()),
                    stack_trace: None,
                    extras: BTreeMap::new(),
                    raw: value,
                });
            }
        };

        // Find matching keys for each canonical field
        let level_key = FieldMapping::find_key(&mapping.level, obj).map(String::from);
        let ts_key = FieldMapping::find_key(&mapping.timestamp, obj).map(String::from);
        let logger_key = FieldMapping::find_key(&mapping.logger, obj).map(String::from);
        let message_key = FieldMapping::find_key(&mapping.message, obj).map(String::from);
        let stack_key = FieldMapping::find_key(&mapping.stack_trace, obj).map(String::from);

        // Extract level
        let level = level_key.as_deref().and_then(|key| {
            let val = obj.get(key)?;
            parse_level(val)
        });

        // Extract and format timestamp
        let timestamp = match ts_key.as_deref() {
            Some(key) => match obj.get(key) {
                Some(val) => match timestamp::parse_timestamp(val) {
                    Some(ts) => Some(timestamp::format_timestamp(&ts, tz, ts_format)?),
                    None => Some(value_to_string(val)),
                },
                None => None,
            },
            None => None,
        };

        // Extract logger
        let logger = logger_key
            .as_deref()
            .and_then(|key| obj.get(key))
            .map(value_to_string);

        // Extract message
        let message = message_key
            .as_deref()
            .and_then(|key| obj.get(key))
            .map(value_to_string);

        // Extract stack trace
        let stack_trace = stack_key
            .as_deref()
            .and_then(|key| obj.get(key))
            .map(value_to_string);

        // Collect canonical keys to exclude from extras
        let canonical_keys: Vec<&str> =
            [&level_key, &ts_key, &logger_key, &message_key, &stack_key]
                .iter()
                .filter_map(|k| k.as_deref())
                .collect();

        // Collect remaining fields as extras
        let extras: BTreeMap<String, Value> = obj
            .iter()
            .filter(|(k, _)| !canonical_keys.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        Ok(LogRecord {
            level,
            timestamp,
            logger,
            message,
            stack_trace,
            extras,
            raw: value,
        })
    }
}

/// Parse a level from a JSON value - handles both string and numeric (Bunyan) levels.
fn parse_level(val: &Value) -> Option<Level> {
    match val {
        Value::String(s) => s.parse::<Level>().ok(),
        Value::Number(n) => n.as_i64().and_then(Level::from_bunyan_int),
        _ => None,
    }
}

/// Convert a JSON value to a display string, stripping quotes from strings.
fn value_to_string(val: &Value) -> String {
    match val {
        Value::String(s) => s.clone(),
        _ => val.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Schema;
    use serde_json::json;

    // --- LogRecord extraction with Logstash schema ---

    #[test]
    fn extract_logstash_full() {
        let mapping = Schema::Logstash.field_mapping();
        let value = json!({
            "@timestamp": "2024-01-15T10:30:00Z",
            "level": "INFO",
            "logger_name": "com.example.App",
            "message": "Application started",
            "thread_name": "main"
        });
        let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
        assert_eq!(record.level, Some(Level::Info));
        assert!(record.timestamp.as_ref().unwrap().contains("2024-01-15"));
        assert_eq!(record.logger.as_deref(), Some("com.example.App"));
        assert_eq!(record.message.as_deref(), Some("Application started"));
        assert!(record.stack_trace.is_none());
        // thread_name should be in extras
        assert!(record.extras.contains_key("thread_name"));
        assert_eq!(record.extras.get("thread_name").unwrap(), "main");
    }

    #[test]
    fn extract_logstash_with_stack_trace() {
        let mapping = Schema::Logstash.field_mapping();
        let value = json!({
            "@timestamp": "2024-01-15T10:30:00Z",
            "level": "ERROR",
            "logger_name": "com.example",
            "message": "Something failed",
            "stack_trace": "java.lang.NullPointerException\n\tat com.example.Foo.bar(Foo.java:42)"
        });
        let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
        assert_eq!(record.level, Some(Level::Error));
        assert!(record.stack_trace.is_some());
        assert!(
            record
                .stack_trace
                .as_ref()
                .unwrap()
                .contains("NullPointerException")
        );
    }

    // --- LogRecord extraction with Logrus schema ---

    #[test]
    fn extract_logrus_full() {
        let mapping = Schema::Logrus.field_mapping();
        let value = json!({
            "level": "warning",
            "msg": "connection pool exhausted",
            "time": "2024-01-15T10:30:00Z",
            "component": "database",
            "retry_count": 3
        });
        let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
        assert_eq!(record.level, Some(Level::Warn));
        assert!(record.timestamp.is_some());
        assert_eq!(record.logger.as_deref(), Some("database"));
        assert_eq!(record.message.as_deref(), Some("connection pool exhausted"));
        assert!(record.extras.contains_key("retry_count"));
    }

    // --- LogRecord extraction with Bunyan schema ---

    #[test]
    fn extract_bunyan_numeric_level() {
        let mapping = Schema::Bunyan.field_mapping();
        let value = json!({
            "v": 0,
            "level": 30,
            "name": "myapp",
            "hostname": "server1",
            "pid": 1234,
            "time": "2024-01-15T10:30:00.000Z",
            "msg": "request completed"
        });
        let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
        assert_eq!(record.level, Some(Level::Info));
        assert_eq!(record.logger.as_deref(), Some("myapp"));
        assert_eq!(record.message.as_deref(), Some("request completed"));
        // v, hostname, pid should be in extras
        assert!(record.extras.contains_key("v"));
        assert!(record.extras.contains_key("hostname"));
        assert!(record.extras.contains_key("pid"));
    }

    #[test]
    fn extract_bunyan_error_level() {
        let mapping = Schema::Bunyan.field_mapping();
        let value = json!({
            "v": 0,
            "level": 50,
            "name": "myapp",
            "time": "2024-01-15T10:30:00Z",
            "msg": "fatal error"
        });
        let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
        assert_eq!(record.level, Some(Level::Error));
    }

    #[test]
    fn extract_bunyan_fatal_level() {
        let mapping = Schema::Bunyan.field_mapping();
        let value = json!({
            "v": 0,
            "level": 60,
            "name": "myapp",
            "time": "2024-01-15T10:30:00Z",
            "msg": "system shutdown"
        });
        let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
        assert_eq!(record.level, Some(Level::Fatal));
    }

    // --- LogRecord extraction with Generic schema ---

    #[test]
    fn extract_generic_with_common_fields() {
        let mapping = Schema::Generic.field_mapping();
        let value = json!({
            "severity": "DEBUG",
            "timestamp": "2024-01-15T10:30:00Z",
            "source": "auth_service",
            "text": "User login attempt",
            "user_id": "abc123"
        });
        let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
        assert_eq!(record.level, Some(Level::Debug));
        assert!(record.timestamp.is_some());
        assert_eq!(record.logger.as_deref(), Some("auth_service"));
        assert_eq!(record.message.as_deref(), Some("User login attempt"));
        assert!(record.extras.contains_key("user_id"));
    }

    #[test]
    fn extract_generic_with_msg_field() {
        let mapping = Schema::Generic.field_mapping();
        let value = json!({
            "level": "error",
            "msg": "failed to connect"
        });
        let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
        assert_eq!(record.level, Some(Level::Error));
        assert_eq!(record.message.as_deref(), Some("failed to connect"));
    }

    // --- Edge cases ---

    #[test]
    fn extract_non_object_value() {
        let mapping = Schema::Logstash.field_mapping();
        let value = json!("just a string");
        let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
        assert!(record.level.is_none());
        assert!(record.timestamp.is_none());
        assert_eq!(record.message.as_deref(), Some("\"just a string\""));
        assert!(record.extras.is_empty());
    }

    #[test]
    fn extract_empty_object() {
        let mapping = Schema::Logstash.field_mapping();
        let value = json!({});
        let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
        assert!(record.level.is_none());
        assert!(record.timestamp.is_none());
        assert!(record.logger.is_none());
        assert!(record.message.is_none());
        assert!(record.extras.is_empty());
    }

    #[test]
    fn extract_missing_some_fields() {
        let mapping = Schema::Logstash.field_mapping();
        let value = json!({
            "level": "WARN",
            "message": "incomplete record"
        });
        let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
        assert_eq!(record.level, Some(Level::Warn));
        assert!(record.timestamp.is_none());
        assert!(record.logger.is_none());
        assert_eq!(record.message.as_deref(), Some("incomplete record"));
    }

    #[test]
    fn extract_unknown_level_string() {
        let mapping = Schema::Logstash.field_mapping();
        let value = json!({
            "level": "VERBOSE",
            "message": "test"
        });
        let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
        // "VERBOSE" is not a recognized level
        assert!(record.level.is_none());
    }

    #[test]
    fn extract_unknown_bunyan_level_number() {
        let mapping = Schema::Bunyan.field_mapping();
        let value = json!({
            "level": 99,
            "msg": "test"
        });
        let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
        assert!(record.level.is_none());
    }

    #[test]
    fn extract_numeric_message_field() {
        let mapping = Schema::Logstash.field_mapping();
        let value = json!({
            "level": "INFO",
            "message": 42
        });
        let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
        assert_eq!(record.message.as_deref(), Some("42"));
    }

    #[test]
    fn extract_epoch_timestamp() {
        let mapping = Schema::Generic.field_mapping();
        let value = json!({
            "level": "INFO",
            "timestamp": 1705314600,
            "message": "epoch seconds"
        });
        let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
        assert!(record.timestamp.is_some());
        let ts = record.timestamp.unwrap();
        assert!(ts.contains("2024-01-15"));
        assert!(ts.contains("10:30:00"));
    }

    #[test]
    fn extract_epoch_millis_timestamp() {
        let mapping = Schema::Generic.field_mapping();
        let value = json!({
            "level": "INFO",
            "timestamp": 1705314600123_i64,
            "message": "epoch millis"
        });
        let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
        assert!(record.timestamp.is_some());
        let ts = record.timestamp.unwrap();
        assert!(ts.contains("2024-01-15"));
        assert!(ts.contains(".123"));
    }

    #[test]
    fn extract_with_named_timezone() {
        let mapping = Schema::Logstash.field_mapping();
        let value = json!({
            "@timestamp": "2024-01-15T10:30:00Z",
            "level": "INFO",
            "message": "timezone test"
        });
        let record = LogRecord::extract(value, &mapping, "America/New_York", TsFormat::Full).unwrap();
        let ts = record.timestamp.unwrap();
        assert!(ts.contains("05:30:00"));
        // Timezone offset is no longer included in formatted output
        assert!(!ts.contains("-05:00"));
    }

    #[test]
    fn extract_with_invalid_timezone() {
        let mapping = Schema::Logstash.field_mapping();
        let value = json!({
            "@timestamp": "2024-01-15T10:30:00Z",
            "level": "INFO",
            "message": "bad tz"
        });
        let result = LogRecord::extract(value, &mapping, "Invalid/Zone", TsFormat::Full);
        assert!(result.is_err());
    }

    #[test]
    fn extract_unparseable_timestamp_kept_as_string() {
        let mapping = Schema::Logstash.field_mapping();
        let value = json!({
            "@timestamp": "not-a-date",
            "level": "INFO",
            "message": "test"
        });
        let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
        // Unparseable timestamp should be kept as the raw string
        assert_eq!(record.timestamp.as_deref(), Some("not-a-date"));
    }

    #[test]
    fn extract_extras_exclude_canonical_fields() {
        let mapping = Schema::Logstash.field_mapping();
        let value = json!({
            "@timestamp": "2024-01-15T10:30:00Z",
            "level": "INFO",
            "logger_name": "test",
            "message": "hello",
            "extra1": "value1",
            "extra2": 42
        });
        let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
        // Only extra1 and extra2 should be in extras
        assert_eq!(record.extras.len(), 2);
        assert!(record.extras.contains_key("extra1"));
        assert!(record.extras.contains_key("extra2"));
        // Canonical fields should NOT be in extras
        assert!(!record.extras.contains_key("@timestamp"));
        assert!(!record.extras.contains_key("level"));
        assert!(!record.extras.contains_key("logger_name"));
        assert!(!record.extras.contains_key("message"));
    }

    #[test]
    fn extract_preserves_raw_value() {
        let mapping = Schema::Logstash.field_mapping();
        let value = json!({
            "level": "INFO",
            "message": "original"
        });
        let record = LogRecord::extract(value.clone(), &mapping, "utc", TsFormat::Full).unwrap();
        assert_eq!(record.raw, value);
    }

    #[test]
    fn extract_level_case_insensitive() {
        let mapping = Schema::Logstash.field_mapping();
        for level_str in &["info", "Info", "INFO", "iNfO"] {
            let value = json!({
                "level": level_str,
                "message": "test"
            });
            let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
            assert_eq!(record.level, Some(Level::Info), "Failed for: {level_str}");
        }
    }

    #[test]
    fn extract_boolean_level_ignored() {
        let mapping = Schema::Logstash.field_mapping();
        let value = json!({
            "level": true,
            "message": "test"
        });
        let record = LogRecord::extract(value, &mapping, "utc", TsFormat::Full).unwrap();
        assert!(record.level.is_none());
    }
}

use serde_json::Value;

use crate::cli::SchemaChoice;

/// Supported log schemas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schema {
    Logstash,
    Logrus,
    Bunyan,
    Generic,
}

/// Maps canonical field roles to the actual JSON key names for a given schema.
#[derive(Debug, Clone)]
pub struct FieldMapping {
    /// Key name(s) to try for the log level field.
    pub level: Vec<&'static str>,
    /// Key name(s) to try for the timestamp field.
    pub timestamp: Vec<&'static str>,
    /// Key name(s) to try for the logger/source field.
    pub logger: Vec<&'static str>,
    /// Key name(s) to try for the message field.
    pub message: Vec<&'static str>,
    /// Key name(s) to try for the stack trace field.
    pub stack_trace: Vec<&'static str>,
}

impl FieldMapping {
    /// Find the first matching key from the candidates in the given JSON object.
    pub fn find_key<'a>(
        candidates: &[&'static str],
        obj: &'a serde_json::Map<String, Value>,
    ) -> Option<&'a str> {
        candidates
            .iter()
            .find(|&&key| obj.contains_key(key))
            .map(|&key| key as &str)
    }
}

impl Schema {
    /// Return the field mapping for this schema.
    pub fn field_mapping(&self) -> FieldMapping {
        match self {
            Schema::Logstash => FieldMapping {
                level: vec!["level"],
                timestamp: vec!["@timestamp"],
                logger: vec!["logger_name"],
                message: vec!["message"],
                stack_trace: vec!["stack_trace"],
            },
            Schema::Logrus => FieldMapping {
                level: vec!["level"],
                timestamp: vec!["time"],
                logger: vec!["component"],
                message: vec!["msg"],
                stack_trace: vec!["stack_trace", "stacktrace"],
            },
            Schema::Bunyan => FieldMapping {
                level: vec!["level"],
                timestamp: vec!["time"],
                logger: vec!["name"],
                message: vec!["msg"],
                stack_trace: vec!["stack"],
            },
            Schema::Generic => FieldMapping {
                level: vec!["level", "severity", "loglevel", "log_level", "lvl"],
                timestamp: vec!["timestamp", "@timestamp", "time", "ts", "datetime", "date"],
                logger: vec![
                    "logger",
                    "logger_name",
                    "name",
                    "component",
                    "source",
                    "caller",
                ],
                message: vec!["message", "msg", "text", "body", "log"],
                stack_trace: vec![
                    "stack_trace",
                    "stacktrace",
                    "stack",
                    "exception",
                    "traceback",
                ],
            },
        }
    }

    /// Convert from a `SchemaChoice` CLI option, using auto-detection if `Auto`.
    pub fn from_choice(choice: SchemaChoice, value: &Value) -> Schema {
        match choice {
            SchemaChoice::Auto => detect_schema(value),
            SchemaChoice::Logstash => Schema::Logstash,
            SchemaChoice::Logrus => Schema::Logrus,
            SchemaChoice::Bunyan => Schema::Bunyan,
            SchemaChoice::Generic => Schema::Generic,
        }
    }
}

/// Known field names for each schema used in scoring.
const LOGSTASH_FIELDS: &[&str] = &[
    "@timestamp",
    "level",
    "logger_name",
    "message",
    "stack_trace",
    "thread_name",
    "@version",
];

const LOGRUS_FIELDS: &[&str] = &["level", "msg", "time", "component"];

const BUNYAN_FIELDS: &[&str] = &["v", "level", "name", "hostname", "pid", "time", "msg"];

/// Detect the most likely schema for the given JSON value by scoring field name matches.
pub fn detect_schema(value: &Value) -> Schema {
    let obj = match value.as_object() {
        Some(o) => o,
        None => return Schema::Generic,
    };

    let mut logstash_score: i32 = 0;
    let mut logrus_score: i32 = 0;
    let mut bunyan_score: i32 = 0;

    // Score Logstash
    for &field in LOGSTASH_FIELDS {
        if obj.contains_key(field) {
            logstash_score += 1;
        }
    }
    // Bonus: @timestamp is a strong Logstash indicator
    if obj.contains_key("@timestamp") {
        logstash_score += 2;
    }

    // Score Logrus
    for &field in LOGRUS_FIELDS {
        if obj.contains_key(field) {
            logrus_score += 1;
        }
    }

    // Score Bunyan
    for &field in BUNYAN_FIELDS {
        if obj.contains_key(field) {
            bunyan_score += 1;
        }
    }
    // Bonus: numeric level + "v" field is a strong Bunyan indicator
    if obj.contains_key("v")
        && let Some(level_val) = obj.get("level")
        && level_val.is_number()
    {
        bunyan_score += 3;
    }

    let max_score = logstash_score.max(logrus_score).max(bunyan_score);

    if max_score == 0 {
        return Schema::Generic;
    }

    // In case of ties, prefer the more specific schema.
    // Logstash and Bunyan are more distinctive than Logrus.
    if logstash_score == max_score && logstash_score > logrus_score && logstash_score > bunyan_score
    {
        Schema::Logstash
    } else if bunyan_score == max_score && bunyan_score > logstash_score {
        Schema::Bunyan
    } else if logrus_score == max_score
        && logrus_score > logstash_score
        && logrus_score > bunyan_score
    {
        Schema::Logrus
    } else if logstash_score == max_score {
        Schema::Logstash
    } else if bunyan_score == max_score {
        Schema::Bunyan
    } else {
        Schema::Generic
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- Detection tests ---

    #[test]
    fn detect_logstash() {
        let value = json!({
            "@timestamp": "2024-01-15T10:30:00Z",
            "level": "INFO",
            "logger_name": "com.example.App",
            "message": "Application started",
            "thread_name": "main"
        });
        assert_eq!(detect_schema(&value), Schema::Logstash);
    }

    #[test]
    fn detect_logrus() {
        let value = json!({
            "level": "info",
            "msg": "server started",
            "time": "2024-01-15T10:30:00Z",
            "component": "http"
        });
        assert_eq!(detect_schema(&value), Schema::Logrus);
    }

    #[test]
    fn detect_bunyan() {
        let value = json!({
            "v": 0,
            "level": 30,
            "name": "myapp",
            "hostname": "server1",
            "pid": 1234,
            "time": "2024-01-15T10:30:00.000Z",
            "msg": "request completed"
        });
        assert_eq!(detect_schema(&value), Schema::Bunyan);
    }

    #[test]
    fn detect_bunyan_numeric_level_bonus() {
        // Even with fewer Bunyan fields, numeric level + v gives strong bonus
        let value = json!({
            "v": 0,
            "level": 30,
            "msg": "hello"
        });
        assert_eq!(detect_schema(&value), Schema::Bunyan);
    }

    #[test]
    fn detect_logstash_at_timestamp_bonus() {
        // @timestamp is a strong Logstash signal even with fewer total fields
        let value = json!({
            "@timestamp": "2024-01-15T10:30:00Z",
            "level": "INFO",
            "message": "test"
        });
        assert_eq!(detect_schema(&value), Schema::Logstash);
    }

    #[test]
    fn detect_generic_fallback_non_object() {
        let value = json!("just a string");
        assert_eq!(detect_schema(&value), Schema::Generic);
    }

    #[test]
    fn detect_generic_fallback_empty_object() {
        let value = json!({});
        assert_eq!(detect_schema(&value), Schema::Generic);
    }

    #[test]
    fn detect_generic_fallback_no_known_fields() {
        let value = json!({
            "custom_field": "value",
            "another_field": 42
        });
        assert_eq!(detect_schema(&value), Schema::Generic);
    }

    #[test]
    fn detect_ambiguous_falls_to_reasonable_choice() {
        // Has "level" and "msg" (shared between Logrus and Bunyan) but no other distinguishing fields
        let value = json!({
            "level": "info",
            "msg": "test"
        });
        // Both Logrus (2) and Bunyan (2) tie on score; tie-breaking favors Bunyan over Logrus
        let schema = detect_schema(&value);
        assert_eq!(schema, Schema::Bunyan);
    }

    // --- Forced schema selection tests ---

    #[test]
    fn from_choice_forced_logstash() {
        let value = json!({"msg": "test"});
        assert_eq!(
            Schema::from_choice(SchemaChoice::Logstash, &value),
            Schema::Logstash
        );
    }

    #[test]
    fn from_choice_forced_logrus() {
        let value = json!({"message": "test"});
        assert_eq!(
            Schema::from_choice(SchemaChoice::Logrus, &value),
            Schema::Logrus
        );
    }

    #[test]
    fn from_choice_forced_bunyan() {
        let value = json!({"message": "test"});
        assert_eq!(
            Schema::from_choice(SchemaChoice::Bunyan, &value),
            Schema::Bunyan
        );
    }

    #[test]
    fn from_choice_forced_generic() {
        let value = json!({"@timestamp": "2024-01-15T10:30:00Z", "level": "INFO"});
        assert_eq!(
            Schema::from_choice(SchemaChoice::Generic, &value),
            Schema::Generic
        );
    }

    #[test]
    fn from_choice_auto_delegates_to_detect() {
        let value = json!({
            "@timestamp": "2024-01-15T10:30:00Z",
            "level": "INFO",
            "logger_name": "test",
            "message": "hello"
        });
        assert_eq!(
            Schema::from_choice(SchemaChoice::Auto, &value),
            Schema::Logstash
        );
    }

    // --- FieldMapping tests ---

    #[test]
    fn logstash_mapping() {
        let mapping = Schema::Logstash.field_mapping();
        assert_eq!(mapping.level, vec!["level"]);
        assert_eq!(mapping.timestamp, vec!["@timestamp"]);
        assert_eq!(mapping.logger, vec!["logger_name"]);
        assert_eq!(mapping.message, vec!["message"]);
        assert_eq!(mapping.stack_trace, vec!["stack_trace"]);
    }

    #[test]
    fn logrus_mapping() {
        let mapping = Schema::Logrus.field_mapping();
        assert_eq!(mapping.level, vec!["level"]);
        assert_eq!(mapping.timestamp, vec!["time"]);
        assert_eq!(mapping.logger, vec!["component"]);
        assert_eq!(mapping.message, vec!["msg"]);
    }

    #[test]
    fn bunyan_mapping() {
        let mapping = Schema::Bunyan.field_mapping();
        assert_eq!(mapping.level, vec!["level"]);
        assert_eq!(mapping.timestamp, vec!["time"]);
        assert_eq!(mapping.logger, vec!["name"]);
        assert_eq!(mapping.message, vec!["msg"]);
    }

    #[test]
    fn generic_mapping_has_multiple_candidates() {
        let mapping = Schema::Generic.field_mapping();
        assert!(mapping.level.len() > 1);
        assert!(mapping.timestamp.len() > 1);
        assert!(mapping.logger.len() > 1);
        assert!(mapping.message.len() > 1);
        assert!(mapping.stack_trace.len() > 1);
    }

    #[test]
    fn generic_mapping_level_candidates() {
        let mapping = Schema::Generic.field_mapping();
        assert!(mapping.level.contains(&"level"));
        assert!(mapping.level.contains(&"severity"));
        assert!(mapping.level.contains(&"loglevel"));
    }

    #[test]
    fn generic_mapping_message_candidates() {
        let mapping = Schema::Generic.field_mapping();
        assert!(mapping.message.contains(&"message"));
        assert!(mapping.message.contains(&"msg"));
        assert!(mapping.message.contains(&"text"));
    }

    #[test]
    fn generic_mapping_timestamp_candidates() {
        let mapping = Schema::Generic.field_mapping();
        assert!(mapping.timestamp.contains(&"timestamp"));
        assert!(mapping.timestamp.contains(&"@timestamp"));
        assert!(mapping.timestamp.contains(&"time"));
        assert!(mapping.timestamp.contains(&"ts"));
    }

    // --- find_key tests ---

    #[test]
    fn find_key_returns_first_match() {
        let obj = json!({"msg": "hello", "message": "world"});
        let map = obj.as_object().unwrap();
        // message comes first in the candidate list for Generic
        let candidates = &["message", "msg"];
        assert_eq!(FieldMapping::find_key(candidates, map), Some("message"));
    }

    #[test]
    fn find_key_returns_none_when_no_match() {
        let obj = json!({"foo": "bar"});
        let map = obj.as_object().unwrap();
        assert_eq!(
            FieldMapping::find_key(&["message", "msg", "text"], map),
            None
        );
    }

    #[test]
    fn find_key_returns_second_candidate_when_first_absent() {
        let obj = json!({"msg": "hello"});
        let map = obj.as_object().unwrap();
        assert_eq!(
            FieldMapping::find_key(&["message", "msg"], map),
            Some("msg")
        );
    }

    // --- Detection edge cases ---

    #[test]
    fn detect_bunyan_without_v_no_bonus() {
        // Has Bunyan-like fields but without "v", no numeric bonus
        let value = json!({
            "level": 30,
            "name": "myapp",
            "msg": "hello",
            "time": "2024-01-15T10:30:00Z"
        });
        let schema = detect_schema(&value);
        // Without v, Bunyan doesn't get the +3 bonus, but still scores 4
        // (level, name, time, msg) vs Logrus 3 (level, msg, time). Bunyan wins.
        assert_eq!(schema, Schema::Bunyan);
    }

    #[test]
    fn detect_with_array_value() {
        let value = json!([1, 2, 3]);
        assert_eq!(detect_schema(&value), Schema::Generic);
    }

    #[test]
    fn detect_with_null_value() {
        let value = json!(null);
        assert_eq!(detect_schema(&value), Schema::Generic);
    }

    #[test]
    fn detect_logstash_with_version() {
        let value = json!({
            "@timestamp": "2024-01-15T10:30:00Z",
            "@version": "1",
            "level": "ERROR",
            "logger_name": "com.example",
            "message": "failed",
            "stack_trace": "java.lang.NullPointerException..."
        });
        assert_eq!(detect_schema(&value), Schema::Logstash);
    }

    #[test]
    fn detect_bunyan_with_all_fields() {
        let value = json!({
            "v": 0,
            "level": 50,
            "name": "myapp",
            "hostname": "prod-server",
            "pid": 9876,
            "time": "2024-01-15T10:30:00.000Z",
            "msg": "error occurred"
        });
        assert_eq!(detect_schema(&value), Schema::Bunyan);
    }
}

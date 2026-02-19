use serde_json::Value;

use crate::cli::NonJsonMode;
use crate::error::JlError;
use crate::format::sanitize_control_chars;

/// The result of parsing a single input line.
#[derive(Debug)]
pub enum ParseResult {
    /// The line was valid JSON.
    Json(Value),
    /// The line was not valid JSON and should be printed through (with sanitization).
    NonJson(String),
    /// The line was not valid JSON and should be skipped.
    Skip,
}

/// Parse a single input line, handling non-JSON lines according to `mode`.
///
/// - Valid JSON always returns `ParseResult::Json(value)`.
/// - Invalid JSON behavior depends on `mode`:
///   - `PrintAsIs` -> `ParseResult::NonJson(line)` (caller sanitizes before output)
///   - `Skip` -> `ParseResult::Skip`
///   - `Fail` -> `Err(JlError::Parse(...))`
pub fn parse_line(line: &str, mode: NonJsonMode) -> Result<ParseResult, JlError> {
    match serde_json::from_str::<Value>(line) {
        Ok(value) => Ok(ParseResult::Json(value)),
        Err(_) => match mode {
            NonJsonMode::PrintAsIs => Ok(ParseResult::NonJson(line.to_string())),
            NonJsonMode::Skip => Ok(ParseResult::Skip),
            NonJsonMode::Fail => Err(JlError::Parse(format!(
                "not valid JSON: {}",
                sanitize_control_chars(line)
            ))),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn valid_json_object() {
        let result = parse_line(
            r#"{"level":"INFO","message":"hello"}"#,
            NonJsonMode::PrintAsIs,
        )
        .unwrap();
        match result {
            ParseResult::Json(v) => {
                assert_eq!(v["level"], json!("INFO"));
                assert_eq!(v["message"], json!("hello"));
            }
            _ => panic!("expected Json variant"),
        }
    }

    #[test]
    fn valid_json_array() {
        let result = parse_line(r#"[1, 2, 3]"#, NonJsonMode::PrintAsIs).unwrap();
        match result {
            ParseResult::Json(v) => {
                assert!(v.is_array());
                assert_eq!(v.as_array().unwrap().len(), 3);
            }
            _ => panic!("expected Json variant"),
        }
    }

    #[test]
    fn valid_json_string() {
        let result = parse_line(r#""just a string""#, NonJsonMode::PrintAsIs).unwrap();
        match result {
            ParseResult::Json(v) => assert_eq!(v.as_str(), Some("just a string")),
            _ => panic!("expected Json variant"),
        }
    }

    #[test]
    fn valid_json_number() {
        let result = parse_line("42", NonJsonMode::PrintAsIs).unwrap();
        match result {
            ParseResult::Json(v) => assert_eq!(v.as_i64(), Some(42)),
            _ => panic!("expected Json variant"),
        }
    }

    #[test]
    fn valid_json_null() {
        let result = parse_line("null", NonJsonMode::PrintAsIs).unwrap();
        match result {
            ParseResult::Json(v) => assert!(v.is_null()),
            _ => panic!("expected Json variant"),
        }
    }

    #[test]
    fn valid_json_nested_object() {
        let result = parse_line(
            r#"{"@timestamp":"2024-01-15T10:30:00Z","level":"INFO","context":{"user":"alice","request_id":"abc123"}}"#,
            NonJsonMode::PrintAsIs,
        )
        .unwrap();
        match result {
            ParseResult::Json(v) => {
                assert!(v.is_object());
                assert_eq!(v["context"]["user"], json!("alice"));
            }
            _ => panic!("expected Json variant"),
        }
    }

    #[test]
    fn non_json_print_as_is() {
        let result = parse_line("this is not json", NonJsonMode::PrintAsIs).unwrap();
        match result {
            ParseResult::NonJson(s) => assert_eq!(s, "this is not json"),
            _ => panic!("expected NonJson variant"),
        }
    }

    #[test]
    fn non_json_skip() {
        let result = parse_line("this is not json", NonJsonMode::Skip).unwrap();
        assert!(matches!(result, ParseResult::Skip));
    }

    #[test]
    fn non_json_fail() {
        let result = parse_line("this is not json", NonJsonMode::Fail);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("not valid JSON"));
        assert!(msg.contains("this is not json"));
    }

    #[test]
    fn empty_line_print_as_is() {
        let result = parse_line("", NonJsonMode::PrintAsIs).unwrap();
        match result {
            ParseResult::NonJson(s) => assert_eq!(s, ""),
            _ => panic!("expected NonJson variant for empty line"),
        }
    }

    #[test]
    fn empty_line_skip() {
        let result = parse_line("", NonJsonMode::Skip).unwrap();
        assert!(matches!(result, ParseResult::Skip));
    }

    #[test]
    fn empty_line_fail() {
        let result = parse_line("", NonJsonMode::Fail);
        assert!(result.is_err());
    }

    #[test]
    fn partial_json_not_valid() {
        let result = parse_line(r#"{"level":"INFO""#, NonJsonMode::PrintAsIs).unwrap();
        match result {
            ParseResult::NonJson(s) => assert_eq!(s, r#"{"level":"INFO""#),
            _ => panic!("expected NonJson variant for partial JSON"),
        }
    }

    #[test]
    fn partial_json_skip() {
        let result = parse_line(r#"{"key":"#, NonJsonMode::Skip).unwrap();
        assert!(matches!(result, ParseResult::Skip));
    }

    #[test]
    fn partial_json_fail() {
        let result = parse_line(r#"{"key":"#, NonJsonMode::Fail);
        assert!(result.is_err());
    }

    #[test]
    fn whitespace_only_line() {
        let result = parse_line("   ", NonJsonMode::PrintAsIs).unwrap();
        match result {
            ParseResult::NonJson(s) => assert_eq!(s, "   "),
            _ => panic!("expected NonJson variant for whitespace-only line"),
        }
    }

    #[test]
    fn json_with_leading_whitespace() {
        // serde_json handles leading whitespace
        let result = parse_line(r#"  {"level":"DEBUG"}"#, NonJsonMode::PrintAsIs).unwrap();
        match result {
            ParseResult::Json(v) => assert_eq!(v["level"], json!("DEBUG")),
            _ => panic!("expected Json variant"),
        }
    }

    #[test]
    fn valid_json_all_modes_return_json() {
        let line = r#"{"msg":"test"}"#;
        for mode in [NonJsonMode::PrintAsIs, NonJsonMode::Skip, NonJsonMode::Fail] {
            let result = parse_line(line, mode).unwrap();
            assert!(
                matches!(result, ParseResult::Json(_)),
                "mode {mode:?} should return Json for valid JSON"
            );
        }
    }

    #[test]
    fn json_boolean_values() {
        let result = parse_line("true", NonJsonMode::PrintAsIs).unwrap();
        match result {
            ParseResult::Json(v) => assert_eq!(v.as_bool(), Some(true)),
            _ => panic!("expected Json variant"),
        }
    }
}

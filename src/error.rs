use std::fmt;

#[derive(Debug)]
pub enum JlError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Parse(String),
    Tz(String),
}

impl fmt::Display for JlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JlError::Io(e) => write!(f, "I/O error: {e}"),
            JlError::Json(e) => write!(f, "JSON error: {e}"),
            JlError::Parse(msg) => write!(f, "Parse error: {msg}"),
            JlError::Tz(msg) => write!(f, "Timezone error: {msg}"),
        }
    }
}

impl std::error::Error for JlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            JlError::Io(e) => Some(e),
            JlError::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for JlError {
    fn from(e: std::io::Error) -> Self {
        JlError::Io(e)
    }
}

impl From<serde_json::Error> for JlError {
    fn from(e: serde_json::Error) -> Self {
        JlError::Json(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = JlError::Io(io_err);
        let msg = format!("{err}");
        assert!(msg.contains("I/O error:"));
        assert!(msg.contains("file not found"));
    }

    #[test]
    fn display_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let err = JlError::Json(json_err);
        let msg = format!("{err}");
        assert!(msg.contains("JSON error:"));
    }

    #[test]
    fn display_parse_error() {
        let err = JlError::Parse("bad input".to_string());
        assert_eq!(format!("{err}"), "Parse error: bad input");
    }

    #[test]
    fn display_tz_error() {
        let err = JlError::Tz("unknown timezone".to_string());
        assert_eq!(format!("{err}"), "Timezone error: unknown timezone");
    }

    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err: JlError = io_err.into();
        assert!(matches!(err, JlError::Io(_)));
    }

    #[test]
    fn from_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("{bad}").unwrap_err();
        let err: JlError = json_err.into();
        assert!(matches!(err, JlError::Json(_)));
    }

    #[test]
    fn error_source_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err = JlError::Io(io_err);
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn error_source_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("x").unwrap_err();
        let err = JlError::Json(json_err);
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn error_source_parse_is_none() {
        let err = JlError::Parse("oops".to_string());
        assert!(std::error::Error::source(&err).is_none());
    }

    #[test]
    fn error_source_tz_is_none() {
        let err = JlError::Tz("bad tz".to_string());
        assert!(std::error::Error::source(&err).is_none());
    }
}

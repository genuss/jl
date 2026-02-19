use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl Level {
    pub fn from_bunyan_int(n: i64) -> Option<Level> {
        match n {
            10 => Some(Level::Trace),
            20 => Some(Level::Debug),
            30 => Some(Level::Info),
            40 => Some(Level::Warn),
            50 => Some(Level::Error),
            60 => Some(Level::Fatal),
            _ => None,
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Level::Trace => "TRACE",
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
            Level::Fatal => "FATAL",
        };
        write!(f, "{s}")
    }
}

impl FromStr for Level {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "TRACE" => Ok(Level::Trace),
            "DEBUG" => Ok(Level::Debug),
            "INFO" => Ok(Level::Info),
            "WARN" | "WARNING" => Ok(Level::Warn),
            "ERROR" => Ok(Level::Error),
            "FATAL" | "CRITICAL" | "PANIC" => Ok(Level::Fatal),
            _ => Err(format!("unknown log level: {s}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_uppercase() {
        assert_eq!("TRACE".parse::<Level>().unwrap(), Level::Trace);
        assert_eq!("DEBUG".parse::<Level>().unwrap(), Level::Debug);
        assert_eq!("INFO".parse::<Level>().unwrap(), Level::Info);
        assert_eq!("WARN".parse::<Level>().unwrap(), Level::Warn);
        assert_eq!("ERROR".parse::<Level>().unwrap(), Level::Error);
        assert_eq!("FATAL".parse::<Level>().unwrap(), Level::Fatal);
    }

    #[test]
    fn parse_lowercase() {
        assert_eq!("trace".parse::<Level>().unwrap(), Level::Trace);
        assert_eq!("debug".parse::<Level>().unwrap(), Level::Debug);
        assert_eq!("info".parse::<Level>().unwrap(), Level::Info);
        assert_eq!("warn".parse::<Level>().unwrap(), Level::Warn);
        assert_eq!("error".parse::<Level>().unwrap(), Level::Error);
        assert_eq!("fatal".parse::<Level>().unwrap(), Level::Fatal);
    }

    #[test]
    fn parse_mixed_case() {
        assert_eq!("Trace".parse::<Level>().unwrap(), Level::Trace);
        assert_eq!("Info".parse::<Level>().unwrap(), Level::Info);
        assert_eq!("WaRn".parse::<Level>().unwrap(), Level::Warn);
    }

    #[test]
    fn parse_aliases() {
        assert_eq!("WARNING".parse::<Level>().unwrap(), Level::Warn);
        assert_eq!("warning".parse::<Level>().unwrap(), Level::Warn);
        assert_eq!("CRITICAL".parse::<Level>().unwrap(), Level::Fatal);
        assert_eq!("PANIC".parse::<Level>().unwrap(), Level::Fatal);
    }

    #[test]
    fn parse_invalid() {
        assert!("unknown".parse::<Level>().is_err());
        assert!("".parse::<Level>().is_err());
        assert!("VERBOSE".parse::<Level>().is_err());
    }

    #[test]
    fn display() {
        assert_eq!(Level::Trace.to_string(), "TRACE");
        assert_eq!(Level::Debug.to_string(), "DEBUG");
        assert_eq!(Level::Info.to_string(), "INFO");
        assert_eq!(Level::Warn.to_string(), "WARN");
        assert_eq!(Level::Error.to_string(), "ERROR");
        assert_eq!(Level::Fatal.to_string(), "FATAL");
    }

    #[test]
    fn bunyan_int_valid() {
        assert_eq!(Level::from_bunyan_int(10), Some(Level::Trace));
        assert_eq!(Level::from_bunyan_int(20), Some(Level::Debug));
        assert_eq!(Level::from_bunyan_int(30), Some(Level::Info));
        assert_eq!(Level::from_bunyan_int(40), Some(Level::Warn));
        assert_eq!(Level::from_bunyan_int(50), Some(Level::Error));
        assert_eq!(Level::from_bunyan_int(60), Some(Level::Fatal));
    }

    #[test]
    fn bunyan_int_invalid() {
        assert_eq!(Level::from_bunyan_int(0), None);
        assert_eq!(Level::from_bunyan_int(15), None);
        assert_eq!(Level::from_bunyan_int(100), None);
        assert_eq!(Level::from_bunyan_int(-1), None);
    }

    #[test]
    fn ordering() {
        assert!(Level::Trace < Level::Debug);
        assert!(Level::Debug < Level::Info);
        assert!(Level::Info < Level::Warn);
        assert!(Level::Warn < Level::Error);
        assert!(Level::Error < Level::Fatal);
    }

    #[test]
    fn ordering_transitive() {
        assert!(Level::Trace < Level::Fatal);
        assert!(Level::Debug < Level::Error);
        assert!(Level::Info < Level::Fatal);
    }

    #[test]
    fn equality() {
        assert_eq!(Level::Info, Level::Info);
        assert_ne!(Level::Info, Level::Debug);
    }
}

use std::fs::File;
use std::io::{self, BufRead, BufReader, Stdin};
use std::path::Path;

use crate::error::JlError;

pub trait LineSource {
    fn next_line(&mut self) -> Result<Option<String>, JlError>;
}

pub struct StdinSource {
    reader: BufReader<Stdin>,
}

impl StdinSource {
    pub fn new() -> Self {
        Self {
            reader: BufReader::new(io::stdin()),
        }
    }
}

impl LineSource for StdinSource {
    fn next_line(&mut self) -> Result<Option<String>, JlError> {
        let mut line = String::new();
        let bytes_read = self.reader.read_line(&mut line)?;
        if bytes_read == 0 {
            Ok(None)
        } else {
            // Remove trailing newline
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
            }
            Ok(Some(line))
        }
    }
}

pub struct FileSource {
    reader: BufReader<File>,
}

impl FileSource {
    pub fn new(path: &Path) -> Result<Self, JlError> {
        let file = File::open(path)?;
        Ok(Self {
            reader: BufReader::new(file),
        })
    }
}

impl LineSource for FileSource {
    fn next_line(&mut self) -> Result<Option<String>, JlError> {
        let mut line = String::new();
        let bytes_read = self.reader.read_line(&mut line)?;
        if bytes_read == 0 {
            Ok(None)
        } else {
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
            }
            Ok(Some(line))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn file_source_reads_lines() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "line one").unwrap();
        writeln!(tmp, "line two").unwrap();
        writeln!(tmp, "line three").unwrap();
        tmp.flush().unwrap();

        let mut source = FileSource::new(tmp.path()).unwrap();
        assert_eq!(source.next_line().unwrap(), Some("line one".to_string()));
        assert_eq!(source.next_line().unwrap(), Some("line two".to_string()));
        assert_eq!(source.next_line().unwrap(), Some("line three".to_string()));
        assert_eq!(source.next_line().unwrap(), None);
    }

    #[test]
    fn file_source_empty_file() {
        let tmp = NamedTempFile::new().unwrap();
        let mut source = FileSource::new(tmp.path()).unwrap();
        assert_eq!(source.next_line().unwrap(), None);
    }

    #[test]
    fn file_source_no_trailing_newline() {
        let mut tmp = NamedTempFile::new().unwrap();
        write!(tmp, "no newline at end").unwrap();
        tmp.flush().unwrap();

        let mut source = FileSource::new(tmp.path()).unwrap();
        assert_eq!(
            source.next_line().unwrap(),
            Some("no newline at end".to_string())
        );
        assert_eq!(source.next_line().unwrap(), None);
    }

    #[test]
    fn file_source_crlf_lines() {
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(b"first\r\nsecond\r\n").unwrap();
        tmp.flush().unwrap();

        let mut source = FileSource::new(tmp.path()).unwrap();
        assert_eq!(source.next_line().unwrap(), Some("first".to_string()));
        assert_eq!(source.next_line().unwrap(), Some("second".to_string()));
        assert_eq!(source.next_line().unwrap(), None);
    }

    #[test]
    fn file_source_nonexistent_file() {
        let result = FileSource::new(Path::new("/nonexistent/path/file.txt"));
        assert!(result.is_err());
    }

    #[test]
    fn file_source_blank_lines() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "before").unwrap();
        writeln!(tmp).unwrap();
        writeln!(tmp, "after").unwrap();
        tmp.flush().unwrap();

        let mut source = FileSource::new(tmp.path()).unwrap();
        assert_eq!(source.next_line().unwrap(), Some("before".to_string()));
        assert_eq!(source.next_line().unwrap(), Some("".to_string()));
        assert_eq!(source.next_line().unwrap(), Some("after".to_string()));
        assert_eq!(source.next_line().unwrap(), None);
    }

    #[test]
    fn file_source_json_lines() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, r#"{{"level":"INFO","message":"hello"}}"#).unwrap();
        writeln!(tmp, r#"{{"level":"ERROR","message":"oops"}}"#).unwrap();
        tmp.flush().unwrap();

        let mut source = FileSource::new(tmp.path()).unwrap();
        let line1 = source.next_line().unwrap().unwrap();
        assert!(line1.contains("INFO"));
        let line2 = source.next_line().unwrap().unwrap();
        assert!(line2.contains("ERROR"));
        assert_eq!(source.next_line().unwrap(), None);
    }
}

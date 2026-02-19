use std::fs::File;
use std::io::{self, BufWriter, Stdout, Write};
use std::path::Path;

use crate::error::JlError;

pub trait OutputSink {
    fn write_line(&mut self, line: &str) -> Result<(), JlError>;
}

pub struct StdoutSink {
    writer: BufWriter<Stdout>,
}

impl Default for StdoutSink {
    fn default() -> Self {
        Self::new()
    }
}

impl StdoutSink {
    pub fn new() -> Self {
        Self {
            writer: BufWriter::new(io::stdout()),
        }
    }
}

impl OutputSink for StdoutSink {
    fn write_line(&mut self, line: &str) -> Result<(), JlError> {
        writeln!(self.writer, "{line}")?;
        self.writer.flush()?;
        Ok(())
    }
}

pub struct FileSink {
    writer: BufWriter<File>,
}

impl FileSink {
    pub fn new(path: &Path) -> Result<Self, JlError> {
        let file = File::create(path)?;
        Ok(Self {
            writer: BufWriter::new(file),
        })
    }
}

impl OutputSink for FileSink {
    fn write_line(&mut self, line: &str) -> Result<(), JlError> {
        writeln!(self.writer, "{line}")?;
        Ok(())
    }
}

impl Drop for FileSink {
    fn drop(&mut self) {
        let _ = self.writer.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn file_sink_writes_lines() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_owned();

        {
            let mut sink = FileSink::new(&path).unwrap();
            sink.write_line("hello").unwrap();
            sink.write_line("world").unwrap();
        }

        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "hello\nworld\n");
    }

    #[test]
    fn file_sink_empty_line() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_owned();

        {
            let mut sink = FileSink::new(&path).unwrap();
            sink.write_line("").unwrap();
        }

        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "\n");
    }

    #[test]
    fn file_sink_special_characters() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_owned();

        {
            let mut sink = FileSink::new(&path).unwrap();
            sink.write_line(r#"{"level":"INFO","msg":"hello"}"#)
                .unwrap();
            sink.write_line("line with\ttab").unwrap();
        }

        let contents = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("INFO"));
        assert!(lines[1].contains("\t"));
    }

    #[test]
    fn file_sink_invalid_path() {
        let result = FileSink::new(Path::new("/nonexistent/dir/file.txt"));
        assert!(result.is_err());
    }

    #[test]
    fn file_sink_multiple_flushes() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_owned();

        {
            let mut sink = FileSink::new(&path).unwrap();
            for i in 0..10 {
                sink.write_line(&format!("line {i}")).unwrap();
            }
        }

        let contents = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 10);
        assert_eq!(lines[0], "line 0");
        assert_eq!(lines[9], "line 9");
    }
}

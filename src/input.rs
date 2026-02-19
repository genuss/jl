use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Seek, SeekFrom, Stdin};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::error::JlError;

pub trait LineSource {
    fn next_line(&mut self) -> Result<Option<String>, JlError>;
}

pub struct StdinSource {
    reader: BufReader<Stdin>,
}

impl Default for StdinSource {
    fn default() -> Self {
        Self::new()
    }
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

/// A source that follows a file like `tail -f`, sleeping and retrying at EOF.
///
/// When EOF is reached, `FollowSource` sleeps briefly and re-reads the file
/// for new data. It never returns `None` (EOF) under normal operation; it
/// blocks until new lines appear or the caller otherwise terminates.
///
/// On unix systems, file rotation is detected by comparing inode numbers,
/// so a replaced file is always read from the beginning even if it is already
/// larger than the previous read position.
pub struct FollowSource {
    reader: BufReader<File>,
    path: PathBuf,
    #[cfg(unix)]
    inode: u64,
    /// Count of consecutive metadata failures after successful file opens.
    /// Used to detect persistent errors and avoid silent infinite stalls.
    metadata_failures: u32,
}

impl FollowSource {
    /// Create a new `FollowSource` for the given file path.
    pub fn new(path: &Path) -> Result<Self, JlError> {
        let file = File::open(path)?;
        #[cfg(unix)]
        let inode = {
            use std::os::unix::fs::MetadataExt;
            file.metadata()?.ino()
        };
        Ok(Self {
            reader: BufReader::new(file),
            path: path.to_path_buf(),
            #[cfg(unix)]
            inode,
            metadata_failures: 0,
        })
    }

    /// Create a new `FollowSource` starting from the end of the file.
    /// Only new lines appended after this point will be read.
    #[cfg(test)]
    pub fn new_from_end(path: &Path) -> Result<Self, JlError> {
        let file = File::open(path)?;
        #[cfg(unix)]
        let inode = {
            use std::os::unix::fs::MetadataExt;
            file.metadata()?.ino()
        };
        let mut reader = BufReader::new(file);
        reader.seek(SeekFrom::End(0))?;
        Ok(Self {
            reader,
            path: path.to_path_buf(),
            #[cfg(unix)]
            inode,
            metadata_failures: 0,
        })
    }

    /// Check whether the file at our path has been replaced (different inode)
    /// or truncated (shorter than our current position). Returns `true` if the
    /// file was rotated/replaced and we should start reading from the beginning.
    fn is_file_rotated(&self, new_meta: &fs::Metadata, current_pos: u64) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if new_meta.ino() != self.inode {
                return true;
            }
        }
        // Fallback for non-unix or same-inode: detect truncation by size
        new_meta.len() < current_pos
    }
}

impl LineSource for FollowSource {
    fn next_line(&mut self) -> Result<Option<String>, JlError> {
        let mut partial = String::new();
        loop {
            let mut line = String::new();
            let bytes_read = self.reader.read_line(&mut line)?;
            if bytes_read == 0 {
                // EOF reached - sleep and retry
                thread::sleep(Duration::from_millis(200));
                // Re-open to pick up new data if the file was replaced/rotated,
                // or just continue reading from the current position
                if let Ok(file) = File::open(&self.path) {
                    let current_pos = self.reader.stream_position()?;
                    if let Ok(new_meta) = file.metadata() {
                        self.metadata_failures = 0;
                        let mut new_reader = BufReader::new(file);
                        if self.is_file_rotated(&new_meta, current_pos) {
                            // File was truncated or replaced; start from beginning
                            new_reader.seek(SeekFrom::Start(0))?;
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::MetadataExt;
                                self.inode = new_meta.ino();
                            }
                            // Discard any partial line buffered from the old file
                            partial.clear();
                        } else {
                            new_reader.seek(SeekFrom::Start(current_pos))?;
                        }
                        self.reader = new_reader;
                    } else {
                        self.metadata_failures += 1;
                        if self.metadata_failures >= 10 {
                            // Persistent metadata failure; switch to the new file
                            // handle. Without metadata we cannot check inodes, but
                            // we can detect truncation by probing file length.
                            let path_display = crate::format::sanitize_control_chars(
                                &self.path.display().to_string(),
                            );
                            eprintln!("jl: warning: repeated metadata failures for {path_display}");
                            let mut new_reader = BufReader::new(file);
                            let file_len = new_reader.seek(SeekFrom::End(0))?;
                            if file_len < current_pos {
                                // File is shorter than our position – likely
                                // truncated or replaced; start from the beginning.
                                new_reader.seek(SeekFrom::Start(0))?;
                                // Discard any partial line buffered from the old file
                                partial.clear();
                            } else {
                                new_reader.seek(SeekFrom::Start(current_pos))?;
                            }
                            self.reader = new_reader;
                            self.metadata_failures = 0;
                        }
                    }
                }
                continue;
            }
            // Check if we got a complete line (ends with newline)
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
                if partial.is_empty() {
                    return Ok(Some(line));
                } else {
                    partial.push_str(&line);
                    return Ok(Some(std::mem::take(&mut partial)));
                }
            } else {
                // Partial line (no trailing newline) - buffer and continue
                // reading to get the rest before returning
                partial.push_str(&line);
            }
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

    // --- FollowSource tests ---

    #[test]
    fn follow_source_reads_existing_lines() {
        // Write some initial content and then append more from another thread.
        // The follow source should read both the initial and the appended lines.
        use std::fs::OpenOptions;
        use std::sync::mpsc;

        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_owned();

        // Write initial content
        {
            let mut f = OpenOptions::new().write(true).open(&path).unwrap();
            writeln!(f, "initial line").unwrap();
            f.flush().unwrap();
        }

        let (tx, rx) = mpsc::channel();
        let path_clone = path.clone();
        let writer = thread::spawn(move || {
            // Wait for signal that the reader has read the initial line
            rx.recv().unwrap();
            // Append a new line
            let mut f = OpenOptions::new().append(true).open(&path_clone).unwrap();
            writeln!(f, "appended line").unwrap();
            f.flush().unwrap();
        });

        let mut source = FollowSource::new(&path).unwrap();

        // Read the initial line
        let line1 = source.next_line().unwrap();
        assert_eq!(line1, Some("initial line".to_string()));

        // Signal the writer to append
        tx.send(()).unwrap();
        writer.join().unwrap();

        // The follow source should pick up the appended line
        let line2 = source.next_line().unwrap();
        assert_eq!(line2, Some("appended line".to_string()));
    }

    #[test]
    fn follow_source_new_from_end() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_owned();

        // Write initial content that should be skipped
        {
            let mut f = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
            writeln!(f, "old line 1").unwrap();
            writeln!(f, "old line 2").unwrap();
            f.flush().unwrap();
        }

        let mut source = FollowSource::new_from_end(&path).unwrap();

        // Append new content
        {
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            writeln!(f, "new line").unwrap();
            f.flush().unwrap();
        }

        // Should only read the new line, not the old ones
        let line = source.next_line().unwrap();
        assert_eq!(line, Some("new line".to_string()));
    }

    #[test]
    fn follow_source_nonexistent_file() {
        let result = FollowSource::new(Path::new("/nonexistent/path/file.txt"));
        assert!(result.is_err());
    }
}

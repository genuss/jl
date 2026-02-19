use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

fn jl() -> Command {
    Command::cargo_bin("jl").unwrap()
}

// --- Logstash JSON piped to stdin, verify output contains expected fields ---

#[test]
fn logstash_json_via_stdin() {
    let input = r#"{"@timestamp":"2024-01-15T10:30:00Z","level":"INFO","logger_name":"com.example","message":"hello world"}"#;
    jl().arg("--color")
        .arg("never")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("INFO"))
        .stdout(predicate::str::contains("com.example"))
        .stdout(predicate::str::contains("hello world"));
}

// --- Bunyan JSON with numeric level, verify correct level name ---

#[test]
fn bunyan_numeric_level_via_stdin() {
    let input =
        r#"{"level":30,"time":"2024-01-15T10:30:00Z","name":"myapp","msg":"started","v":0}"#;
    jl().arg("--color")
        .arg("never")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("INFO"))
        .stdout(predicate::str::contains("myapp"))
        .stdout(predicate::str::contains("started"));
}

#[test]
fn bunyan_error_level_via_stdin() {
    let input = r#"{"level":50,"time":"2024-01-15T10:30:00Z","name":"myapp","msg":"failed","v":0}"#;
    jl().arg("--color")
        .arg("never")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("ERROR"))
        .stdout(predicate::str::contains("failed"));
}

// --- Non-JSON line with --non-json skip: verify it is omitted ---

#[test]
fn non_json_skip_omits_plain_text() {
    let input = "plain text line\n{\"@timestamp\":\"2024-01-15T10:30:00Z\",\"level\":\"INFO\",\"logger_name\":\"app\",\"message\":\"json line\"}\nanother plain line\n";
    jl().arg("--color")
        .arg("never")
        .arg("--non-json")
        .arg("skip")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("plain text line").not())
        .stdout(predicate::str::contains("json line"))
        .stdout(predicate::str::contains("another plain line").not());
}

// --- Non-JSON line with --non-json print-as-is: verify it passes through ---

#[test]
fn non_json_print_as_is_passes_through() {
    let input = "plain text line\n{\"@timestamp\":\"2024-01-15T10:30:00Z\",\"level\":\"INFO\",\"logger_name\":\"app\",\"message\":\"json line\"}\n";
    jl().arg("--color")
        .arg("never")
        .arg("--non-json")
        .arg("print-as-is")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("plain text line"))
        .stdout(predicate::str::contains("json line"));
}

// --- --min-level WARN: verify INFO lines are filtered out ---

#[test]
fn min_level_filters_info_lines() {
    let input = concat!(
        r#"{"@timestamp":"2024-01-15T10:30:00Z","level":"DEBUG","logger_name":"app","message":"debug msg"}"#,
        "\n",
        r#"{"@timestamp":"2024-01-15T10:30:01Z","level":"INFO","logger_name":"app","message":"info msg"}"#,
        "\n",
        r#"{"@timestamp":"2024-01-15T10:30:02Z","level":"WARN","logger_name":"app","message":"warn msg"}"#,
        "\n",
        r#"{"@timestamp":"2024-01-15T10:30:03Z","level":"ERROR","logger_name":"app","message":"error msg"}"#,
        "\n",
    );
    jl().arg("--color")
        .arg("never")
        .arg("--min-level")
        .arg("WARN")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("debug msg").not())
        .stdout(predicate::str::contains("info msg").not())
        .stdout(predicate::str::contains("warn msg"))
        .stdout(predicate::str::contains("error msg"));
}

// --- --schema logrus to force schema, verify correct field extraction ---

#[test]
fn forced_logrus_schema() {
    let input = r#"{"level":"info","msg":"logrus message","time":"2024-01-15T10:30:00Z","component":"web"}"#;
    jl().arg("--color")
        .arg("never")
        .arg("--schema")
        .arg("logrus")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("INFO"))
        .stdout(predicate::str::contains("logrus message"))
        .stdout(predicate::str::contains("[web]"));
}

// --- --color never: verify no ANSI codes in output ---

#[test]
fn color_never_no_ansi_codes() {
    let input = r#"{"@timestamp":"2024-01-15T10:30:00Z","level":"ERROR","logger_name":"app","message":"error msg"}"#;
    let output = jl()
        .arg("--color")
        .arg("never")
        .write_stdin(input)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();
    // ANSI escape sequences start with \x1b[
    assert!(
        !stdout.contains("\x1b["),
        "Output should not contain ANSI codes with --color never, but got: {stdout}"
    );
    assert!(stdout.contains("ERROR"));
}

// --- --format with custom template ---

#[test]
fn custom_format_template() {
    let input = r#"{"@timestamp":"2024-01-15T10:30:00Z","level":"INFO","logger_name":"com.example","message":"hello"}"#;
    jl().arg("--color")
        .arg("never")
        .arg("--format")
        .arg("[{level}] {message}")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("[INFO] hello"));
}

#[test]
fn custom_format_with_custom_field() {
    let input = r#"{"@timestamp":"2024-01-15T10:30:00Z","level":"INFO","logger_name":"app","message":"hello","host":"server1"}"#;
    jl().arg("--color")
        .arg("never")
        .arg("--format")
        .arg("{level} [{host}] {message}")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("INFO [server1] hello"));
}

// --- File input: write temp file, pass as positional arg ---

#[test]
fn file_input_positional_arg() {
    let mut tmp = NamedTempFile::new().unwrap();
    writeln!(
        tmp,
        r#"{{"@timestamp":"2024-01-15T10:30:00Z","level":"INFO","logger_name":"app","message":"from file"}}"#
    )
    .unwrap();
    tmp.flush().unwrap();

    jl().arg("--color")
        .arg("never")
        .arg(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("from file"));
}

#[test]
fn multiple_file_inputs() {
    let mut tmp1 = NamedTempFile::new().unwrap();
    writeln!(
        tmp1,
        r#"{{"@timestamp":"2024-01-15T10:30:00Z","level":"INFO","logger_name":"app","message":"file one"}}"#
    )
    .unwrap();
    tmp1.flush().unwrap();

    let mut tmp2 = NamedTempFile::new().unwrap();
    writeln!(
        tmp2,
        r#"{{"@timestamp":"2024-01-15T10:31:00Z","level":"WARN","logger_name":"app","message":"file two"}}"#
    )
    .unwrap();
    tmp2.flush().unwrap();

    let output = jl()
        .arg("--color")
        .arg("never")
        .arg(tmp1.path())
        .arg(tmp2.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("file one"));
    assert!(stdout.contains("file two"));
    // file one should appear before file two
    let pos1 = stdout.find("file one").unwrap();
    let pos2 = stdout.find("file two").unwrap();
    assert!(pos1 < pos2);
}

// --- -o output file option ---

#[test]
fn output_file_option() {
    let input = r#"{"@timestamp":"2024-01-15T10:30:00Z","level":"INFO","logger_name":"app","message":"to file"}"#;
    let output_file = NamedTempFile::new().unwrap();
    let output_path = output_file.path().to_owned();

    jl().arg("--color")
        .arg("never")
        .arg("-o")
        .arg(&output_path)
        .write_stdin(input)
        .assert()
        .success();

    let contents = std::fs::read_to_string(&output_path).unwrap();
    assert!(contents.contains("to file"));
    assert!(contents.contains("INFO"));
}

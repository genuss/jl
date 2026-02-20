use owo_colors::{OwoColorize, Style};

use crate::cli::{CliColor, ColorMode};
use crate::level::Level;

/// Configuration for colorized output.
#[derive(Debug, Clone)]
pub struct ColorConfig {
    pub enabled: bool,
    key_style: Style,
    value_style: Style,
}

/// Convert a CLI color choice to an owo-colors `Style`.
fn cli_color_to_style(c: CliColor) -> Style {
    match c {
        CliColor::Black => Style::new().black(),
        CliColor::Red => Style::new().red(),
        CliColor::Green => Style::new().green(),
        CliColor::Yellow => Style::new().yellow(),
        CliColor::Blue => Style::new().blue(),
        CliColor::Magenta => Style::new().magenta(),
        CliColor::Cyan => Style::new().cyan(),
        CliColor::White => Style::new().white(),
    }
}

impl ColorConfig {
    /// Create a new `ColorConfig` from the CLI color mode and extra-field color choices.
    ///
    /// For `Auto`, color is enabled only if stdout is a terminal.
    /// For `Always`, color is always on. For `Never`, always off.
    pub fn new(mode: ColorMode, key_color: CliColor, value_color: CliColor) -> Self {
        let enabled = match mode {
            ColorMode::Auto => is_terminal::is_terminal(std::io::stdout()),
            ColorMode::Always => true,
            ColorMode::Never => false,
        };
        ColorConfig {
            enabled,
            key_style: cli_color_to_style(key_color),
            value_style: cli_color_to_style(value_color),
        }
    }

    /// Create a `ColorConfig` with color explicitly enabled or disabled.
    /// Uses default extra-field colors (magenta key, cyan value). Useful for testing.
    #[cfg(test)]
    pub fn with_enabled(enabled: bool) -> Self {
        ColorConfig {
            enabled,
            key_style: cli_color_to_style(CliColor::Magenta),
            value_style: cli_color_to_style(CliColor::Cyan),
        }
    }

    /// Return the `owo-colors` style for the given log level.
    pub fn level_style(&self, level: &Level) -> Style {
        if !self.enabled {
            return Style::new();
        }
        match level {
            Level::Trace => Style::new().dimmed(),
            Level::Debug => Style::new().blue(),
            Level::Info => Style::new().green(),
            Level::Warn => Style::new().yellow(),
            Level::Error => Style::new().red(),
            Level::Fatal => Style::new().red().bold(),
        }
    }

    /// Apply the level style to the given text.
    pub fn style_level(&self, level: &Level) -> String {
        let text = level.to_string();
        if !self.enabled {
            return text;
        }
        let style = self.level_style(level);
        format!("{}", text.style(style))
    }

    /// Style an extra field key using the configured key color.
    pub fn style_extra_key(&self, key: &str) -> String {
        if !self.enabled {
            return key.to_string();
        }
        format!("{}", key.style(self.key_style))
    }

    /// Style an extra field value using the configured value color.
    pub fn style_extra_value(&self, val: &str) -> String {
        if !self.enabled {
            return val.to_string();
        }
        format!("{}", val.style(self.value_style))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_never_is_disabled() {
        let config = ColorConfig::new(ColorMode::Never, CliColor::Blue, CliColor::Green);
        assert!(!config.enabled);
    }

    #[test]
    fn color_always_is_enabled() {
        let config = ColorConfig::new(ColorMode::Always, CliColor::Blue, CliColor::Green);
        assert!(config.enabled);
    }

    #[test]
    fn with_enabled_true() {
        let config = ColorConfig::with_enabled(true);
        assert!(config.enabled);
    }

    #[test]
    fn with_enabled_false() {
        let config = ColorConfig::with_enabled(false);
        assert!(!config.enabled);
    }

    #[test]
    fn style_level_no_color_returns_plain_text() {
        let config = ColorConfig::with_enabled(false);
        assert_eq!(config.style_level(&Level::Info), "INFO");
        assert_eq!(config.style_level(&Level::Error), "ERROR");
        assert_eq!(config.style_level(&Level::Trace), "TRACE");
        assert_eq!(config.style_level(&Level::Debug), "DEBUG");
        assert_eq!(config.style_level(&Level::Warn), "WARN");
        assert_eq!(config.style_level(&Level::Fatal), "FATAL");
    }

    #[test]
    fn style_level_with_color_contains_ansi_codes() {
        let config = ColorConfig::with_enabled(true);
        let styled = config.style_level(&Level::Info);
        // When color is enabled, the output should contain ANSI escape codes
        assert!(styled.contains("\x1b["));
        assert!(styled.contains("INFO"));
    }

    #[test]
    fn style_level_with_color_different_levels_differ() {
        let config = ColorConfig::with_enabled(true);
        let info = config.style_level(&Level::Info);
        let error = config.style_level(&Level::Error);
        let warn = config.style_level(&Level::Warn);
        // Different levels should produce different styled strings
        assert_ne!(info, error);
        assert_ne!(info, warn);
        assert_ne!(error, warn);
    }

    #[test]
    fn level_style_disabled_returns_default_style() {
        let config = ColorConfig::with_enabled(false);
        // When disabled, all levels should return the same (empty) style
        let trace_style = config.level_style(&Level::Trace);
        let fatal_style = config.level_style(&Level::Fatal);
        // Verify by applying to the same text
        let trace_text = format!("{}", "test".style(trace_style));
        let fatal_text = format!("{}", "test".style(fatal_style));
        assert_eq!(trace_text, fatal_text);
        assert_eq!(trace_text, "test");
    }

    #[test]
    fn level_style_enabled_trace_is_dimmed() {
        let config = ColorConfig::with_enabled(true);
        let styled = config.style_level(&Level::Trace);
        // Dimmed text uses ANSI code \x1b[2m
        assert!(styled.contains("\x1b[2m"));
    }

    #[test]
    fn level_style_enabled_fatal_is_bold_red() {
        let config = ColorConfig::with_enabled(true);
        let styled = config.style_level(&Level::Fatal);
        assert!(styled.contains("FATAL"));
        // owo_colors may combine bold+red as \x1b[1;31m or emit them separately
        let has_bold = styled.contains("\x1b[1m") || styled.contains(";1m") || styled.contains("[1;");
        let has_red = styled.contains("\x1b[31m") || styled.contains(";31m") || styled.contains("[31;");
        assert!(has_bold, "FATAL should be bold, got: {styled:?}");
        assert!(has_red, "FATAL should be red, got: {styled:?}");
    }

    #[test]
    fn style_extra_key_no_color_returns_plain() {
        let config = ColorConfig::with_enabled(false);
        assert_eq!(config.style_extra_key("host"), "host");
    }

    #[test]
    fn style_extra_key_with_color_contains_ansi_magenta() {
        let config = ColorConfig::with_enabled(true);
        let styled = config.style_extra_key("host");
        // Magenta uses ANSI code \x1b[35m
        assert!(styled.contains("\x1b[35m"));
        assert!(styled.contains("host"));
    }

    #[test]
    fn style_extra_value_no_color_returns_plain() {
        let config = ColorConfig::with_enabled(false);
        assert_eq!(config.style_extra_value("server1"), "server1");
    }

    #[test]
    fn style_extra_value_with_color_contains_ansi_cyan() {
        let config = ColorConfig::with_enabled(true);
        let styled = config.style_extra_value("server1");
        // Cyan uses ANSI code \x1b[36m
        assert!(styled.contains("\x1b[36m"));
        assert!(styled.contains("server1"));
    }

    #[test]
    fn style_extra_key_custom_color() {
        let config = ColorConfig::new(ColorMode::Always, CliColor::Cyan, CliColor::Green);
        let styled = config.style_extra_key("host");
        // Cyan uses ANSI code \x1b[36m
        assert!(styled.contains("\x1b[36m"));
        assert!(styled.contains("host"));
    }

    #[test]
    fn style_extra_value_custom_color() {
        let config = ColorConfig::new(ColorMode::Always, CliColor::Blue, CliColor::Yellow);
        let styled = config.style_extra_value("server1");
        // Yellow uses ANSI code \x1b[33m
        assert!(styled.contains("\x1b[33m"));
        assert!(styled.contains("server1"));
    }
}

use owo_colors::{OwoColorize, Style};

use crate::cli::ColorMode;
use crate::level::Level;

/// Configuration for colorized output.
#[derive(Debug, Clone)]
pub struct ColorConfig {
    pub enabled: bool,
}

impl ColorConfig {
    /// Create a new `ColorConfig` from the CLI color mode.
    ///
    /// For `Auto`, color is enabled only if stdout is a terminal.
    /// For `Always`, color is always on. For `Never`, always off.
    pub fn new(mode: ColorMode) -> Self {
        let enabled = match mode {
            ColorMode::Auto => is_terminal::is_terminal(std::io::stdout()),
            ColorMode::Always => true,
            ColorMode::Never => false,
        };
        ColorConfig { enabled }
    }

    /// Create a `ColorConfig` with color explicitly enabled or disabled.
    /// Useful for testing.
    #[cfg(test)]
    pub fn with_enabled(enabled: bool) -> Self {
        ColorConfig { enabled }
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

    /// Style a separator character (e.g., `=` or `:`) used in extra fields output.
    ///
    /// Returns the separator with dimmed styling when color is enabled,
    /// or plain text when color is disabled.
    pub fn style_separator(&self, sep: &str) -> String {
        if !self.enabled {
            return sep.to_string();
        }
        format!("{}", sep.style(Style::new().dimmed()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_never_is_disabled() {
        let config = ColorConfig::new(ColorMode::Never);
        assert!(!config.enabled);
    }

    #[test]
    fn color_always_is_enabled() {
        let config = ColorConfig::new(ColorMode::Always);
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
    fn style_separator_no_color_returns_plain() {
        let config = ColorConfig::with_enabled(false);
        assert_eq!(config.style_separator("="), "=");
        assert_eq!(config.style_separator(":"), ":");
    }

    #[test]
    fn style_separator_with_color_contains_ansi_dimmed() {
        let config = ColorConfig::with_enabled(true);
        let styled_eq = config.style_separator("=");
        // Dimmed uses ANSI code \x1b[2m
        assert!(styled_eq.contains("\x1b[2m"));
        assert!(styled_eq.contains("="));

        let styled_colon = config.style_separator(":");
        assert!(styled_colon.contains("\x1b[2m"));
        assert!(styled_colon.contains(":"));
    }
}

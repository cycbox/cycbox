use super::Color;

/// Severity of a `MESSAGE_TYPE_LOG` message. Drives the prefix and colour the
/// log line is rendered with (see [`MessageBuilder::log`](super::MessageBuilder::log)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Debug,
}

impl LogLevel {
    pub fn color(&self) -> Color {
        match self {
            LogLevel::Info => Color::Primary,
            LogLevel::Warning => Color::Tertiary,
            LogLevel::Error => Color::Error,
            LogLevel::Debug => Color::OnSurfaceVariant,
        }
    }

    pub fn prefix(&self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Debug => "DEBUG",
        }
    }
}

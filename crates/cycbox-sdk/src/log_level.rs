use crate::Color;

/// Severity of a log line, shared by the engine's own logging and by transports that
/// report through a [`TransportSink`](crate::transport::TransportSink).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Debug,
}

impl LogLevel {
    /// Color the UI renders a line of this level in.
    pub fn color(&self) -> Color {
        match self {
            LogLevel::Info => Color::Primary,
            LogLevel::Warning => Color::Tertiary,
            LogLevel::Error => Color::Error,
            LogLevel::Debug => Color::OnSurfaceVariant,
        }
    }

    /// Prefix the engine writes in front of a formatted log line.
    pub fn prefix(&self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Debug => "DEBUG",
        }
    }

    /// Lowercase name, as carried in the `level` value of a sink `LOG` message and as
    /// accepted by Lua's `log()`.
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Info => "info",
            LogLevel::Warning => "warn",
            LogLevel::Error => "error",
            LogLevel::Debug => "debug",
        }
    }

    /// Inverse of [`Self::as_str`], case-insensitive; also accepts `warning`.
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "info" => Some(LogLevel::Info),
            "warn" | "warning" => Some(LogLevel::Warning),
            "error" => Some(LogLevel::Error),
            "debug" => Some(LogLevel::Debug),
            _ => None,
        }
    }
}

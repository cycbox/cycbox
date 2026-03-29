use std::{io, result};

pub type Result<T> = result::Result<T, CycBoxError>;

/// Errors produced by codec encode/decode operations.
#[derive(Debug, thiserror::Error)]
pub enum CycBoxError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Connection Error will cause the connection task to attempt reconnection.
    #[error("Connection failed: {0}")]
    Connection(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Pending error: {0}")]
    Pending(String),

    #[error("Invalid value for '{field}': {reason}")]
    InvalidValue { field: String, reason: String },

    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    #[error("Missing required field: '{0}'")]
    MissingField(String),

    #[error("Unsupported: {0}")]
    Unsupported(String),

    #[error("Lua error: {0}")]
    Lua(#[from] mlua::Error),

    #[error("Lua function error: {0}")]
    LuaFunction(String),

    #[error("{0}")]
    Other(String),
}

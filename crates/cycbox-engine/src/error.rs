use tokio::sync::{mpsc, oneshot};

#[derive(Debug, Clone, thiserror::Error)]
pub enum EngineError {
    #[error("Failed to send command to engine: {0}")]
    CommandSend(String),

    #[error("Failed to receive response from engine: {0}")]
    ResponseReceive(#[from] oneshot::error::RecvError),

    #[error("Engine error: {0}")]
    Engine(String),
}

impl<T> From<mpsc::error::SendError<T>> for EngineError {
    fn from(e: mpsc::error::SendError<T>) -> Self {
        EngineError::CommandSend(e.to_string())
    }
}

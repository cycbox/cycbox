use crate::error::CycBoxError;
use crate::{Configurable, Manifestable, Message};
use async_trait::async_trait;

#[async_trait]
pub trait Transformer: Configurable + Manifestable + Send + Sync {
    fn on_receive(&self, message: &mut Message) -> Result<(), CycBoxError>;
    fn on_send(&self, _message: &mut Message) -> Result<(), CycBoxError> {
        Ok(())
    }
}

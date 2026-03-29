use crate::lua::LuaFunctionRegistry;
use crate::message_input::MessageInputRegistry;
use crate::{Codec, CycBoxError, FormGroup, Manifestable, MessageTransport, Transformer};
use async_trait::async_trait;
use std::time::Duration;

#[async_trait]
pub trait RunMode: Manifestable + Send + Sync {
    async fn create_transport(
        &self,
        id: &str,
        configs: &[FormGroup],
        codec: Box<dyn Codec>,
        timeout: Duration,
    ) -> Result<Box<dyn MessageTransport>, CycBoxError>;
    async fn create_transformer(
        &self,
        id: &str,
        configs: &[FormGroup],
    ) -> Result<Option<Box<dyn Transformer>>, CycBoxError>;
    async fn create_codec(
        &self,
        id: &str,
        configs: &[FormGroup],
    ) -> Result<Box<dyn Codec>, CycBoxError>;

    /// Return the message input registry for this run mode.
    ///
    /// The registry is used by MCP/CLI callers to convert raw JSON message
    /// inputs into [`Message`]s without going through the Dart UI path.
    fn message_input_registry(&self) -> &MessageInputRegistry;

    /// Return the Lua helper registry for this run mode.
    ///
    /// The registry provides protocol-specific Lua functions (e.g., `mqtt_publish`)
    /// that are registered into the Lua state when the script engine starts.
    fn lua_helper_registry(&self) -> &LuaFunctionRegistry;
}

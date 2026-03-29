use crate::Message;
use async_trait::async_trait;
use std::sync::Arc;

/// Engine capabilities exposed to Lua function implementations.
#[async_trait]
pub trait LuaEngine: Send + Sync {
    /// Queue a message to be sent via connections (fire-and-forget)
    async fn send_message(&self, message: Message);

    fn debug(&self, message: &str);
    fn info(&self, message: &str);
    fn warn(&self, message: &str);
    fn error(&self, message: &str);
}

/// Trait for registering Lua functions into a Lua state.
///
/// Transport and codec crates implement this to provide protocol-specific
/// Lua globals (e.g., `mqtt_publish`, `modbus_read`). Implementations are
/// collected by `LuaFunctionRegistry` and registered when the Lua script starts.
pub trait LuaFunctionRegistrar: Send + Sync {
    /// Unique identifier for this function set (e.g., "mqtt", "modbus_rtu")
    fn function_id(&self) -> &str;

    /// Register Lua functions into the given Lua state.
    fn register(
        &self,
        lua: &mlua::Lua,
        engine: Arc<dyn LuaEngine>,
    ) -> Result<(), crate::CycBoxError>;
}

/// Registry of Lua function registrars.
///
/// Collected by `RunMode` implementations and used by the script engine
/// to register all protocol-specific Lua functions at startup.
pub struct LuaFunctionRegistry {
    registrars: Vec<Box<dyn LuaFunctionRegistrar>>,
}

impl LuaFunctionRegistry {
    pub fn new() -> Self {
        Self {
            registrars: Vec::new(),
        }
    }

    pub fn register(&mut self, registrar: Box<dyn LuaFunctionRegistrar>) {
        self.registrars.push(registrar);
    }

    /// Register all functions into the Lua state.
    /// Returns a list of `(function_id, error)` for any registrations that failed.
    /// Failures are non-fatal — other helpers are still registered.
    pub fn register_all(
        &self,
        lua: &mlua::Lua,
        engine: Arc<dyn LuaEngine>,
    ) -> Vec<(String, crate::CycBoxError)> {
        let mut errors = Vec::new();
        for registrar in &self.registrars {
            if let Err(e) = registrar.register(lua, engine.clone()) {
                errors.push((registrar.function_id().to_string(), e));
            }
        }
        errors
    }
}

impl Default for LuaFunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

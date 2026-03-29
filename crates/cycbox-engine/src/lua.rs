use crate::engine::EngineRef;
use cycbox_sdk::lua::{LuaEngine, LuaFunctionRegistry};
use cycbox_sdk::{CycBoxError, FormGroup, FormUtils};
use cycbox_sdk::{Message, MessageBuilder, PayloadType};
use mlua::{AnyUserData, Function, Lua};
use std::sync::Arc;

/// Default example script shown in UI
pub const DEFAULT_LUA_SCRIPT: &str = r#"
-- CycBox Lua Script
-- Documentation: https://cycbox.io/docs/lua-api/

-- Available hooks (uncomment to use):

-- function on_start()
--   -- Called once when engine starts
--   log("info", "Engine started")
-- end

-- function on_timer(timestamp_ms)
--   -- Called every 100ms with current timestamp in milliseconds
-- end

-- function on_receive()
--   -- Called for each received message
--   -- Access message fields: message.payload, message.connection_id
--   -- Return true if modified, false otherwise
--   return false
-- end

-- function on_send()
--   -- Called for each outgoing message (before encoding)
--   -- Modify message fields if needed
--   return false
-- end

-- function on_send_confirm()
--   -- Called after message is successfully sent
--   return false
-- end

-- function on_stop()
--   -- Called before engine stops or script is reloaded
-- end

"#;

/// Main Lua scripting engine
pub struct LuaScript {
    has_on_start_hook: bool,
    has_on_send_hook: bool,
    has_on_receive_hook: bool,
    has_on_timer_hook: bool,
    has_on_send_confirm_hook: bool,
    has_on_stop_hook: bool,
    lua: Lua,
    engine: EngineRef,
    // Store transport and codec info for all connections
    connection_transports: Vec<String>,
    connection_codecs: Vec<String>,
}

impl LuaScript {
    pub fn new(
        lua_code: &str,
        configs: &[&[FormGroup]],
        engine: EngineRef,
        lua_helper_registry: &LuaFunctionRegistry,
    ) -> Result<Self, CycBoxError> {
        let mut connection_transports = vec![];
        let mut connection_codecs = vec![];
        for config in configs {
            let transport = FormUtils::get_text_value(config, "app", "app_transport")
                .unwrap_or("")
                .to_string();
            let codec = FormUtils::get_text_value(config, "app", "app_codec")
                .unwrap_or("")
                .to_string();

            connection_transports.push(transport);
            connection_codecs.push(codec);
        }
        let mut lua_script = LuaScript {
            has_on_start_hook: false,
            has_on_send_hook: false,
            has_on_receive_hook: false,
            has_on_timer_hook: false,
            has_on_send_confirm_hook: false,
            has_on_stop_hook: false,
            lua: Lua::new(),
            engine: engine.clone(),
            connection_transports,
            connection_codecs,
        };
        lua_script.setup_utility_functions()?;
        lua_script.lua.load(lua_code).exec()?;
        // Detect hooks
        let globals = lua_script.lua.globals();
        lua_script.has_on_start_hook = globals
            .get::<Option<Function>>("on_start")
            .ok()
            .flatten()
            .is_some();
        lua_script.has_on_send_hook = globals
            .get::<Option<Function>>("on_send")
            .ok()
            .flatten()
            .is_some();
        lua_script.has_on_receive_hook = globals
            .get::<Option<Function>>("on_receive")
            .ok()
            .flatten()
            .is_some();
        lua_script.has_on_timer_hook = globals
            .get::<Option<Function>>("on_timer")
            .ok()
            .flatten()
            .is_some();
        lua_script.has_on_send_confirm_hook = globals
            .get::<Option<Function>>("on_send_confirm")
            .ok()
            .flatten()
            .is_some();
        lua_script.has_on_stop_hook = globals
            .get::<Option<Function>>("on_stop")
            .ok()
            .flatten()
            .is_some();
        if lua_script.has_on_start_hook
            || lua_script.has_on_send_hook
            || lua_script.has_on_receive_hook
            || lua_script.has_on_timer_hook
            || lua_script.has_on_send_confirm_hook
        {
            engine.info(&format!(
                "Lua script hooks enabled: start={}, send={}, receive={}, timer={}, send_confirm={}, stop={}",
                lua_script.has_on_start_hook,
                lua_script.has_on_send_hook,
                lua_script.has_on_receive_hook,
                lua_script.has_on_timer_hook,
                lua_script.has_on_send_confirm_hook,
                lua_script.has_on_stop_hook,
            ));

            // Register helpers from the registry (protocol-specific helpers like MQTT)
            let engine_ref: Arc<dyn LuaEngine> = Arc::new(lua_script.engine.clone());
            for (id, err) in lua_helper_registry.register_all(&lua_script.lua, engine_ref) {
                engine.warn(&format!(
                    "[Lua] Failed to register {} functions: {}",
                    id, err
                ));
            }
        }
        Ok(lua_script)
    }

    /// Common helper to execute Lua message hooks
    /// Returns Ok(()) if hook executed successfully (regardless of whether message was modified)
    async fn call_message_hook(
        &self,
        hook_name: &str,
        message: &mut Message,
    ) -> Result<(), CycBoxError> {
        let globals = &self.lua.globals();

        // Set message to global
        globals.set("message", message.clone())?;
        let hook_fn: Function = globals.get(hook_name)?;
        let modified: bool = hook_fn.call_async(()).await?;

        // Only copy back the message if it was modified
        if modified {
            let message_userdata: AnyUserData = globals.get("message")?;
            let modified_message = message_userdata.borrow::<Message>()?.clone();

            // Update the original message with modifications
            *message = modified_message;
        }

        Ok(())
    }

    /// Process message using Lua on_receive hook (RX only)
    /// Sets message as global, calls on_receive(), retrieves modified message only if hook returns true
    pub async fn on_receive(&mut self, message: &mut Message) -> Result<(), CycBoxError> {
        if !self.has_on_receive_hook {
            return Ok(());
        }
        self.call_message_hook("on_receive", message).await
    }

    /// Process message using Lua on_send hook (TX only)
    /// Sets message as global, calls on_send(), retrieves modified message only if hook returns true
    pub async fn on_send(&mut self, message: &mut Message) -> Result<(), CycBoxError> {
        if !self.has_on_send_hook {
            return Ok(());
        }
        self.call_message_hook("on_send", message).await
    }

    /// Process TX confirmation message using Lua on_send_confirm hook
    /// Sets message as global, calls on_send_confirm(), retrieves modified message only if hook returns true
    pub async fn on_send_confirm(&mut self, message: &mut Message) -> Result<(), CycBoxError> {
        if !self.has_on_send_confirm_hook {
            return Ok(());
        }
        self.call_message_hook("on_send_confirm", message).await
    }

    /// Process timer tick using Lua on_timer hook
    /// Called periodically (typically every 100ms)
    ///
    /// # Arguments
    /// * `timestamp_ms` - Current timestamp in milliseconds
    pub async fn on_timer(&mut self, timestamp_ms: u64) -> Result<(), CycBoxError> {
        if !self.has_on_timer_hook {
            return Ok(());
        }
        let globals = &self.lua.globals();

        // Get the on_timer function
        let on_timer_fn: Function = globals.get("on_timer")?;

        on_timer_fn.call_async::<()>(timestamp_ms).await?;

        Ok(())
    }

    /// Process engine start using Lua on_start hook
    /// Called once when the processor task starts
    pub async fn on_start(&mut self) -> Result<(), CycBoxError> {
        if !self.has_on_start_hook {
            return Ok(());
        }

        let globals = &self.lua.globals();
        let on_start_fn: Function = globals.get("on_start")?;
        on_start_fn.call_async::<()>(()).await?;
        Ok(())
    }

    /// Called before the engine stops or before the script is hot-reloaded
    pub async fn on_stop(&mut self) -> Result<(), CycBoxError> {
        if !self.has_on_stop_hook {
            return Ok(());
        }

        let globals = &self.lua.globals();
        let on_stop_fn: Function = globals.get("on_stop")?;
        on_stop_fn.call_async::<()>(()).await?;
        Ok(())
    }

    fn setup_utility_functions(&self) -> Result<(), CycBoxError> {
        let lua = &self.lua;
        let engine = self.engine.clone();
        let globals = lua.globals();
        // Add log function: log(level, message)
        // Accepts nil values gracefully - converts to "<nil>" string
        let log_fn = lua.create_function(
            move |_, (level, message): (Option<String>, Option<String>)| {
                let level_str = level.unwrap_or_else(|| "info".to_string());
                let message_str = message.unwrap_or_else(|| "<nil>".to_string());
                let message_str = format!("[Lua] {}", message_str);
                match level_str.as_str() {
                    "debug" => engine.debug(&message_str),
                    "info" => engine.info(&message_str),
                    "warn" => engine.warn(&message_str),
                    "error" => engine.error(&message_str),
                    _ => engine.info(&message_str), // default to info level
                }
                Ok(())
            },
        )?;
        globals.set("log", log_fn)?;

        // Add connection query APIs
        let connection_transports = self.connection_transports.clone();
        let get_transport_fn = lua.create_function(move |_, connection_id: u32| {
            let transport = connection_transports
                .get(connection_id as usize)
                .cloned()
                .unwrap_or_default();
            Ok(transport)
        })?;
        globals.set("get_transport", get_transport_fn)?;

        let connection_codecs = self.connection_codecs.clone();
        let get_codec_fn = lua.create_function(move |_, connection_id: u32| {
            let codec = connection_codecs
                .get(connection_id as usize)
                .cloned()
                .unwrap_or_default();
            Ok(codec)
        })?;
        globals.set("get_codec", get_codec_fn)?;

        let connection_count = self.connection_transports.len();
        let get_connection_count_fn = lua.create_function(move |_, ()| Ok(connection_count))?;
        globals.set("get_connection_count", get_connection_count_fn)?;

        // Add get_env function: get_env(var_name)
        // Returns environment variable value or nil if not found
        let get_env_fn = lua.create_function(|_, var_name: Option<String>| {
            let var_name_str = var_name.ok_or_else(|| {
                mlua::Error::RuntimeError("get_env: variable name cannot be nil".to_string())
            })?;

            match std::env::var(&var_name_str) {
                Ok(value) => Ok(Some(value)),
                Err(_) => Ok(None),
            }
        })?;
        globals.set("get_env", get_env_fn)?;

        // Add async send_after function: send_after(payload, delay_ms, connection_id)
        // Returns error if payload is nil
        let engine = self.engine.clone();
        let send_after_fn =
            lua.create_async_function(
                move |_lua,
                      (payload, delay_ms, connection_id): (
                    Option<mlua::String>,
                    u64,
                    Option<u32>,
                )| {
                    let engine = engine.clone();
                    async move {
                        // Validate payload is not nil
                        let payload_str = payload.ok_or_else(|| {
                            mlua::Error::RuntimeError(
                                "send_after: payload cannot be nil".to_string(),
                            )
                        })?;

                        // Convert payload string to bytes (handles arbitrary binary data)
                        let payload_bytes = payload_str.as_bytes().to_vec();
                        let connection_id = connection_id.unwrap_or(0);
                        let mut timestamp = Message::current_timestamp();
                        if delay_ms > 0 {
                            timestamp += delay_ms * 1000; // Convert ms to µs
                        }
                        let builder = MessageBuilder::tx(
                            connection_id,
                            PayloadType::Binary,
                            payload_bytes,
                            Vec::new(),
                        )
                        .timestamp(timestamp);

                        // Send to delay queue
                        engine.send_message(builder.build()).await;

                        Ok(true)
                    }
                },
            )?;

        globals.set("send_after", send_after_fn)?;

        Ok(())
    }
}

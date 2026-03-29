use cycbox_sdk::prelude::*;
use std::sync::Arc;

mod discord;
mod http;
mod influxdb;
mod ntfy;
mod redis;
mod smtp;
mod timescaledb;

pub struct RuntimeLuaFunctionRegistrar;

impl LuaFunctionRegistrar for RuntimeLuaFunctionRegistrar {
    fn function_id(&self) -> &str {
        "runtime"
    }

    fn register(&self, lua: &mlua::Lua, _engine: Arc<dyn LuaEngine>) -> Result<(), CycBoxError> {
        http::register_http_helpers(lua).map_err(|e| CycBoxError::Other(e.to_string()))?;
        discord::register_discord_helpers(lua).map_err(|e| CycBoxError::Other(e.to_string()))?;
        influxdb::register_influxdb_helpers(lua).map_err(|e| CycBoxError::Other(e.to_string()))?;
        ntfy::register_ntfy_helpers(lua).map_err(|e| CycBoxError::Other(e.to_string()))?;
        redis::register_redis_helpers(lua).map_err(|e| CycBoxError::Other(e.to_string()))?;
        smtp::register_smtp_helpers(lua).map_err(|e| CycBoxError::Other(e.to_string()))?;
        timescaledb::register_timescaledb_helpers(lua)
            .map_err(|e| CycBoxError::Other(e.to_string()))?;
        Ok(())
    }
}

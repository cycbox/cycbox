use crate::CycBoxError;
use crate::lua::{LuaEngine, LuaFunctionRegistrar};
use std::sync::Arc;

pub struct MessageLuaHelper;

impl LuaFunctionRegistrar for MessageLuaHelper {
    fn function_id(&self) -> &str {
        "message"
    }

    fn register(&self, lua: &mlua::Lua, _engine: Arc<dyn LuaEngine>) -> Result<(), CycBoxError> {
        let globals = lua.globals();

        // Binary reading helper functions (offset is 1-indexed in Lua)

        // read_u8(bytes, offset) -> integer
        let read_u8_fn = lua
            .create_function(|_, (bytes, offset): (mlua::String, usize)| {
                let data = bytes.as_bytes();
                if offset < 1 || offset > data.len() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "read_u8: offset {offset} out of range, valid range is 1-{}, data length is {} bytes",
                        data.len(),
                        data.len()
                    )));
                }
                Ok(data[offset - 1] as i64)
            })
            .map_err(|e| CycBoxError::Other(format!("Failed to create read_u8: {e}")))?;
        globals
            .set("read_u8", read_u8_fn)
            .map_err(|e| CycBoxError::Other(format!("Failed to set read_u8: {e}")))?;

        // read_i8(bytes, offset) -> integer
        let read_i8_fn = lua
            .create_function(|_, (bytes, offset): (mlua::String, usize)| {
                let data = bytes.as_bytes();
                if offset < 1 || offset > data.len() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "read_i8: offset {offset} out of range, valid range is 1-{}, data length is {} bytes",
                        data.len(),
                        data.len()
                    )));
                }
                Ok(data[offset - 1] as i8 as i64)
            })
            .map_err(|e| CycBoxError::Other(format!("Failed to create read_i8: {e}")))?;
        globals
            .set("read_i8", read_i8_fn)
            .map_err(|e| CycBoxError::Other(format!("Failed to set read_i8: {e}")))?;

        // read_u16_be(bytes, offset) -> integer
        let read_u16_be_fn = lua
            .create_function(|_, (bytes, offset): (mlua::String, usize)| {
                let data = bytes.as_bytes();
                if offset < 1 || offset + 1 > data.len() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "read_u16_be: offset {offset} out of range, needs 2 bytes at offset (valid range 1-{}), data length is {} bytes",
                        data.len().saturating_sub(1),
                        data.len()
                    )));
                }
                let idx = offset - 1;
                let value = u16::from_be_bytes([data[idx], data[idx + 1]]);
                Ok(value as i64)
            })
            .map_err(|e| CycBoxError::Other(format!("Failed to create read_u16_be: {e}")))?;
        globals
            .set("read_u16_be", read_u16_be_fn)
            .map_err(|e| CycBoxError::Other(format!("Failed to set read_u16_be: {e}")))?;

        // read_u16_le(bytes, offset) -> integer
        let read_u16_le_fn = lua
            .create_function(|_, (bytes, offset): (mlua::String, usize)| {
                let data = bytes.as_bytes();
                if offset < 1 || offset + 1 > data.len() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "read_u16_le: offset {offset} out of range, needs 2 bytes at offset (valid range 1-{}), data length is {} bytes",
                        data.len().saturating_sub(1),
                        data.len()
                    )));
                }
                let idx = offset - 1;
                let value = u16::from_le_bytes([data[idx], data[idx + 1]]);
                Ok(value as i64)
            })
            .map_err(|e| CycBoxError::Other(format!("Failed to create read_u16_le: {e}")))?;
        globals
            .set("read_u16_le", read_u16_le_fn)
            .map_err(|e| CycBoxError::Other(format!("Failed to set read_u16_le: {e}")))?;

        // read_i16_be(bytes, offset) -> integer
        let read_i16_be_fn = lua
            .create_function(|_, (bytes, offset): (mlua::String, usize)| {
                let data = bytes.as_bytes();
                if offset < 1 || offset + 1 > data.len() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "read_i16_be: offset {offset} out of range, needs 2 bytes at offset (valid range 1-{}), data length is {} bytes",
                        data.len().saturating_sub(1),
                        data.len()
                    )));
                }
                let idx = offset - 1;
                let value = i16::from_be_bytes([data[idx], data[idx + 1]]);
                Ok(value as i64)
            })
            .map_err(|e| CycBoxError::Other(format!("Failed to create read_i16_be: {e}")))?;
        globals
            .set("read_i16_be", read_i16_be_fn)
            .map_err(|e| CycBoxError::Other(format!("Failed to set read_i16_be: {e}")))?;

        // read_i16_le(bytes, offset) -> integer
        let read_i16_le_fn = lua
            .create_function(|_, (bytes, offset): (mlua::String, usize)| {
                let data = bytes.as_bytes();
                if offset < 1 || offset + 1 > data.len() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "read_i16_le: offset {offset} out of range, needs 2 bytes at offset (valid range 1-{}), data length is {} bytes",
                        data.len().saturating_sub(1),
                        data.len()
                    )));
                }
                let idx = offset - 1;
                let value = i16::from_le_bytes([data[idx], data[idx + 1]]);
                Ok(value as i64)
            })
            .map_err(|e| CycBoxError::Other(format!("Failed to create read_i16_le: {e}")))?;
        globals
            .set("read_i16_le", read_i16_le_fn)
            .map_err(|e| CycBoxError::Other(format!("Failed to set read_i16_le: {e}")))?;

        // read_u32_be(bytes, offset) -> integer
        let read_u32_be_fn = lua
            .create_function(|_, (bytes, offset): (mlua::String, usize)| {
                let data = bytes.as_bytes();
                if offset < 1 || offset + 3 > data.len() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "read_u32_be: offset {offset} out of range, needs 4 bytes at offset (valid range 1-{}), data length is {} bytes",
                        data.len().saturating_sub(3),
                        data.len()
                    )));
                }
                let idx = offset - 1;
                let value =
                    u32::from_be_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]);
                Ok(value as i64)
            })
            .map_err(|e| CycBoxError::Other(format!("Failed to create read_u32_be: {e}")))?;
        globals
            .set("read_u32_be", read_u32_be_fn)
            .map_err(|e| CycBoxError::Other(format!("Failed to set read_u32_be: {e}")))?;

        // read_u32_le(bytes, offset) -> integer
        let read_u32_le_fn = lua
            .create_function(|_, (bytes, offset): (mlua::String, usize)| {
                let data = bytes.as_bytes();
                if offset < 1 || offset + 3 > data.len() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "read_u32_le: offset {offset} out of range, needs 4 bytes at offset (valid range 1-{}), data length is {} bytes",
                        data.len().saturating_sub(3),
                        data.len()
                    )));
                }
                let idx = offset - 1;
                let value =
                    u32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]);
                Ok(value as i64)
            })
            .map_err(|e| CycBoxError::Other(format!("Failed to create read_u32_le: {e}")))?;
        globals
            .set("read_u32_le", read_u32_le_fn)
            .map_err(|e| CycBoxError::Other(format!("Failed to set read_u32_le: {e}")))?;

        // read_i32_be(bytes, offset) -> integer
        let read_i32_be_fn = lua
            .create_function(|_, (bytes, offset): (mlua::String, usize)| {
                let data = bytes.as_bytes();
                if offset < 1 || offset + 3 > data.len() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "read_i32_be: offset {offset} out of range, needs 4 bytes at offset (valid range 1-{}), data length is {} bytes",
                        data.len().saturating_sub(3),
                        data.len()
                    )));
                }
                let idx = offset - 1;
                let value =
                    i32::from_be_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]);
                Ok(value as i64)
            })
            .map_err(|e| CycBoxError::Other(format!("Failed to create read_i32_be: {e}")))?;
        globals
            .set("read_i32_be", read_i32_be_fn)
            .map_err(|e| CycBoxError::Other(format!("Failed to set read_i32_be: {e}")))?;

        // read_i32_le(bytes, offset) -> integer
        let read_i32_le_fn = lua
            .create_function(|_, (bytes, offset): (mlua::String, usize)| {
                let data = bytes.as_bytes();
                if offset < 1 || offset + 3 > data.len() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "read_i32_le: offset {offset} out of range, needs 4 bytes at offset (valid range 1-{}), data length is {} bytes",
                        data.len().saturating_sub(3),
                        data.len()
                    )));
                }
                let idx = offset - 1;
                let value =
                    i32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]);
                Ok(value as i64)
            })
            .map_err(|e| CycBoxError::Other(format!("Failed to create read_i32_le: {e}")))?;
        globals
            .set("read_i32_le", read_i32_le_fn)
            .map_err(|e| CycBoxError::Other(format!("Failed to set read_i32_le: {e}")))?;

        // read_float_be(bytes, offset) -> number (f32)
        let read_float_be_fn = lua
            .create_function(|_, (bytes, offset): (mlua::String, usize)| {
                let data = bytes.as_bytes();
                if offset < 1 || offset + 3 > data.len() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "read_float_be: offset {offset} out of range, needs 4 bytes at offset (valid range 1-{}), data length is {} bytes",
                        data.len().saturating_sub(3),
                        data.len()
                    )));
                }
                let idx = offset - 1;
                let value =
                    f32::from_be_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]);
                Ok(value as f64)
            })
            .map_err(|e| CycBoxError::Other(format!("Failed to create read_float_be: {e}")))?;
        globals
            .set("read_float_be", read_float_be_fn)
            .map_err(|e| CycBoxError::Other(format!("Failed to set read_float_be: {e}")))?;

        // read_float_le(bytes, offset) -> number (f32)
        let read_float_le_fn = lua
            .create_function(|_, (bytes, offset): (mlua::String, usize)| {
                let data = bytes.as_bytes();
                if offset < 1 || offset + 3 > data.len() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "read_float_le: offset {offset} out of range, needs 4 bytes at offset (valid range 1-{}), data length is {} bytes",
                        data.len().saturating_sub(3),
                        data.len()
                    )));
                }
                let idx = offset - 1;
                let value =
                    f32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]);
                Ok(value as f64)
            })
            .map_err(|e| CycBoxError::Other(format!("Failed to create read_float_le: {e}")))?;
        globals
            .set("read_float_le", read_float_le_fn)
            .map_err(|e| CycBoxError::Other(format!("Failed to set read_float_le: {e}")))?;

        // read_double_be(bytes, offset) -> number (f64)
        let read_double_be_fn = lua
            .create_function(|_, (bytes, offset): (mlua::String, usize)| {
                let data = bytes.as_bytes();
                if offset < 1 || offset + 7 > data.len() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "read_double_be: offset {offset} out of range, needs 8 bytes at offset (valid range 1-{}), data length is {} bytes",
                        data.len().saturating_sub(7),
                        data.len()
                    )));
                }
                let idx = offset - 1;
                let value = f64::from_be_bytes([
                    data[idx],
                    data[idx + 1],
                    data[idx + 2],
                    data[idx + 3],
                    data[idx + 4],
                    data[idx + 5],
                    data[idx + 6],
                    data[idx + 7],
                ]);
                Ok(value)
            })
            .map_err(|e| CycBoxError::Other(format!("Failed to create read_double_be: {e}")))?;
        globals
            .set("read_double_be", read_double_be_fn)
            .map_err(|e| CycBoxError::Other(format!("Failed to set read_double_be: {e}")))?;

        // read_double_le(bytes, offset) -> number (f64)
        let read_double_le_fn = lua
            .create_function(|_, (bytes, offset): (mlua::String, usize)| {
                let data = bytes.as_bytes();
                if offset < 1 || offset + 7 > data.len() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "read_double_le: offset {offset} out of range, needs 8 bytes at offset (valid range 1-{}), data length is {} bytes",
                        data.len().saturating_sub(7),
                        data.len()
                    )));
                }
                let idx = offset - 1;
                let value = f64::from_le_bytes([
                    data[idx],
                    data[idx + 1],
                    data[idx + 2],
                    data[idx + 3],
                    data[idx + 4],
                    data[idx + 5],
                    data[idx + 6],
                    data[idx + 7],
                ]);
                Ok(value)
            })
            .map_err(|e| CycBoxError::Other(format!("Failed to create read_double_le: {e}")))?;
        globals
            .set("read_double_le", read_double_le_fn)
            .map_err(|e| CycBoxError::Other(format!("Failed to set read_double_le: {e}")))?;

        Ok(())
    }
}

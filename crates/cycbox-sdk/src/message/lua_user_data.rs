// ============================================================================
// Lua integration for Message struct, exposing fields and methods to Lua scripts.
// ============================================================================

use super::Message;
use super::Value;
use super::ValueType;

use mlua::{Integer as LuaInteger, UserData, UserDataFields, UserDataMethods, Value as LuaValue};

/// Return a short human-readable description of a LuaValue for error messages.
fn lua_value_desc(val: &LuaValue) -> String {
    match val {
        LuaValue::Nil => "nil".to_string(),
        LuaValue::Boolean(b) => format!("boolean ({b})"),
        LuaValue::Integer(n) => format!("integer ({n})"),
        LuaValue::Number(n) => format!("number ({n})"),
        LuaValue::String(s) => match s.to_str() {
            Ok(preview) if preview.len() > 32 => format!("string (\"{}...\")", &preview[..32]),
            Ok(preview) => format!("string (\"{preview}\")"),
            Err(_) => "string (<non-utf8>)".to_string(),
        },
        LuaValue::Table(_) => "table".to_string(),
        LuaValue::Function(_) => "function".to_string(),
        LuaValue::UserData(_) => "userdata".to_string(),
        _ => "unknown type".to_string(),
    }
}

/// Extract a numeric value as i64 from a LuaValue, accepting both Integer and Number.
#[allow(clippy::unnecessary_cast)]
fn lua_value_to_i64(val: &LuaValue) -> Option<i64> {
    match val {
        LuaValue::Integer(n) => Some(*n as i64),
        LuaValue::Number(n) => Some(*n as i64),
        _ => None,
    }
}

/// Extract a numeric value as f64 from a LuaValue, accepting both Integer and Number.
fn lua_value_to_f64(val: &LuaValue) -> Option<f64> {
    match val {
        LuaValue::Number(n) => Some(*n),
        LuaValue::Integer(n) => Some(*n as f64),
        _ => None,
    }
}

/// Extract a numeric value as u64 from a LuaValue, accepting both Integer and Number.
fn lua_value_to_u64(val: &LuaValue) -> Option<u64> {
    match val {
        LuaValue::Integer(n) => {
            if *n >= 0 {
                Some(*n as u64)
            } else {
                None
            }
        }
        LuaValue::Number(n) => {
            if *n >= 0.0 {
                Some(*n as u64)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Extract id string from the first argument, with descriptive error messages.
fn extract_id(method: &str, val: LuaValue) -> Result<String, mlua::Error> {
    val.as_string()
        .and_then(|s| s.to_str().ok())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            mlua::Error::RuntimeError(format!(
                "{method}: 'id' (arg #1) must be a string, got {}",
                lua_value_desc(&val)
            ))
        })
}

/// Extract optional timestamp from an argument, with descriptive error on negative values.
fn extract_timestamp(
    method: &str,
    val: Option<LuaValue>,
    default: u64,
) -> Result<u64, mlua::Error> {
    match val {
        None => Ok(default),
        Some(v) => lua_value_to_u64(&v).ok_or_else(|| {
            mlua::Error::RuntimeError(format!(
                "{method}: 'timestamp' (arg #3) must be a non-negative number, got {}",
                lua_value_desc(&v)
            ))
        }),
    }
}

impl UserData for Message {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        // Payload field
        fields.add_field_method_get("payload", |lua, this| lua.create_string(&this.payload));

        fields.add_field_method_set("payload", |_, this, bytes: Option<mlua::String>| {
            this.payload = bytes.map(|s| s.as_bytes().to_vec()).unwrap_or_default();
            Ok(())
        });

        // Frame field
        fields.add_field_method_get("frame", |lua, this| lua.create_string(&this.frame));

        fields.add_field_method_set("frame", |_, this, bytes: Option<mlua::String>| {
            this.frame = bytes.map(|s| s.as_bytes().to_vec()).unwrap_or_default();
            Ok(())
        });

        // Timestamp field (read-only)
        fields.add_field_method_get("timestamp", |_, this| Ok(this.timestamp));

        // Checksum valid field (derived from metadata)
        fields.add_field_method_get("checksum_valid", |_, this| {
            let valid = this
                .metadata
                .iter()
                .find(|v| v.id == "checksum_valid")
                .and_then(|v| v.value_payload.first().copied())
                .map(|b| b != 0)
                .unwrap_or(true);
            Ok(valid)
        });

        // Connection ID field
        fields.add_field_method_get("connection_id", |_, this| Ok(this.connection_id));

        fields.add_field_method_set("connection_id", |_, this, connection_id: u32| {
            this.connection_id = connection_id;
            Ok(())
        });

        // values_json field - converts values to JSON in real-time
        fields.add_field_method_get("values_json", |_, this| {
            let mut map = serde_json::Map::new();

            for val in &this.values {
                let json_value = match val.value_type {
                    // Boolean
                    ValueType::Boolean => {
                        let b = val.value_payload.first().map(|&b| b != 0).unwrap_or(false);
                        serde_json::Value::Bool(b)
                    }

                    // Signed integers
                    ValueType::Int8 => {
                        if !val.value_payload.is_empty() {
                            serde_json::Value::Number((val.value_payload[0] as i8).into())
                        } else {
                            serde_json::Value::Null
                        }
                    }
                    ValueType::Int16 => {
                        if val.value_payload.len() >= 2 {
                            let bytes: [u8; 2] = val.value_payload[..2].try_into().unwrap();
                            serde_json::Value::Number(i16::from_le_bytes(bytes).into())
                        } else {
                            serde_json::Value::Null
                        }
                    }
                    ValueType::Int32 => {
                        if val.value_payload.len() >= 4 {
                            let bytes: [u8; 4] = val.value_payload[..4].try_into().unwrap();
                            serde_json::Value::Number(i32::from_le_bytes(bytes).into())
                        } else {
                            serde_json::Value::Null
                        }
                    }
                    ValueType::Int64 => {
                        if val.value_payload.len() >= 8 {
                            let bytes: [u8; 8] = val.value_payload[..8].try_into().unwrap();
                            serde_json::Value::Number(i64::from_le_bytes(bytes).into())
                        } else {
                            serde_json::Value::Null
                        }
                    }

                    // Unsigned integers
                    ValueType::UInt8 => {
                        if !val.value_payload.is_empty() {
                            serde_json::Value::Number(val.value_payload[0].into())
                        } else {
                            serde_json::Value::Null
                        }
                    }
                    ValueType::UInt16 => {
                        if val.value_payload.len() >= 2 {
                            let bytes: [u8; 2] = val.value_payload[..2].try_into().unwrap();
                            serde_json::Value::Number(u16::from_le_bytes(bytes).into())
                        } else {
                            serde_json::Value::Null
                        }
                    }
                    ValueType::UInt32 => {
                        if val.value_payload.len() >= 4 {
                            let bytes: [u8; 4] = val.value_payload[..4].try_into().unwrap();
                            serde_json::Value::Number(u32::from_le_bytes(bytes).into())
                        } else {
                            serde_json::Value::Null
                        }
                    }
                    ValueType::UInt64 => {
                        if val.value_payload.len() >= 8 {
                            let bytes: [u8; 8] = val.value_payload[..8].try_into().unwrap();
                            serde_json::Value::Number(u64::from_le_bytes(bytes).into())
                        } else {
                            serde_json::Value::Null
                        }
                    }

                    // Floating point
                    ValueType::Float32 => {
                        if val.value_payload.len() >= 4 {
                            let bytes: [u8; 4] = val.value_payload[..4].try_into().unwrap();
                            let f = f32::from_le_bytes(bytes);
                            if let Some(n) = serde_json::Number::from_f64(f as f64) {
                                serde_json::Value::Number(n)
                            } else {
                                serde_json::Value::Null
                            }
                        } else {
                            serde_json::Value::Null
                        }
                    }
                    ValueType::Float64 => {
                        if val.value_payload.len() >= 8 {
                            let bytes: [u8; 8] = val.value_payload[..8].try_into().unwrap();
                            let f = f64::from_le_bytes(bytes);
                            if let Some(n) = serde_json::Number::from_f64(f) {
                                serde_json::Value::Number(n)
                            } else {
                                serde_json::Value::Null
                            }
                        } else {
                            serde_json::Value::Null
                        }
                    }

                    // String
                    ValueType::String => {
                        if let Ok(s) = String::from_utf8(val.value_payload.clone()) {
                            serde_json::Value::String(s)
                        } else {
                            serde_json::Value::Null
                        }
                    }

                    // Arrays not yet supported
                    _ => serde_json::Value::Null,
                };

                map.insert(val.id.clone(), json_value);
            }

            let json_str = serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string());

            Ok(json_str)
        });
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // ---- Complex operations remain as methods ----

        // ---- Value manipulation ----
        methods.add_method_mut("add_int_value", |_, this, args: mlua::MultiValue| {
            const METHOD: &str = "add_int_value";
            let mut args_iter = args.into_iter();
            let id = extract_id(
                METHOD,
                args_iter.next().ok_or_else(|| {
                    mlua::Error::RuntimeError(format!(
                        "{METHOD}: missing 'id' (arg #1), usage: msg:{METHOD}(id, value [, timestamp])"
                    ))
                })?,
            )?;
            let value_arg = args_iter.next().ok_or_else(|| {
                mlua::Error::RuntimeError(format!(
                    "{METHOD}: missing 'value' (arg #2), usage: msg:{METHOD}(id, value [, timestamp])"
                ))
            })?;
            let value: i64 = lua_value_to_i64(&value_arg).ok_or_else(|| {
                mlua::Error::RuntimeError(format!(
                    "{METHOD}: 'value' (arg #2) must be a number (integer or float), got {}",
                    lua_value_desc(&value_arg)
                ))
            })?;
            let timestamp = extract_timestamp(METHOD, args_iter.next(), this.timestamp)?;

            let val = Value {
                id,
                timestamp,
                value_type: ValueType::Int64,
                value_payload: value.to_le_bytes().to_vec(),
            };
            this.values.push(val);
            Ok(())
        });

        methods.add_method_mut("add_float_value", |_, this, args: mlua::MultiValue| {
            const METHOD: &str = "add_float_value";
            let mut args_iter = args.into_iter();
            let id = extract_id(
                METHOD,
                args_iter.next().ok_or_else(|| {
                    mlua::Error::RuntimeError(format!(
                        "{METHOD}: missing 'id' (arg #1), usage: msg:{METHOD}(id, value [, timestamp])"
                    ))
                })?,
            )?;
            let value_arg = args_iter.next().ok_or_else(|| {
                mlua::Error::RuntimeError(format!(
                    "{METHOD}: missing 'value' (arg #2), usage: msg:{METHOD}(id, value [, timestamp])"
                ))
            })?;
            let value: f64 = lua_value_to_f64(&value_arg).ok_or_else(|| {
                mlua::Error::RuntimeError(format!(
                    "{METHOD}: 'value' (arg #2) must be a number (integer or float), got {}",
                    lua_value_desc(&value_arg)
                ))
            })?;
            let timestamp = extract_timestamp(METHOD, args_iter.next(), this.timestamp)?;

            let val = Value {
                id,
                timestamp,
                value_type: ValueType::Float64,
                value_payload: value.to_le_bytes().to_vec(),
            };
            this.values.push(val);
            Ok(())
        });

        methods.add_method_mut("add_string_value", |_, this, args: mlua::MultiValue| {
            const METHOD: &str = "add_string_value";
            let mut args_iter = args.into_iter();
            let id = extract_id(
                METHOD,
                args_iter.next().ok_or_else(|| {
                    mlua::Error::RuntimeError(format!(
                        "{METHOD}: missing 'id' (arg #1), usage: msg:{METHOD}(id, value [, timestamp])"
                    ))
                })?,
            )?;
            let value_arg = args_iter.next().ok_or_else(|| {
                mlua::Error::RuntimeError(format!(
                    "{METHOD}: missing 'value' (arg #2), usage: msg:{METHOD}(id, value [, timestamp])"
                ))
            })?;
            let value: String = match &value_arg {
                LuaValue::String(s) => s.to_str().map_err(|_| {
                    mlua::Error::RuntimeError(format!(
                        "{METHOD}: 'value' (arg #2) string is not valid UTF-8"
                    ))
                })?.to_string(),
                LuaValue::Integer(n) => n.to_string(),
                LuaValue::Number(n) => n.to_string(),
                LuaValue::Boolean(b) => b.to_string(),
                _ => {
                    return Err(mlua::Error::RuntimeError(format!(
                        "{METHOD}: 'value' (arg #2) must be a string, number, or boolean, got {}",
                        lua_value_desc(&value_arg)
                    )))
                }
            };
            let timestamp = extract_timestamp(METHOD, args_iter.next(), this.timestamp)?;

            let val = Value {
                id,
                timestamp,
                value_type: ValueType::String,
                value_payload: value.into_bytes(),
            };
            this.values.push(val);
            Ok(())
        });

        methods.add_method_mut("add_bool_value", |_, this, args: mlua::MultiValue| {
            const METHOD: &str = "add_bool_value";
            let mut args_iter = args.into_iter();
            let id = extract_id(
                METHOD,
                args_iter.next().ok_or_else(|| {
                    mlua::Error::RuntimeError(format!(
                        "{METHOD}: missing 'id' (arg #1), usage: msg:{METHOD}(id, value [, timestamp])"
                    ))
                })?,
            )?;
            let value_arg = args_iter.next().ok_or_else(|| {
                mlua::Error::RuntimeError(format!(
                    "{METHOD}: missing 'value' (arg #2), usage: msg:{METHOD}(id, value [, timestamp])"
                ))
            })?;
            // Accept boolean directly, or use Lua truthiness (nil/false = false, everything else = true)
            let value: bool = match &value_arg {
                LuaValue::Boolean(b) => *b,
                LuaValue::Nil => false,
                LuaValue::Integer(n) => *n != 0,
                LuaValue::Number(n) => *n != 0.0,
                _ => true,
            };
            let timestamp = extract_timestamp(METHOD, args_iter.next(), this.timestamp)?;

            let val = Value {
                id,
                timestamp,
                value_type: ValueType::Boolean,
                value_payload: vec![if value { 1 } else { 0 }],
            };
            this.values.push(val);
            Ok(())
        });

        methods.add_method("get_value", |lua, this, id: Option<String>| {
            // Return nil if id is nil
            let id_str = match id {
                Some(s) => s,
                None => return Ok(LuaValue::Nil),
            };
            let value = this.values.iter().find(|v| v.id == id_str);

            match value {
                Some(val) => match val.value_type {
                    // Boolean
                    ValueType::Boolean => {
                        let b = val.value_payload.first().map(|&b| b != 0).unwrap_or(false);
                        Ok(LuaValue::Boolean(b))
                    }

                    // Signed integers
                    ValueType::Int8 => {
                        if !val.value_payload.is_empty() {
                            Ok(LuaValue::Integer(val.value_payload[0] as i8 as LuaInteger))
                        } else {
                            Ok(LuaValue::Nil)
                        }
                    }
                    ValueType::Int16 => {
                        if val.value_payload.len() >= 2 {
                            let bytes: [u8; 2] = val.value_payload[..2].try_into().unwrap();
                            Ok(LuaValue::Integer(i16::from_le_bytes(bytes) as LuaInteger))
                        } else {
                            Ok(LuaValue::Nil)
                        }
                    }
                    ValueType::Int32 => {
                        if val.value_payload.len() >= 4 {
                            let bytes: [u8; 4] = val.value_payload[..4].try_into().unwrap();
                            Ok(LuaValue::Integer(i32::from_le_bytes(bytes) as LuaInteger))
                        } else {
                            Ok(LuaValue::Nil)
                        }
                    }
                    ValueType::Int64 => {
                        if val.value_payload.len() >= 8 {
                            let bytes: [u8; 8] = val.value_payload[..8].try_into().unwrap();
                            let i = i64::from_le_bytes(bytes);
                            match LuaInteger::try_from(i) {
                                Ok(n) => Ok(LuaValue::Integer(n)),
                                Err(_) => Ok(LuaValue::Number(i as f64)),
                            }
                        } else {
                            Ok(LuaValue::Nil)
                        }
                    }

                    // Unsigned integers (convert to i64, will overflow for large u64)
                    ValueType::UInt8 => {
                        if !val.value_payload.is_empty() {
                            Ok(LuaValue::Integer(val.value_payload[0] as LuaInteger))
                        } else {
                            Ok(LuaValue::Nil)
                        }
                    }
                    ValueType::UInt16 => {
                        if val.value_payload.len() >= 2 {
                            let bytes: [u8; 2] = val.value_payload[..2].try_into().unwrap();
                            Ok(LuaValue::Integer(u16::from_le_bytes(bytes) as LuaInteger))
                        } else {
                            Ok(LuaValue::Nil)
                        }
                    }
                    ValueType::UInt32 => {
                        if val.value_payload.len() >= 4 {
                            let bytes: [u8; 4] = val.value_payload[..4].try_into().unwrap();
                            let u = u32::from_le_bytes(bytes);
                            match LuaInteger::try_from(u as i64) {
                                Ok(n) => Ok(LuaValue::Integer(n)),
                                Err(_) => Ok(LuaValue::Number(u as f64)),
                            }
                        } else {
                            Ok(LuaValue::Nil)
                        }
                    }
                    ValueType::UInt64 => {
                        if val.value_payload.len() >= 8 {
                            let bytes: [u8; 8] = val.value_payload[..8].try_into().unwrap();
                            let u = u64::from_le_bytes(bytes);
                            if u <= LuaInteger::MAX as u64 {
                                Ok(LuaValue::Integer(u as LuaInteger))
                            } else {
                                Ok(LuaValue::Number(u as f64))
                            }
                        } else {
                            Ok(LuaValue::Nil)
                        }
                    }

                    // Floating point
                    ValueType::Float32 => {
                        if val.value_payload.len() >= 4 {
                            let bytes: [u8; 4] = val.value_payload[..4].try_into().unwrap();
                            Ok(LuaValue::Number(f32::from_le_bytes(bytes) as f64))
                        } else {
                            Ok(LuaValue::Nil)
                        }
                    }
                    ValueType::Float64 => {
                        if val.value_payload.len() >= 8 {
                            let bytes: [u8; 8] = val.value_payload[..8].try_into().unwrap();
                            Ok(LuaValue::Number(f64::from_le_bytes(bytes)))
                        } else {
                            Ok(LuaValue::Nil)
                        }
                    }

                    // String
                    ValueType::String => {
                        Ok(LuaValue::String(lua.create_string(&val.value_payload)?))
                    }

                    // Arrays are not yet supported, return nil
                    _ => Ok(LuaValue::Nil),
                },
                None => Ok(LuaValue::Nil),
            }
        });
    }
}

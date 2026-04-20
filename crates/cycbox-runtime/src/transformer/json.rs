use async_trait::async_trait;
use cycbox_sdk::prelude::*;
use serde_json::value::Map;
use std::str;

pub const JSON_TRANSFORMER_ID: &str = "json_transformer";

#[derive(Clone, Debug, Default)]
pub struct JsonTransformer;

impl JsonTransformer {
    pub fn new() -> Self {
        Self
    }

    /// Recursively flatten a JSON value into a flat map with `.` separator and plain array indices.
    fn flatten_value(
        current: &serde_json::Value,
        parent_key: String,
        depth: u32,
        flattened: &mut Map<String, serde_json::Value>,
    ) {
        if depth == 0 {
            if let Some(map) = current.as_object() {
                for (k, v) in map {
                    Self::flatten_value(v, k.clone(), 1, flattened);
                }
            }
            return;
        }

        if let Some(map) = current.as_object() {
            if map.is_empty() {
                return;
            }
            for (k, v) in map {
                let key = format!("{}.{}", parent_key, k);
                Self::flatten_value(v, key, depth + 1, flattened);
            }
        } else if let Some(arr) = current.as_array() {
            if arr.is_empty() {
                return;
            }
            for (i, v) in arr.iter().enumerate() {
                let key = format!("{}.{}", parent_key, i);
                Self::flatten_value(v, key, depth + 1, flattened);
            }
        } else {
            flattened.insert(parent_key, current.clone());
        }
    }

    /// Parse JSON object into values, flattening nested objects/arrays with `.` separator.
    fn parse_json(&self, timestamp: u64, line: &str) -> Result<Vec<Value>, CycBoxError> {
        if !line.starts_with('{') {
            return Err(CycBoxError::InvalidFormat("Not a JSON object".to_string()));
        }

        let json_val: serde_json::Value = serde_json::from_str(line)
            .map_err(|e| CycBoxError::InvalidFormat(format!("Failed to parse JSON: {}", e)))?;

        let mut flat = Map::new();
        Self::flatten_value(&json_val, String::new(), 0, &mut flat);

        let mut values = Vec::new();

        for (key, val) in flat {
            let value = match val {
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        Value {
                            id: key,
                            timestamp,
                            value_type: ValueType::Int64,
                            value_payload: i.to_le_bytes().to_vec(),
                        }
                    } else if let Some(u) = n.as_u64() {
                        Value {
                            id: key,
                            timestamp,
                            value_type: ValueType::UInt64,
                            value_payload: u.to_le_bytes().to_vec(),
                        }
                    } else if let Some(f) = n.as_f64() {
                        Value {
                            id: key,
                            timestamp,
                            value_type: ValueType::Float64,
                            value_payload: f.to_le_bytes().to_vec(),
                        }
                    } else {
                        continue;
                    }
                }
                serde_json::Value::Bool(b) => Value {
                    id: key,
                    timestamp,
                    value_type: ValueType::Boolean,
                    value_payload: vec![if b { 1 } else { 0 }],
                },
                serde_json::Value::String(s) => Value {
                    id: key,
                    timestamp,
                    value_type: ValueType::String,
                    value_payload: s.into_bytes(),
                },
                _ => continue,
            };

            values.push(value);
        }

        Ok(values)
    }
}

impl Transformer for JsonTransformer {
    fn on_receive(&self, message: &mut Message) -> Result<(), CycBoxError> {
        let line = str::from_utf8(&message.payload)
            .map_err(|_| CycBoxError::InvalidFormat("Invalid UTF-8 string".to_string()))?;

        let trimmed = line.trim();

        let prefix = message
            .metadata_value("mqtt_topic")
            .and_then(|v| v.as_string())
            .map(|topic| topic.replace('/', "."));

        let mut values = self.parse_json(message.timestamp, trimmed)?;

        if let Some(prefix) = prefix {
            for v in &mut values {
                v.id = format!("{}.{}", prefix, v.id);
            }
        }

        message.values.extend(values);

        Ok(())
    }
}

#[async_trait]
impl Manifestable for JsonTransformer {
    async fn manifest(&self, locale: &str) -> Manifest {
        let l10n = crate::l10n::get_l10n();
        Manifest {
            id: JSON_TRANSFORMER_ID.to_string(),
            name: l10n.get(locale, "json-transformer-name"),
            description: l10n.get(locale, "json-transformer-description"),
            category: PluginCategory::Transformer,
            ..Default::default()
        }
    }
}

#[async_trait]
impl Configurable for JsonTransformer {}

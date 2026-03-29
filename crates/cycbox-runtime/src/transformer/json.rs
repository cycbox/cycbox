use async_trait::async_trait;
use cycbox_sdk::prelude::*;
use std::str;

pub const JSON_TRANSFORMER_ID: &str = "json_transformer";

#[derive(Clone, Debug, Default)]
pub struct JsonTransformer;

impl JsonTransformer {
    pub fn new() -> Self {
        Self
    }

    /// Parse JSON object into values
    fn parse_json(&self, timestamp: u64, line: &str) -> Result<Vec<Value>, CycBoxError> {
        // Parse JSON object - use IndexMap to preserve key order
        let json_obj: indexmap::IndexMap<String, serde_json::Value> = serde_json::from_str(line)
            .map_err(|e| CycBoxError::InvalidFormat(format!("Failed to parse JSON: {}", e)))?;

        let mut values = Vec::new();

        for (key, val) in json_obj {
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
        // Convert message payload to UTF-8 string
        let line = str::from_utf8(&message.payload)
            .map_err(|_| CycBoxError::InvalidFormat("Invalid UTF-8 string".to_string()))?;

        let trimmed = line.trim();

        // Parse JSON and extract values
        let values = self.parse_json(message.timestamp, trimmed)?;
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

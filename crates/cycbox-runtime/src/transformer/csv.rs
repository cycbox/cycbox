use async_trait::async_trait;
use cycbox_sdk::prelude::*;
use std::str;

pub const CSV_TRANSFORMER_ID: &str = "csv_transformer";

#[derive(Clone, Debug, Default)]
pub struct CsvTransformer;

impl CsvTransformer {
    pub fn new() -> Self {
        Self
    }

    /// Parse line into typed values
    /// Splits on whitespace AND commas, treats all as single flat list
    /// Detects type: integer → Int64, float → Float64, bool → Boolean, otherwise → String
    fn parse_line(&self, timestamp: u64, line: &str) -> Vec<Value> {
        let mut result = Vec::new();

        // Split by whitespace and commas, filter empty strings
        let values_str: Vec<&str> = line
            .split(&[' ', '\t', ','][..])
            .filter(|s| !s.is_empty())
            .collect();

        for (index, value_str) in values_str.iter().enumerate() {
            let trimmed = value_str.trim();
            let id = format!("csv_{}", index);

            let value = if let Ok(i) = trimmed.parse::<i64>() {
                Value {
                    id,
                    timestamp,
                    value_type: ValueType::Int64,
                    value_payload: i.to_le_bytes().to_vec(),
                }
            } else if let Ok(f) = trimmed.parse::<f64>() {
                Value {
                    id,
                    timestamp,
                    value_type: ValueType::Float64,
                    value_payload: f.to_le_bytes().to_vec(),
                }
            } else if trimmed.eq_ignore_ascii_case("true") || trimmed.eq_ignore_ascii_case("false")
            {
                Value {
                    id,
                    timestamp,
                    value_type: ValueType::Boolean,
                    value_payload: vec![if trimmed.eq_ignore_ascii_case("true") {
                        1
                    } else {
                        0
                    }],
                }
            } else {
                Value {
                    id,
                    timestamp,
                    value_type: ValueType::String,
                    value_payload: trimmed.as_bytes().to_vec(),
                }
            };

            result.push(value);
        }

        result
    }
}

impl Transformer for CsvTransformer {
    fn on_receive(&self, message: &mut Message) -> Result<(), CycBoxError> {
        // Convert message payload to UTF-8 string
        let line = str::from_utf8(&message.payload)
            .map_err(|_| CycBoxError::InvalidFormat("Invalid UTF-8 string".to_string()))?;

        let trimmed = line.trim();

        // Parse CSV values
        let values = self.parse_line(message.timestamp, trimmed);
        message.values.extend(values);

        Ok(())
    }
}

#[async_trait]
impl Manifestable for CsvTransformer {
    async fn manifest(&self, locale: &str) -> Manifest {
        let l10n = crate::l10n::get_l10n();
        Manifest {
            id: CSV_TRANSFORMER_ID.to_string(),
            name: l10n.get(locale, "csv-transformer-name"),
            description: l10n.get(locale, "csv-transformer-description"),
            category: PluginCategory::Transformer,
            ..Default::default()
        }
    }
}

#[async_trait]
impl Configurable for CsvTransformer {}

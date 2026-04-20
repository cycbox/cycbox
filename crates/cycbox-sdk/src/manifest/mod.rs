mod form_condition;
mod form_field;
mod form_field_option;
mod form_group;
mod form_utils;
mod form_value;
mod manifest_values;

use async_trait::async_trait;
pub use form_condition::{ConditionOperator, FormCondition};
pub use form_field::{FieldType, FormField};
pub use form_field_option::FormFieldOption;
pub use form_group::FormGroup;
pub use form_utils::FormUtils;
pub use form_value::FormValue;
pub use manifest_values::{ManifestValues, ValidationError};

use crate::message_input::MessageInputGroup;

use crate::CycBoxError;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginCategory {
    Transport,
    Codec,
    Script,
    Formatter,
    Transformer,
    MessageInput,
    RunMode,
}

#[async_trait]
pub trait Configurable {
    async fn config(&mut self, _config: &[FormGroup]) -> Result<(), CycBoxError> {
        Ok(())
    }
}

#[async_trait]
pub trait Manifestable {
    async fn manifest(&self, locale: &str) -> Manifest;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub id: String, // use to identify the module/plugin
    pub version: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub config_schema: Vec<FormGroup>,
    pub configs: Vec<Vec<FormGroup>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub message_input_groups: Vec<MessageInputGroup>,
    pub category: PluginCategory,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lua_script: Option<String>,
    /// Raw dashboard configurations. Each entry is a dashboard with title, icon, and widgets.
    /// Stored as JSON values without parsing widget types.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dashboards: Vec<JsonValue>,
}

impl Default for Manifest {
    fn default() -> Self {
        Manifest {
            id: String::new(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            name: String::new(),
            description: String::new(),
            author: "CycBox".to_string(),
            config_schema: vec![],
            configs: vec![],
            message_input_groups: vec![],
            category: PluginCategory::Transport,
            lua_script: None,
            dashboards: vec![],
        }
    }
}

impl Manifest {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Manifest, CycBoxError> {
        let contents = fs::read_to_string(path)?;
        Self::new_from_str(&contents)
    }

    pub fn new_from_str(s: &str) -> Result<Manifest, CycBoxError> {
        let manifest = serde_json::from_str(s)?;
        Ok(manifest)
    }

    pub fn with_groups(&mut self, group_id: &str, group_label: &str, group_items: Vec<Manifest>) {
        if group_items.is_empty() {
            return;
        }
        let mut item_ids = vec![];
        let mut values = None;
        let mut options: Option<Vec<FormFieldOption>> = None;

        for mut item_manifest in group_items {
            let item_id = item_manifest.id.to_string();
            item_ids.push(item_id.clone());
            if values.is_none() {
                values = Some(vec![FormValue::Text(item_id.clone())]);
            }
            if let Some(ref mut options) = options {
                options.push(FormFieldOption::with_description(
                    item_manifest.name.clone(),
                    FormValue::Text(item_id.clone()),
                    item_manifest.description.clone(),
                ));
            } else {
                options = Some(vec![FormFieldOption::with_description(
                    item_manifest.name.clone(),
                    FormValue::Text(item_id.clone()),
                    item_manifest.description.clone(),
                )]);
            }
            // Add condition to config_schema
            for config in item_manifest.config_schema.iter_mut() {
                config.condition = Some(FormCondition {
                    field_key: format!("app_{}", group_id),
                    operator: ConditionOperator::Equal,
                    value: FormValue::Text(item_id.clone()),
                });
            }

            // Extract requires_codec value from manifest's hidden field
            let requires_codec = item_manifest
                .config_schema
                .iter()
                .flat_map(|group| &group.fields)
                .find(|field| field.key.ends_with("_requires_codec"))
                .and_then(|field| field.values.as_ref())
                .and_then(|values| values.first())
                .and_then(|value| match value {
                    FormValue::Boolean(b) => Some(*b),
                    _ => None,
                })
                .unwrap_or(true); // Default to true (requires codec) if not specified

            // Add hidden field to app group for this codec requirement
            let requires_codec_field = FormField {
                key: format!("app_{}_{}_requires_codec", group_id, item_id),
                field_type: FieldType::BooleanInput,
                label: format!("{} Requires Codec", item_id),
                description: None,
                values: Some(vec![FormValue::Boolean(requires_codec)]),
                options: None,
                is_required: false,
                condition: Some(FormCondition {
                    field_key: "__hidden__".to_string(),
                    operator: ConditionOperator::Equal,
                    value: FormValue::Boolean(true),
                }),
                span: 3,
            };

            if let Some(app_group) = FormUtils::find_group_mut(&mut self.config_schema, "app") {
                app_group.fields.push(requires_codec_field);
            }

            self.config_schema.extend(item_manifest.config_schema);
        }

        let item_len = item_ids.len();
        let condition = if item_len == 1 {
            Some(FormCondition {
                field_key: "__not_exists_key__".to_string(),
                operator: ConditionOperator::Equal,
                value: FormValue::Text("__not_exists_value__".to_string()),
            })
        } else {
            None
        };

        // Primary field
        let field = FormField {
            key: format!("app_{}", group_id),
            field_type: FieldType::TextDropdown,
            label: group_label.to_string(),
            description: None,
            values,
            options,
            is_required: true,
            condition,
            span: 3,
        };
        if let Some(app_group) = FormUtils::find_group_mut(&mut self.config_schema, "app") {
            app_group.fields.push(field)
        }
    }

    pub fn with_encoding_field(&mut self, field_label: &str) {
        // Encoding field for text formatting
        let encoding_field = FormField {
            key: "app_encoding".to_string(),
            field_type: FieldType::TextDropdown,
            label: field_label.to_string(),
            description: None,
            values: Some(vec![FormValue::Text("UTF-8".to_string())]),
            options: Some(vec![
                FormFieldOption::new("UTF-8".to_string(), FormValue::Text("UTF-8".to_string())),
                FormFieldOption::new(
                    "UTF-16LE (Little Endian)".to_string(),
                    FormValue::Text("UTF-16LE".to_string()),
                ),
                FormFieldOption::new(
                    "UTF-16BE (Big Endian)".to_string(),
                    FormValue::Text("UTF-16BE".to_string()),
                ),
                FormFieldOption::new(
                    "GB18030 (Chinese)".to_string(),
                    FormValue::Text("GB18030".to_string()),
                ),
                FormFieldOption::new(
                    "Big5 (Chinese Traditional)".to_string(),
                    FormValue::Text("Big5".to_string()),
                ),
                FormFieldOption::new(
                    "Shift JIS (Japanese)".to_string(),
                    FormValue::Text("Shift_JIS".to_string()),
                ),
                FormFieldOption::new(
                    "EUC-JP (Japanese)".to_string(),
                    FormValue::Text("EUC-JP".to_string()),
                ),
                FormFieldOption::new(
                    "EUC-KR (Korean)".to_string(),
                    FormValue::Text("EUC-KR".to_string()),
                ),
                FormFieldOption::new(
                    "ISO-8859-1 (Latin-1)".to_string(),
                    FormValue::Text("ISO-8859-1".to_string()),
                ),
            ]),
            is_required: true,
            condition: None,
            span: 3,
        };
        if let Some(app_group) = FormUtils::find_group_mut(&mut self.config_schema, "app") {
            app_group.fields.push(encoding_field)
        }
    }
}

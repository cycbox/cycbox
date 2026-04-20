use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::{FieldType, FormField, FormGroup, FormValue, Manifest};
use crate::CycBoxError;
use crate::message_input::MessageInputGroup;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// A lightweight representation of manifest configuration values.
///
/// This type provides a flattened, serializable view of configuration values
/// extracted from a Manifest, making it easier to:
/// - Persist user configurations to files (JSON/YAML)
/// - Load saved configurations and merge them back into manifests
/// - Transfer configuration values across FFI boundaries
///
/// Similar to the Dart implementation in ui/lib/repositories/manifest.dart
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct ManifestValues {
    /// Optional manifest version override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Optional manifest name override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Optional Lua script
    #[serde(skip_serializing)]
    pub lua_script: Option<String>,

    /// Multiple configuration instances, each organized as: {group_key: {field_key: json_value}}
    ///
    /// Using JsonValue allows for flexible serialization while maintaining
    /// compatibility with various field types (text, integer, float, boolean)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub configs: Vec<HashMap<String, HashMap<String, JsonValue>>>,

    /// Message input groups for the send panel
    ///
    /// Matches the Dart `messageInputGroups` field.
    // #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[serde(skip)]
    pub message_input_groups: Vec<MessageInputGroup>,

    /// Raw dashboard configurations. Each entry is a dashboard with title, icon, and widgets.
    /// Stored as JSON values without parsing widget types.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dashboards: Vec<JsonValue>,
}

/// Transport group key migrations for versions < 1.12.0
const TRANSPORT_KEY_MIGRATIONS: &[(&str, &str)] = &[
    ("udp", "udp_transport"),
    ("serial", "serial_port_transport"),
    ("mqtt", "mqtt_transport"),
    ("tcp_client", "tcp_client_transport"),
    ("tcp_server", "tcp_server_transport"),
    ("websocket_client", "websocket_client_transport"),
    ("websocket_server", "websocket_server_transport"),
];

impl ManifestValues {
    /// Create a new empty ManifestValues
    pub fn new() -> Self {
        Self {
            version: None,
            name: None,
            description: None,
            lua_script: None,
            configs: Vec::new(),
            message_input_groups: Vec::new(),
            dashboards: Vec::new(),
        }
    }

    /// Extract configuration values from a Manifest
    ///
    /// Extracts saved configs from manifest.configs and filters to only include:
    /// - The 'app' group (always)
    /// - The transport group selected via `app_transport` field
    /// - The codec group selected via `app_codec` field
    /// - Removes fields ending with 'requires_codec'
    /// - Removes empty groups
    ///
    /// # Arguments
    /// * `manifest` - The manifest to extract values from
    ///
    /// # Returns
    /// A new ManifestValues with filtered configurations
    pub fn from_manifest(manifest: &Manifest) -> Self {
        let mut all_configs = Vec::new();

        // Extract and filter all saved configs
        for config_groups in &manifest.configs {
            let config_map = Self::extract_config_from_groups(config_groups);
            if !config_map.is_empty() {
                // Filter config to only include selected groups
                let filtered_config = Self::filter_config_by_app_selection(&config_map);
                if !filtered_config.is_empty() {
                    all_configs.push(filtered_config);
                }
            }
        }

        // Only include dashboards that have widgets
        let dashboards: Vec<JsonValue> = manifest
            .dashboards
            .iter()
            .filter(|d| {
                d.get("widgets")
                    .and_then(|w| w.as_array())
                    .map(|arr| !arr.is_empty())
                    .unwrap_or(false)
            })
            .cloned()
            .collect();

        Self {
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            name: Some(manifest.name.clone()),
            description: if manifest.description.is_empty() {
                None
            } else {
                Some(manifest.description.clone())
            },
            lua_script: manifest.lua_script.clone(),
            configs: all_configs,
            message_input_groups: manifest.message_input_groups.clone(),
            dashboards,
        }
    }

    /// Filter configuration to only include relevant groups based on app selection
    ///
    /// This method implements the filtering logic to only persist the configuration
    /// groups that are actually being used:
    /// - Always include 'app' group
    /// - Include group matching selected transport (from app_transport field)
    /// - Include group matching selected codec (from app_codec field)
    /// - Remove fields ending with 'requires_codec'
    /// - Remove empty groups
    ///
    /// # Arguments
    /// * `config_map` - The full config map with all groups
    ///
    /// # Returns
    /// A filtered HashMap containing only relevant groups
    fn filter_config_by_app_selection(
        config_map: &HashMap<String, HashMap<String, JsonValue>>,
    ) -> HashMap<String, HashMap<String, JsonValue>> {
        let mut filtered_config: HashMap<String, HashMap<String, JsonValue>> = HashMap::new();

        // Always include app group if it exists
        if let Some(app_group) = config_map.get("app") {
            filtered_config.insert("app".to_string(), app_group.clone());

            // Get selected transport and codec from app group
            let selected_transport = app_group
                .get("app_transport")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let selected_codec = app_group
                .get("app_codec")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            // Include the selected transport group if it exists
            if let Some(transport_name) = selected_transport
                && let Some(transport_group) = config_map.get(&transport_name)
            {
                filtered_config.insert(transport_name, transport_group.clone());
            }

            // Include the selected codec group if it exists
            if let Some(codec_name) = selected_codec
                && let Some(codec_group) = config_map.get(&codec_name)
            {
                filtered_config.insert(codec_name, codec_group.clone());
            }
        }

        // Remove fields ending with 'requires_codec' from all groups
        for group in filtered_config.values_mut() {
            group.retain(|key, _| !key.ends_with("requires_codec"));
        }

        // Remove empty groups
        filtered_config.retain(|_, group| !group.is_empty());

        filtered_config
    }

    /// Extract configuration values from a single form group array
    ///
    /// Helper method to extract values from a Vec<FormGroup>
    ///
    /// # Arguments
    /// * `groups` - The form groups to extract from
    ///
    /// # Returns
    /// A HashMap representing a single config instance
    fn extract_config_from_groups(
        groups: &[FormGroup],
    ) -> HashMap<String, HashMap<String, JsonValue>> {
        let mut config_map: HashMap<String, HashMap<String, JsonValue>> = HashMap::new();

        for group in groups {
            let mut fields = HashMap::new();

            for field in &group.fields {
                // Skip fields without values
                if field.values.is_none() || field.values.as_ref().unwrap().is_empty() {
                    continue;
                }

                // Extract the first value and convert to JsonValue
                if let Some(json_value) =
                    Self::form_value_to_json(field.values.as_ref().unwrap().first().unwrap())
                {
                    fields.insert(field.key.clone(), json_value);
                }
            }

            // Only add groups that have fields with values
            if !fields.is_empty() {
                config_map.insert(group.key.clone(), fields);
            }
        }

        config_map
    }

    /// Convert FormValue to JsonValue
    fn form_value_to_json(form_value: &FormValue) -> Option<JsonValue> {
        match form_value {
            FormValue::Text(s) => Some(JsonValue::String(s.clone())),
            FormValue::Integer(i) => Some(JsonValue::Number((*i).into())),
            FormValue::Float(f) => serde_json::Number::from_f64(*f).map(JsonValue::Number),
            FormValue::Boolean(b) => Some(JsonValue::Bool(*b)),
        }
    }

    /// Load ManifestValues from JSON string
    ///
    /// # Arguments
    /// * `content` - JSON string containing configuration values
    ///
    /// # Returns
    /// Parsed ManifestValues or error
    pub fn from_json_str(content: &str) -> Result<Self, CycBoxError> {
        let values: Self = serde_json::from_str(content)?;
        Ok(Self::migrate_transport_keys(values))
    }

    /// Convert ManifestValues to pretty-printed JSON string
    ///
    /// Uses 2-space indentation for readability
    ///
    /// # Returns
    /// JSON string or error
    pub fn to_json_string(&self) -> Result<String, CycBoxError> {
        let json_string = serde_json::to_string_pretty(self)?;
        Ok(json_string)
    }

    /// Load ManifestValues from JSON file
    ///
    /// # Arguments
    /// * `path` - Path to JSON file
    ///
    /// # Returns
    /// Parsed ManifestValues or error
    pub fn load_from_json_file<P: AsRef<Path>>(path: P) -> Result<Self, CycBoxError> {
        let content = fs::read_to_string(path.as_ref())?;
        Self::from_json_str(&content)
    }

    /// Save ManifestValues to JSON file
    ///
    /// Creates parent directories if they don't exist.
    ///
    /// # Arguments
    /// * `path` - Path to save JSON file
    ///
    /// # Returns
    /// Ok or error
    pub fn save_to_json_file<P: AsRef<Path>>(&self, path: P) -> Result<(), CycBoxError> {
        // Ensure parent directory exists
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }

        let json_string = self.to_json_string()?;
        fs::write(path.as_ref(), json_string)?;
        Ok(())
    }

    /// Load ManifestValues from a Lua string
    ///
    /// Expects the Lua format produced by the Flutter UI: Lua code at the top,
    /// followed by JSON configuration inside a `--[[ ... ]]` block comment.
    /// The Lua code (everything before `--[[`) becomes the `lua_script` field,
    /// overriding any `lua_script` that may appear inside the JSON.
    ///
    /// # Arguments
    /// * `content` - Lua file content
    ///
    /// # Returns
    /// Parsed ManifestValues or error
    pub fn from_lua_str(content: &str) -> Result<Self, CycBoxError> {
        let block_start = content.find("--[[").ok_or_else(|| {
            CycBoxError::Parse("No --[[ block comment found in Lua file".to_string())
        })?;

        let after_open = block_start + 4;
        let block_end = content[after_open..].find("]]").ok_or_else(|| {
            CycBoxError::Parse("Unclosed --[[ block comment in Lua file".to_string())
        })? + after_open;

        let lua_code = content[..block_start].trim();
        let json_content = content[after_open..block_end].trim();

        let mut values = Self::from_json_str(json_content)?;

        // Lua code before --[[ takes precedence over any lua_script in json
        values.lua_script = if lua_code.is_empty() {
            None
        } else {
            Some(lua_code.to_string())
        };

        Ok(values)
    }

    /// Load ManifestValues from a Lua file
    ///
    /// # Arguments
    /// * `path` - Path to Lua file
    ///
    /// # Returns
    /// Parsed ManifestValues or error
    pub fn load_from_lua_file<P: AsRef<Path>>(path: P) -> Result<Self, CycBoxError> {
        let content = fs::read_to_string(path.as_ref()).map_err(|e| {
            CycBoxError::Other(format!(
                "Failed to read Lua file: {:?}: {}",
                path.as_ref(),
                e
            ))
        })?;
        Self::from_lua_str(&content)
    }

    /// Serialize ManifestValues to Lua file format
    ///
    /// Produces the format expected by `from_lua_str`: optional Lua code at the
    /// top, followed by a `--[[ JSON ]]` block comment containing the config.
    ///
    /// # Returns
    /// Lua-formatted string or error
    pub fn to_lua_str(&self) -> Result<String, CycBoxError> {
        let json = self.to_json_string()?;
        let lua_header = match &self.lua_script {
            Some(script) if !script.is_empty() => format!("{}\n", script),
            _ => String::new(),
        };
        Ok(format!("{}--[[\n{}\n]]", lua_header, json))
    }

    /// Save ManifestValues to a Lua file
    ///
    /// Creates parent directories if they don't exist.
    ///
    /// # Arguments
    /// * `path` - Path to save Lua file
    ///
    /// # Returns
    /// Ok or error
    pub fn save_to_lua_file<P: AsRef<Path>>(&self, path: P) -> Result<(), CycBoxError> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent).map_err(|e| {
                CycBoxError::Other(format!("Failed to create directory: {:?}: {}", parent, e))
            })?;
        }
        let content = self.to_lua_str()?;
        fs::write(path.as_ref(), content).map_err(|e| {
            CycBoxError::Other(format!(
                "Failed to write Lua file: {:?}: {}",
                path.as_ref(),
                e
            ))
        })?;
        Ok(())
    }

    /// Merge configuration values into a Manifest
    ///
    /// Creates a new Manifest with values from this ManifestValues applied.
    /// This replaces all configs in the manifest with the configs from ManifestValues.
    /// Each config is merged with the config_schema template.
    ///
    /// Only updates fields that:
    /// 1. Exist in both the config_schema and the config values
    /// 2. Have values that match the expected field type
    ///
    /// Type mismatches are silently skipped (field keeps its original value).
    ///
    /// # Arguments
    /// * `manifest` - The manifest to merge values into
    ///
    /// # Returns
    /// A new Manifest with updated configs
    pub fn merge_into_manifest(&self, manifest: Manifest) -> Manifest {
        // Clone the manifest to avoid mutation
        let mut merged_manifest = manifest;

        // Update metadata if provided
        if let Some(version) = &self.version {
            merged_manifest.version = version.clone();
        }
        if let Some(name) = &self.name {
            merged_manifest.name = name.clone();
        }
        if let Some(description) = &self.description {
            merged_manifest.description = description.clone();
        }
        if let Some(lua_script) = &self.lua_script {
            merged_manifest.lua_script = Some(lua_script.clone());
        }
        if !self.message_input_groups.is_empty() {
            merged_manifest.message_input_groups = self.message_input_groups.clone();
        }
        if !self.dashboards.is_empty() {
            merged_manifest.dashboards = self.dashboards.clone();
        }

        // Merge all configuration values into new config instances
        merged_manifest.configs = self
            .configs
            .iter()
            .map(|config_values| {
                // Clone the config_schema as template for each config
                merged_manifest
                    .config_schema
                    .iter()
                    .map(|group| self.merge_group_with_values(group.clone(), config_values))
                    .collect()
            })
            .collect();

        merged_manifest
    }

    /// Merge values into a single FormGroup using a specific config's values
    ///
    /// # Arguments
    /// * `group` - The form group (usually from config_schema)
    /// * `config_values` - The values for this specific config instance
    fn merge_group_with_values(
        &self,
        mut group: FormGroup,
        config_values: &HashMap<String, HashMap<String, JsonValue>>,
    ) -> FormGroup {
        // Check if this group exists in config
        if let Some(group_config) = config_values.get(&group.key) {
            // Merge fields
            group.fields = group
                .fields
                .into_iter()
                .map(|field| Self::merge_field(field, group_config))
                .collect();
        }

        group
    }

    /// Merge values into a single FormField
    ///
    /// # Arguments
    /// * `field` - The form field
    /// * `group_config` - The values for the group containing this field
    fn merge_field(mut field: FormField, group_config: &HashMap<String, JsonValue>) -> FormField {
        // Check if this field exists in config
        if let Some(config_value) = group_config.get(&field.key) {
            // Validate and convert value
            if let Some(form_value) =
                Self::validate_and_convert_value(config_value, &field.field_type)
            {
                // Apply value using the field's set_value method
                field.set_value(form_value);
            }
        }

        field
    }

    /// Validate and convert a JSON value to match the expected field type
    ///
    /// Returns None if the value doesn't match the expected type.
    ///
    /// # Arguments
    /// * `json_value` - The JSON value to validate and convert
    /// * `field_type` - The expected field type
    ///
    /// # Returns
    /// Some(FormValue) if valid, None if type mismatch
    fn validate_and_convert_value(
        json_value: &JsonValue,
        field_type: &FieldType,
    ) -> Option<FormValue> {
        match field_type {
            // Text fields expect String
            FieldType::TextInput
            | FieldType::TextMultiline
            | FieldType::TextChoiceChip
            | FieldType::TextDropdown
            | FieldType::TextInputDropdown
            | FieldType::Code
            | FieldType::FileInput => {
                if let JsonValue::String(s) = json_value {
                    Some(FormValue::Text(s.clone()))
                } else {
                    None
                }
            }

            // Integer fields expect i64
            FieldType::IntegerInput
            | FieldType::IntegerChoiceChip
            | FieldType::IntegerDropdown
            | FieldType::IntegerInputDropdown => {
                if let JsonValue::Number(n) = json_value {
                    n.as_i64().map(FormValue::Integer)
                } else {
                    None
                }
            }

            // Float fields expect f64 (can also accept integer and coerce to float)
            FieldType::FloatInput
            | FieldType::FloatChoiceChip
            | FieldType::FloatDropdown
            | FieldType::FloatInputDropdown => {
                if let JsonValue::Number(n) = json_value {
                    n.as_f64().map(FormValue::Float)
                } else {
                    None
                }
            }

            // Boolean fields expect bool
            FieldType::BooleanInput | FieldType::BooleanChoiceChip | FieldType::BooleanDropdown => {
                if let JsonValue::Bool(b) = json_value {
                    Some(FormValue::Boolean(*b))
                } else {
                    None
                }
            }
        }
    }

    /// Migrate transport group keys and field keys for versions < 1.12.0.
    fn migrate_transport_keys(mut values: Self) -> Self {
        let migration_map: HashMap<&str, &str> = TRANSPORT_KEY_MIGRATIONS.iter().copied().collect();

        for config in &mut values.configs {
            let keys: Vec<String> = config.keys().cloned().collect();
            for old_group_key in keys {
                if let Some(&new_group_key) = migration_map.get(old_group_key.as_str()) {
                    if let Some(old_fields) = config.remove(&old_group_key) {
                        let new_fields: HashMap<String, JsonValue> = old_fields
                            .into_iter()
                            .map(|(field_key, value)| {
                                let new_key =
                                    if field_key.starts_with(&format!("{}_", old_group_key)) {
                                        field_key.replacen(
                                            &format!("{}_", old_group_key),
                                            &format!("{}_", new_group_key),
                                            1,
                                        )
                                    } else {
                                        field_key
                                    };
                                (new_key, value)
                            })
                            .collect();
                        config.insert(new_group_key.to_string(), new_fields);
                    }
                } else if old_group_key == "app" {
                    // Migrate app_transport value
                    if let Some(app_fields) = config.get_mut("app")
                        && let Some(JsonValue::String(transport_val)) =
                            app_fields.get("app_transport")
                        && let Some(&new_transport) = migration_map.get(transport_val.as_str())
                    {
                        app_fields.insert(
                            "app_transport".to_string(),
                            JsonValue::String(new_transport.to_string()),
                        );
                    }
                }
            }
        }

        values
    }
}

impl Default for ManifestValues {
    fn default() -> Self {
        Self::new()
    }
}

/// An error produced when validating ManifestValues against a Manifest schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// Group key where the error occurred
    pub group: String,
    /// Field key (None for group-level errors)
    pub field: Option<String>,
    /// Human-readable error message
    pub message: String,
}

impl ManifestValues {
    /// Validate this ManifestValues against a Manifest's config_schema.
    ///
    /// Checks each config entry for:
    /// - Unknown group keys
    /// - Unknown field keys within each group
    /// - Type mismatches between JSON values and expected FieldType
    /// - Invalid option values for strict choice fields (ChoiceChip/Dropdown, excluding InputDropdown)
    ///
    /// # Returns
    /// `Ok(())` if all values are valid, `Err(Vec<ValidationError>)` with all errors found.
    pub fn validate(&self, manifest: &Manifest) -> Result<(), Vec<ValidationError>> {
        let schema_map: std::collections::HashMap<&str, &FormGroup> = manifest
            .config_schema
            .iter()
            .map(|g| (g.key.as_str(), g))
            .collect();

        let mut errors: Vec<ValidationError> = Vec::new();

        for config in &self.configs {
            for (group_key, fields_map) in config {
                let Some(schema_group) = schema_map.get(group_key.as_str()) else {
                    errors.push(ValidationError {
                        group: group_key.clone(),
                        field: None,
                        message: "Unknown group".to_string(),
                    });
                    continue;
                };

                let field_map: std::collections::HashMap<&str, &FormField> = schema_group
                    .fields
                    .iter()
                    .map(|f| (f.key.as_str(), f))
                    .collect();

                for (field_key, json_value) in fields_map {
                    let Some(schema_field) = field_map.get(field_key.as_str()) else {
                        errors.push(ValidationError {
                            group: group_key.clone(),
                            field: Some(field_key.clone()),
                            message: "Unknown field".to_string(),
                        });
                        continue;
                    };

                    let converted =
                        Self::validate_and_convert_value(json_value, &schema_field.field_type);

                    if converted.is_none() {
                        let expected = match &schema_field.field_type {
                            FieldType::TextInput
                            | FieldType::TextMultiline
                            | FieldType::TextChoiceChip
                            | FieldType::TextDropdown
                            | FieldType::TextInputDropdown
                            | FieldType::Code
                            | FieldType::FileInput => "string",
                            FieldType::IntegerInput
                            | FieldType::IntegerChoiceChip
                            | FieldType::IntegerDropdown
                            | FieldType::IntegerInputDropdown => "integer",
                            FieldType::FloatInput
                            | FieldType::FloatChoiceChip
                            | FieldType::FloatDropdown
                            | FieldType::FloatInputDropdown => "float",
                            FieldType::BooleanInput
                            | FieldType::BooleanChoiceChip
                            | FieldType::BooleanDropdown => "boolean",
                        };
                        errors.push(ValidationError {
                            group: group_key.clone(),
                            field: Some(field_key.clone()),
                            message: format!(
                                "Expected {}, got {}",
                                expected,
                                json_value_type_name(json_value)
                            ),
                        });
                        continue;
                    }

                    // For strict choice fields (not InputDropdown), validate against options
                    let is_strict_choice = matches!(
                        &schema_field.field_type,
                        FieldType::TextChoiceChip
                            | FieldType::IntegerChoiceChip
                            | FieldType::FloatChoiceChip
                            | FieldType::BooleanChoiceChip
                            | FieldType::TextDropdown
                            | FieldType::IntegerDropdown
                            | FieldType::FloatDropdown
                            | FieldType::BooleanDropdown
                    );

                    if is_strict_choice && let Some(options) = &schema_field.options {
                        let form_val = converted.unwrap();
                        let is_valid = options.iter().any(|opt| opt.value == form_val);
                        if !is_valid {
                            let allowed: Vec<String> = options
                                .iter()
                                .map(|opt| format!("{:?}", opt.value))
                                .collect();
                            errors.push(ValidationError {
                                group: group_key.clone(),
                                field: Some(field_key.clone()),
                                message: format!(
                                    "Value not in allowed options: [{}]",
                                    allowed.join(", ")
                                ),
                            });
                        }
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

fn json_value_type_name(v: &JsonValue) -> &'static str {
    match v {
        JsonValue::String(_) => "string",
        JsonValue::Number(n) => {
            if n.is_f64() && !n.is_i64() && !n.is_u64() {
                "float"
            } else {
                "number"
            }
        }
        JsonValue::Bool(_) => "boolean",
        JsonValue::Null => "null",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

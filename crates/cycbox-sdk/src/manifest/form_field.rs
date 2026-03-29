use serde::{Deserialize, Serialize};

use super::form_condition::FormCondition;
use super::form_field_option::FormFieldOption;
use super::form_value::FormValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    TextInput,     // Text input field (single line)
    TextMultiline, // Multiline text input field (up to 10 lines)
    IntegerInput,  // Integer input field
    BooleanInput,  // Boolean input field
    FloatInput,    // Float input field

    TextChoiceChip,    // Text choice chip field
    IntegerChoiceChip, // Integer choice chip field
    BooleanChoiceChip, // Boolean choice chip field
    FloatChoiceChip,   // Float choice chip field

    TextDropdown,    // Text dropdown field
    IntegerDropdown, // Integer dropdown field
    BooleanDropdown, // Boolean dropdown field
    FloatDropdown,   // Float dropdown field

    TextInputDropdown,    // Text dropdown field can input new value
    IntegerInputDropdown, // Integer dropdown field can input new value
    FloatInputDropdown,   // Float dropdown field can input new value

    Code,      // Code editor field for syntax highlighting
    FileInput, // File picker field that returns file path as string
}

fn default_span() -> u8 {
    12
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    pub key: String,
    pub field_type: FieldType,
    pub label: String,
    pub is_required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<FormValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<FormFieldOption>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<FormCondition>,
    #[serde(default = "default_span")]
    pub span: u8,
}

impl FormField {
    /// Get the current value of the field (first value for single-value fields)
    pub fn get_value(&self) -> Option<&FormValue> {
        self.values.as_ref()?.first()
    }

    /// Set the value of the field (replaces all values)
    pub fn set_value(&mut self, value: FormValue) {
        self.values = Some(vec![value]);
    }

    /// Clear the value of the field
    pub fn clear_value(&mut self) {
        self.values = None;
    }

    /// Get all values of the field
    pub fn get_values(&self) -> Option<&Vec<FormValue>> {
        self.values.as_ref()
    }

    /// Get the multiple values of the field (alias for get_values for compatibility)
    pub fn get_multiple_values(&self) -> Option<&Vec<FormValue>> {
        self.values.as_ref()
    }

    /// Set the multiple values of the field
    pub fn set_multiple_values(&mut self, values: Vec<FormValue>) {
        self.values = Some(values);
    }

    /// Append a value to the multiple values
    pub fn append_value(&mut self, value: FormValue) {
        match &mut self.values {
            Some(values) => values.push(value),
            None => self.values = Some(vec![value]),
        }
    }

    /// Append multiple values to the multiple values
    pub fn append_values(&mut self, mut values: Vec<FormValue>) {
        match &mut self.values {
            Some(existing_values) => existing_values.append(&mut values),
            None => self.values = Some(values),
        }
    }

    /// Clear the multiple values of the field
    pub fn clear_multiple_values(&mut self) {
        self.values = None;
    }

    /// Get value as string (for text fields)
    pub fn get_text_value(&self) -> Option<&str> {
        match self.values.as_ref()?.first()? {
            FormValue::Text(text) => Some(text),
            _ => None,
        }
    }

    /// Get value as integer (for integer fields)
    pub fn get_integer_value(&self) -> Option<i64> {
        match self.values.as_ref()?.first()? {
            FormValue::Integer(val) => Some(*val),
            _ => None,
        }
    }

    /// Get value as float (for float fields)
    pub fn get_float_value(&self) -> Option<f64> {
        match self.values.as_ref()?.first()? {
            FormValue::Float(val) => Some(*val),
            _ => None,
        }
    }

    /// Get value as boolean (for boolean fields)
    pub fn get_boolean_value(&self) -> Option<bool> {
        match self.values.as_ref()?.first()? {
            FormValue::Boolean(val) => Some(*val),
            _ => None,
        }
    }

    /// Set text value
    pub fn set_text_value(&mut self, text: String) {
        self.values = Some(vec![FormValue::Text(text)]);
    }

    /// Set integer value
    pub fn set_integer_value(&mut self, val: i64) {
        self.values = Some(vec![FormValue::Integer(val)]);
    }

    /// Set float value
    pub fn set_float_value(&mut self, val: f64) {
        self.values = Some(vec![FormValue::Float(val)]);
    }

    /// Set boolean value
    pub fn set_boolean_value(&mut self, val: bool) {
        self.values = Some(vec![FormValue::Boolean(val)]);
    }

    /// Get multiple values as text strings
    pub fn get_multiple_text_values(&self) -> Vec<&str> {
        self.values.as_ref().map_or(Vec::new(), |values| {
            values
                .iter()
                .filter_map(|v| match v {
                    FormValue::Text(text) => Some(text.as_str()),
                    _ => None,
                })
                .collect()
        })
    }

    /// Get multiple values as integers
    pub fn get_multiple_integer_values(&self) -> Vec<i64> {
        self.values.as_ref().map_or(Vec::new(), |values| {
            values
                .iter()
                .filter_map(|v| match v {
                    FormValue::Integer(val) => Some(*val),
                    _ => None,
                })
                .collect()
        })
    }

    /// Get multiple values as floats
    pub fn get_multiple_float_values(&self) -> Vec<f64> {
        self.values.as_ref().map_or(Vec::new(), |values| {
            values
                .iter()
                .filter_map(|v| match v {
                    FormValue::Float(val) => Some(*val),
                    _ => None,
                })
                .collect()
        })
    }

    /// Get multiple values as booleans
    pub fn get_multiple_boolean_values(&self) -> Vec<bool> {
        self.values.as_ref().map_or(Vec::new(), |values| {
            values
                .iter()
                .filter_map(|v| match v {
                    FormValue::Boolean(val) => Some(*val),
                    _ => None,
                })
                .collect()
        })
    }

    /// Convert a field key to L10n label key format
    /// Example: "transport_mqtt" -> "transport-mqtt-label"
    fn key_to_l10n_key(key: &str, suffix: &str) -> String {
        format!("{}-{}", key.replace('_', "-"), suffix)
    }

    /// Create a new FormField with auto-generated L10n label key
    /// - label: {key with hyphens}-label (for L10n)
    /// - is_required: true (default)
    /// - condition: None
    /// - description: None
    /// - values: None
    /// - options: None
    /// - span: 12 (default)
    pub fn new(key: String, field_type: FieldType) -> Self {
        let label = Self::key_to_l10n_key(&key, "label");
        Self {
            key,
            field_type,
            label,
            description: None,
            values: None,
            options: None,
            is_required: true,
            condition: None,
            span: 12,
        }
    }

    /// Create a new FormField with auto-generated L10n label and description keys
    /// - label: {key with hyphens}-label (for L10n)
    /// - description: {key with hyphens}-description (for L10n)
    /// - is_required: true (default)
    /// - condition: None
    /// - values: None
    /// - options: None
    /// - span: 12 (default)
    pub fn with_description(key: String, field_type: FieldType) -> Self {
        let label = Self::key_to_l10n_key(&key, "label");
        let description = Self::key_to_l10n_key(&key, "description");
        Self {
            key,
            field_type,
            label,
            description: Some(description),
            values: None,
            options: None,
            is_required: true,
            condition: None,
            span: 12,
        }
    }

    /// Set the options for choice/dropdown fields (chainable)
    pub fn with_options(mut self, options: Vec<FormFieldOption>) -> Self {
        self.options = Some(options);
        self
    }

    /// Set whether the field is required (chainable)
    pub fn required(mut self, is_required: bool) -> Self {
        self.is_required = is_required;
        self
    }

    /// Set a condition for conditional field visibility (chainable)
    pub fn with_condition(mut self, condition: FormCondition) -> Self {
        self.condition = Some(condition);
        self
    }

    /// Set a default value for the field (chainable)
    pub fn with_default_value(mut self, value: FormValue) -> Self {
        self.values = Some(vec![value]);
        self
    }

    /// Override the auto-generated label with a custom one (chainable)
    pub fn with_custom_label(mut self, label: String) -> Self {
        self.label = label;
        self
    }

    /// Set the span for responsive grid layout (chainable)
    /// Default is 12, valid range is 1-12
    pub fn with_span(mut self, span: u8) -> Self {
        self.span = span.clamp(1, 12);
        self
    }
}

use super::form_field::FormField;
use super::form_group::FormGroup;
use super::form_value::FormValue;
use std::time::Duration;

/// Utility functions for working with configuration data from Vec<FormGroup>
pub struct FormUtils;

impl FormUtils {
    /// Find a field by group key and field key
    pub fn find_field<'a>(config: &'a [FormGroup], group_key: &str, field_key: &str) -> Option<&'a FormField> {
        config
            .iter()
            .find(|group| group.key == group_key)?
            .fields
            .iter()
            .find(|field| field.key == field_key)
    }

    /// Find a mutable field by group key and field key
    pub fn find_field_mut<'a>(config: &'a mut [FormGroup], group_key: &str, field_key: &str) -> Option<&'a mut FormField> {
        config
            .iter_mut()
            .find(|group| group.key == group_key)?
            .fields
            .iter_mut()
            .find(|field| field.key == field_key)
    }

    pub fn find_group<'a>(config: &'a [FormGroup], group_key: &str) -> Option<&'a FormGroup> {
        config.iter().find(|group| group.key == group_key)
    }

    pub fn find_group_mut<'a>(config: &'a mut [FormGroup], group_key: &str) -> Option<&'a mut FormGroup> {
        config.iter_mut().find(|group| group.key == group_key)
    }

    /// Get the value of a field by group key and field key
    pub fn get_value<'a>(config: &'a [FormGroup], group_key: &str, field_key: &str) -> Option<&'a FormValue> {
        Self::find_field(config, group_key, field_key)?.get_value()
    }

    /// Get the multiple values of a field by group key and field key
    pub fn get_multiple_values<'a>(config: &'a [FormGroup], group_key: &str, field_key: &str) -> Option<&'a Vec<FormValue>> {
        Self::find_field(config, group_key, field_key)?.get_multiple_values()
    }

    /// Get text value by group key and field key
    pub fn get_text_value<'a>(config: &'a [FormGroup], group_key: &str, field_key: &str) -> Option<&'a str> {
        Self::find_field(config, group_key, field_key)?.get_text_value()
    }

    /// Get integer value by group key and field key
    pub fn get_integer_value(config: &[FormGroup], group_key: &str, field_key: &str) -> Option<i64> {
        Self::find_field(config, group_key, field_key)?.get_integer_value()
    }

    /// Get float value by group key and field key
    pub fn get_float_value(config: &[FormGroup], group_key: &str, field_key: &str) -> Option<f64> {
        Self::find_field(config, group_key, field_key)?.get_float_value()
    }

    /// Get boolean value by group key and field key
    pub fn get_boolean_value(config: &[FormGroup], group_key: &str, field_key: &str) -> Option<bool> {
        Self::find_field(config, group_key, field_key)?.get_boolean_value()
    }

    /// Get multiple text values by group key and field key
    pub fn get_multiple_text_values<'a>(config: &'a [FormGroup], group_key: &str, field_key: &str) -> Vec<&'a str> {
        Self::find_field(config, group_key, field_key)
            .map_or(Vec::new(), |field| field.get_multiple_text_values())
    }

    /// Get text list (owned strings) by group key and field key
    pub fn get_text_list_value(config: &[FormGroup], group_key: &str, field_key: &str) -> Vec<String> {
        Self::find_field(config, group_key, field_key)
            .map_or(Vec::new(), |field| field.get_multiple_text_values().into_iter().map(|s| s.to_string()).collect())
    }

    /// Get multiple integer values by group key and field key
    pub fn get_multiple_integer_values(config: &[FormGroup], group_key: &str, field_key: &str) -> Vec<i64> {
        Self::find_field(config, group_key, field_key)
            .map_or(Vec::new(), |field| field.get_multiple_integer_values())
    }

    /// Get multiple float values by group key and field key
    pub fn get_multiple_float_values(config: &[FormGroup], group_key: &str, field_key: &str) -> Vec<f64> {
        Self::find_field(config, group_key, field_key)
            .map_or(Vec::new(), |field| field.get_multiple_float_values())
    }

    /// Get multiple boolean values by group key and field key
    pub fn get_multiple_boolean_values(config: &[FormGroup], group_key: &str, field_key: &str) -> Vec<bool> {
        Self::find_field(config, group_key, field_key)
            .map_or(Vec::new(), |field| field.get_multiple_boolean_values())
    }

    /// Set the value of a field by group key and field key
    pub fn set_value(config: &mut [FormGroup], group_key: &str, field_key: &str, value: FormValue) -> Result<(), String> {
        match Self::find_field_mut(config, group_key, field_key) {
            Some(field) => {
                field.set_value(value);
                Ok(())
            }
            None => Err(format!("Field '{field_key}' not found in group '{group_key}'")),
        }
    }

    pub fn set_integer_value(config: &mut [FormGroup], group_key: &str, field_key: &str, value: i64) -> Result<(), String> {
        match Self::find_field_mut(config, group_key, field_key) {
            Some(field) => {
                field.set_integer_value(value);
                Ok(())
            }
            None => Err(format!("Field '{field_key}' not found in group '{group_key}'")),
        }
    }

    pub fn set_float_value(config: &mut [FormGroup], group_key: &str, field_key: &str, value: f64) -> Result<(), String> {
        match Self::find_field_mut(config, group_key, field_key) {
            Some(field) => {
                field.set_float_value(value);
                Ok(())
            }
            None => Err(format!("Field '{field_key}' not found in group '{group_key}'")),
        }
    }

    pub fn set_boolean_value(config: &mut [FormGroup], group_key: &str, field_key: &str, value: bool) -> Result<(), String> {
        match Self::find_field_mut(config, group_key, field_key) {
            Some(field) => {
                field.set_boolean_value(value);
                Ok(())
            }
            None => Err(format!("Field '{field_key}' not found in group '{group_key}'")),
        }
    }

    pub fn set_text_value(config: &mut [FormGroup], group_key: &str, field_key: &str, value: String) -> Result<(), String> {
        match Self::find_field_mut(config, group_key, field_key) {
            Some(field) => {
                field.set_text_value(value);
                Ok(())
            }
            None => Err(format!("Field '{field_key}' not found in group '{group_key}'")),
        }
    }

    pub fn set_multiple_values(config: &mut [FormGroup], group_key: &str, field_key: &str, values: Vec<FormValue>) -> Result<(), String> {
        match Self::find_field_mut(config, group_key, field_key) {
            Some(field) => {
                field.set_multiple_values(values);
                Ok(())
            }
            None => Err(format!("Field '{field_key}' not found in group '{group_key}'"))
        }
    }

    /// Append a value to the multiple values of a field by group key and field key
    pub fn append_value(config: &mut [FormGroup], group_key: &str, field_key: &str, value: FormValue) -> Result<(), String> {
        match Self::find_field_mut(config, group_key, field_key) {
            Some(field) => {
                field.append_value(value);
                Ok(())
            }
            None => Err(format!("Field '{field_key}' not found in group '{group_key}'")),
        }
    }

    /// Append multiple values to the multiple values of a field by group key and field key
    pub fn append_values(config: &mut [FormGroup], group_key: &str, field_key: &str, values: Vec<FormValue>) -> Result<(), String> {
        match Self::find_field_mut(config, group_key, field_key) {
            Some(field) => {
                field.append_values(values);
                Ok(())
            }
            None => Err(format!("Field '{field_key}' not found in group '{group_key}'")),
        }
    }

    /// Get all fields in a group by group key
    pub fn get_group_fields<'a>(config: &'a [FormGroup], group_key: &str) -> Option<&'a Vec<FormField>> {
        config
            .iter()
            .find(|group| group.key == group_key)
            .map(|group| &group.fields)
    }

    /// Get all field keys in a group by group key
    pub fn get_field_keys(config: &[FormGroup], group_key: &str) -> Vec<String> {
        Self::get_group_fields(config, group_key)
            .map(|fields| fields.iter().map(|field| field.key.clone()).collect())
            .unwrap_or_default()
    }

    /// Get all group keys
    pub fn get_group_keys(config: &[FormGroup]) -> Vec<String> {
        config.iter().map(|group| group.key.clone()).collect()
    }

    /// Get receive timeout from configuration
    pub fn get_receive_timeout(configs: &[FormGroup]) -> Duration {
        // Search all config groups for a field with key "with_receive_timeout"
        for group in configs {
            for field in &group.fields {
                if field.key == "with_receive_timeout"
                    && let Some(values) = &field.values
                {
                    for value in values {
                        if let FormValue::Integer(timeout_ms) = value {
                            return Duration::from_millis(*timeout_ms as u64);
                        }
                    }
                }
            }
        }
        Duration::from_millis(25)
    }
}

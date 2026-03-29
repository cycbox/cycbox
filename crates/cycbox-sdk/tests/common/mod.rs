#![allow(dead_code)]

use cycbox_sdk::{FieldType, FormField, FormGroup, FormValue, Manifest, PluginCategory};

/// Create a FormField with given key and type, default label, required.
pub fn make_form_field(key: &str, field_type: FieldType) -> FormField {
    FormField::new(key.to_string(), field_type)
}

/// Create a FormField with a pre-set value.
pub fn make_form_field_with_value(key: &str, field_type: FieldType, value: FormValue) -> FormField {
    let mut field = FormField::new(key.to_string(), field_type);
    field.set_value(value);
    field
}

/// Create a FormGroup with the given key and fields. Label = key, no condition.
pub fn make_form_group(key: &str, fields: Vec<FormField>) -> FormGroup {
    FormGroup {
        key: key.to_string(),
        label: key.to_string(),
        fields,
        description: None,
        condition: None,
    }
}

/// Create a Manifest with the given config_schema and sensible defaults.
pub fn make_manifest_with_schema(schema: Vec<FormGroup>) -> Manifest {
    Manifest {
        id: "test".to_string(),
        name: "Test Manifest".to_string(),
        description: String::new(),
        author: "test".to_string(),
        config_schema: schema,
        configs: vec![],
        category: PluginCategory::Transport,
        ..Default::default()
    }
}

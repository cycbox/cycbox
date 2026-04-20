mod common;

use common::{make_form_field, make_form_field_with_value, make_form_group, make_manifest_with_schema};
use cycbox_sdk::{FieldType, FormFieldOption, FormValue, ManifestValues};
use std::collections::HashMap;

// ---- Basic construction ----

#[test]
fn new_default() {
    let mv = ManifestValues::new();
    assert!(mv.configs.is_empty());
    assert!(mv.version.is_none());
    assert!(mv.name.is_none());
}

// ---- JSON round-trip ----

#[test]
fn from_json_str_empty() {
    let mv = ManifestValues::from_json_str("{}").unwrap();
    assert!(mv.configs.is_empty());
}

#[test]
fn json_roundtrip() {
    let mut mv = ManifestValues::new();
    mv.version = Some("1.0.0".into());
    mv.name = Some("Test".into());
    let mut group = HashMap::new();
    group.insert("host".into(), serde_json::json!("localhost"));
    mv.configs.push({
        let mut c = HashMap::new();
        c.insert("net".into(), group);
        c
    });

    let json = mv.to_json_string().unwrap();
    let back = ManifestValues::from_json_str(&json).unwrap();
    assert_eq!(back.version, mv.version);
    assert_eq!(back.name, mv.name);
    assert_eq!(back.configs.len(), 1);
}

#[test]
fn from_json_str_with_configs() {
    let json = r#"{"configs": [{"app": {"app_transport": "tcp_client_transport"}}]}"#;
    let mv = ManifestValues::from_json_str(json).unwrap();
    assert_eq!(mv.configs.len(), 1);
    assert_eq!(
        mv.configs[0]["app"]["app_transport"],
        serde_json::json!("tcp_client_transport")
    );
}

// ---- from_manifest filtering ----

#[test]
fn from_manifest_filters_by_app_selection() {
    let schema = vec![
        make_form_group(
            "app",
            vec![make_form_field_with_value(
                "app_transport",
                FieldType::TextDropdown,
                FormValue::Text("tcp_client_transport".into()),
            )],
        ),
        make_form_group(
            "tcp_client_transport",
            vec![make_form_field_with_value(
                "tcp_client_transport_host",
                FieldType::TextInput,
                FormValue::Text("localhost".into()),
            )],
        ),
        make_form_group(
            "serial_port_transport",
            vec![make_form_field_with_value(
                "serial_port_transport_port",
                FieldType::TextInput,
                FormValue::Text("/dev/ttyUSB0".into()),
            )],
        ),
    ];
    let mut manifest = make_manifest_with_schema(schema.clone());
    manifest.configs.push(schema);

    let mv = ManifestValues::from_manifest(&manifest);
    // Only app and tcp_client_transport should be present
    assert_eq!(mv.configs.len(), 1);
    let config = &mv.configs[0];
    assert!(config.contains_key("app"));
    assert!(config.contains_key("tcp_client_transport"));
    assert!(!config.contains_key("serial_port_transport"));
}

#[test]
fn from_manifest_removes_requires_codec() {
    let schema = vec![make_form_group(
        "app",
        vec![
            make_form_field_with_value(
                "app_transport",
                FieldType::TextDropdown,
                FormValue::Text("tcp".into()),
            ),
            make_form_field_with_value(
                "app_transport_tcp_requires_codec",
                FieldType::BooleanInput,
                FormValue::Boolean(true),
            ),
        ],
    )];
    let mut manifest = make_manifest_with_schema(schema.clone());
    manifest.configs.push(schema);

    let mv = ManifestValues::from_manifest(&manifest);
    if !mv.configs.is_empty() {
        if let Some(app) = mv.configs[0].get("app") {
            assert!(!app.contains_key("app_transport_tcp_requires_codec"));
        }
    }
}

#[test]
fn from_manifest_empty_groups_excluded() {
    let schema = vec![
        make_form_group(
            "app",
            vec![make_form_field_with_value(
                "app_transport",
                FieldType::TextDropdown,
                FormValue::Text("tcp".into()),
            )],
        ),
        make_form_group("tcp", vec![make_form_field("empty_field", FieldType::TextInput)]),
    ];
    let mut manifest = make_manifest_with_schema(schema.clone());
    manifest.configs.push(schema);

    let mv = ManifestValues::from_manifest(&manifest);
    // tcp group has no values → excluded
    if !mv.configs.is_empty() {
        assert!(!mv.configs[0].contains_key("tcp"));
    }
}

// ---- merge_into_manifest ----

#[test]
fn merge_into_manifest_applies_values() {
    let schema = vec![make_form_group(
        "net",
        vec![make_form_field("host", FieldType::TextInput)],
    )];
    let manifest = make_manifest_with_schema(schema);

    let mut mv = ManifestValues::new();
    let mut group = HashMap::new();
    group.insert("host".into(), serde_json::json!("example.com"));
    mv.configs.push({
        let mut c = HashMap::new();
        c.insert("net".into(), group);
        c
    });

    let merged = mv.merge_into_manifest(manifest);
    assert_eq!(merged.configs.len(), 1);
    let field = &merged.configs[0][0].fields[0];
    assert_eq!(field.get_text_value(), Some("example.com"));
}

#[test]
fn merge_type_mismatch_skipped() {
    let schema = vec![make_form_group(
        "net",
        vec![make_form_field_with_value(
            "host",
            FieldType::TextInput,
            FormValue::Text("original".into()),
        )],
    )];
    let manifest = make_manifest_with_schema(schema);

    let mut mv = ManifestValues::new();
    let mut group = HashMap::new();
    // Boolean value for a text field → should be skipped
    group.insert("host".into(), serde_json::json!(true));
    mv.configs.push({
        let mut c = HashMap::new();
        c.insert("net".into(), group);
        c
    });

    let merged = mv.merge_into_manifest(manifest);
    // Field keeps original value from schema template (type mismatch is skipped)
    let field = &merged.configs[0][0].fields[0];
    assert_eq!(field.get_text_value(), Some("original"));
}

#[test]
fn merge_metadata_override() {
    let manifest = make_manifest_with_schema(vec![]);
    let mut mv = ManifestValues::new();
    mv.version = Some("2.0.0".into());
    mv.name = Some("Override".into());
    mv.description = Some("Desc".into());

    let merged = mv.merge_into_manifest(manifest);
    assert_eq!(merged.version, "2.0.0");
    assert_eq!(merged.name, "Override");
    assert_eq!(merged.description, "Desc");
}

// ---- validate ----

#[test]
fn validate_ok() {
    let schema = vec![make_form_group(
        "net",
        vec![make_form_field("host", FieldType::TextInput)],
    )];
    let manifest = make_manifest_with_schema(schema);

    let mut mv = ManifestValues::new();
    let mut group = HashMap::new();
    group.insert("host".into(), serde_json::json!("localhost"));
    mv.configs.push({
        let mut c = HashMap::new();
        c.insert("net".into(), group);
        c
    });

    assert!(mv.validate(&manifest).is_ok());
}

#[test]
fn validate_unknown_group() {
    let manifest = make_manifest_with_schema(vec![]);
    let mut mv = ManifestValues::new();
    let mut group = HashMap::new();
    group.insert("host".into(), serde_json::json!("x"));
    mv.configs.push({
        let mut c = HashMap::new();
        c.insert("unknown".into(), group);
        c
    });

    let err = mv.validate(&manifest).unwrap_err();
    assert!(err[0].message.contains("Unknown group"));
}

#[test]
fn validate_unknown_field() {
    let schema = vec![make_form_group(
        "net",
        vec![make_form_field("host", FieldType::TextInput)],
    )];
    let manifest = make_manifest_with_schema(schema);

    let mut mv = ManifestValues::new();
    let mut group = HashMap::new();
    group.insert("nonexistent".into(), serde_json::json!("x"));
    mv.configs.push({
        let mut c = HashMap::new();
        c.insert("net".into(), group);
        c
    });

    let err = mv.validate(&manifest).unwrap_err();
    assert!(err[0].message.contains("Unknown field"));
}

#[test]
fn validate_type_mismatch() {
    let schema = vec![make_form_group(
        "net",
        vec![make_form_field("host", FieldType::TextInput)],
    )];
    let manifest = make_manifest_with_schema(schema);

    let mut mv = ManifestValues::new();
    let mut group = HashMap::new();
    group.insert("host".into(), serde_json::json!(42)); // integer for text field
    mv.configs.push({
        let mut c = HashMap::new();
        c.insert("net".into(), group);
        c
    });

    let err = mv.validate(&manifest).unwrap_err();
    assert!(err[0].message.contains("Expected string"));
}

#[test]
fn validate_invalid_option() {
    let field = make_form_field("transport", FieldType::TextDropdown).with_options(vec![
        FormFieldOption::new("TCP".into(), FormValue::Text("tcp".into())),
        FormFieldOption::new("UDP".into(), FormValue::Text("udp".into())),
    ]);
    let schema = vec![make_form_group("app", vec![field])];
    let manifest = make_manifest_with_schema(schema);

    let mut mv = ManifestValues::new();
    let mut group = HashMap::new();
    group.insert("transport".into(), serde_json::json!("serial")); // not in options
    mv.configs.push({
        let mut c = HashMap::new();
        c.insert("app".into(), group);
        c
    });

    let err = mv.validate(&manifest).unwrap_err();
    assert!(err[0].message.contains("not in allowed options"));
}

#[test]
fn validate_input_dropdown_accepts_any() {
    let field = make_form_field("transport", FieldType::TextInputDropdown).with_options(vec![
        FormFieldOption::new("TCP".into(), FormValue::Text("tcp".into())),
    ]);
    let schema = vec![make_form_group("app", vec![field])];
    let manifest = make_manifest_with_schema(schema);

    let mut mv = ManifestValues::new();
    let mut group = HashMap::new();
    group.insert("transport".into(), serde_json::json!("custom_value"));
    mv.configs.push({
        let mut c = HashMap::new();
        c.insert("app".into(), group);
        c
    });

    assert!(mv.validate(&manifest).is_ok());
}

// ---- Transport key migration ----

#[test]
fn transport_key_migration_group() {
    let json = r#"{"configs": [{"tcp_client": {"tcp_client_host": "localhost"}}]}"#;
    let mv = ManifestValues::from_json_str(json).unwrap();
    assert!(mv.configs[0].contains_key("tcp_client_transport"));
    assert!(!mv.configs[0].contains_key("tcp_client"));
}

#[test]
fn transport_key_migration_field_keys() {
    let json = r#"{"configs": [{"tcp_client": {"tcp_client_host": "localhost"}}]}"#;
    let mv = ManifestValues::from_json_str(json).unwrap();
    let group = &mv.configs[0]["tcp_client_transport"];
    assert!(group.contains_key("tcp_client_transport_host"));
    assert!(!group.contains_key("tcp_client_host"));
}

#[test]
fn transport_key_migration_app_transport_value() {
    let json = r#"{"configs": [{"app": {"app_transport": "tcp_client"}}]}"#;
    let mv = ManifestValues::from_json_str(json).unwrap();
    assert_eq!(
        mv.configs[0]["app"]["app_transport"],
        serde_json::json!("tcp_client_transport")
    );
}

// ---- Lua string round-trip ----

#[test]
fn from_lua_str_with_script() {
    let lua = r#"print("hello")
--[[
{"name": "test"}
]]"#;
    let mv = ManifestValues::from_lua_str(lua).unwrap();
    assert_eq!(mv.lua_script, Some("print(\"hello\")".to_string()));
    assert_eq!(mv.name, Some("test".into()));
}

#[test]
fn from_lua_str_no_lua_code() {
    let lua = r#"--[[
{"name": "test"}
]]"#;
    let mv = ManifestValues::from_lua_str(lua).unwrap();
    assert_eq!(mv.lua_script, None);
}

#[test]
fn from_lua_str_missing_block() {
    let result = ManifestValues::from_lua_str("no block here");
    assert!(result.is_err());
}

#[test]
fn lua_str_roundtrip() {
    let mut mv = ManifestValues::new();
    mv.name = Some("test".into());
    mv.lua_script = Some("print('hi')".into());

    let lua_str = mv.to_lua_str().unwrap();
    let back = ManifestValues::from_lua_str(&lua_str).unwrap();
    assert_eq!(back.name, Some("test".into()));
    assert_eq!(back.lua_script, Some("print('hi')".into()));
}

// ---- Dashboard ----

#[test]
fn from_manifest_dashboards_with_widgets() {
    let mut manifest = make_manifest_with_schema(vec![]);
    manifest.dashboards = vec![serde_json::json!({"widgets": [{"type": "line_chart"}]})];

    let mv = ManifestValues::from_manifest(&manifest);
    assert_eq!(mv.dashboards.len(), 1);
}

#[test]
fn from_manifest_dashboards_empty_widgets() {
    let mut manifest = make_manifest_with_schema(vec![]);
    manifest.dashboards = vec![serde_json::json!({"widgets": []})];

    let mv = ManifestValues::from_manifest(&manifest);
    assert!(mv.dashboards.is_empty());
}

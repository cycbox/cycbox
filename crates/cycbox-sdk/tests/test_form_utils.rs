mod common;

use common::{make_form_field, make_form_field_with_value, make_form_group};
use cycbox_sdk::{FieldType, FormUtils, FormValue};
use std::time::Duration;

fn sample_config() -> Vec<cycbox_sdk::FormGroup> {
    vec![
        make_form_group(
            "net",
            vec![
                make_form_field_with_value("host", FieldType::TextInput, FormValue::Text("localhost".into())),
                make_form_field_with_value("port", FieldType::IntegerInput, FormValue::Integer(8080)),
                make_form_field_with_value("tls", FieldType::BooleanInput, FormValue::Boolean(true)),
                make_form_field_with_value("rate", FieldType::FloatInput, FormValue::Float(1.5)),
            ],
        ),
        make_form_group(
            "app",
            vec![make_form_field("name", FieldType::TextInput)],
        ),
    ]
}

// ---- find ----

#[test]
fn find_field_exists() {
    let cfg = sample_config();
    assert!(FormUtils::find_field(&cfg, "net", "host").is_some());
}

#[test]
fn find_field_missing_group() {
    let cfg = sample_config();
    assert!(FormUtils::find_field(&cfg, "none", "host").is_none());
}

#[test]
fn find_field_missing_key() {
    let cfg = sample_config();
    assert!(FormUtils::find_field(&cfg, "net", "none").is_none());
}

#[test]
fn find_group_exists() {
    let cfg = sample_config();
    assert!(FormUtils::find_group(&cfg, "net").is_some());
}

#[test]
fn find_group_missing() {
    let cfg = sample_config();
    assert!(FormUtils::find_group(&cfg, "none").is_none());
}

// ---- Typed getters ----

#[test]
fn get_text_value() {
    let cfg = sample_config();
    assert_eq!(FormUtils::get_text_value(&cfg, "net", "host"), Some("localhost"));
}

#[test]
fn get_integer_value() {
    let cfg = sample_config();
    assert_eq!(FormUtils::get_integer_value(&cfg, "net", "port"), Some(8080));
}

#[test]
fn get_float_value() {
    let cfg = sample_config();
    assert_eq!(FormUtils::get_float_value(&cfg, "net", "rate"), Some(1.5));
}

#[test]
fn get_boolean_value() {
    let cfg = sample_config();
    assert_eq!(FormUtils::get_boolean_value(&cfg, "net", "tls"), Some(true));
}

// ---- Setters ----

#[test]
fn set_value_ok() {
    let mut cfg = sample_config();
    assert!(FormUtils::set_value(&mut cfg, "net", "host", FormValue::Text("example.com".into())).is_ok());
    assert_eq!(FormUtils::get_text_value(&cfg, "net", "host"), Some("example.com"));
}

#[test]
fn set_value_missing_field() {
    let mut cfg = sample_config();
    assert!(FormUtils::set_value(&mut cfg, "net", "none", FormValue::Text("x".into())).is_err());
}

#[test]
fn set_integer_value() {
    let mut cfg = sample_config();
    assert!(FormUtils::set_integer_value(&mut cfg, "net", "port", 9090).is_ok());
    assert_eq!(FormUtils::get_integer_value(&cfg, "net", "port"), Some(9090));
}

#[test]
fn set_float_value() {
    let mut cfg = sample_config();
    assert!(FormUtils::set_float_value(&mut cfg, "net", "rate", 2.0).is_ok());
    assert_eq!(FormUtils::get_float_value(&cfg, "net", "rate"), Some(2.0));
}

#[test]
fn set_boolean_value() {
    let mut cfg = sample_config();
    assert!(FormUtils::set_boolean_value(&mut cfg, "net", "tls", false).is_ok());
    assert_eq!(FormUtils::get_boolean_value(&cfg, "net", "tls"), Some(false));
}

#[test]
fn set_text_value() {
    let mut cfg = sample_config();
    assert!(FormUtils::set_text_value(&mut cfg, "net", "host", "new".into()).is_ok());
    assert_eq!(FormUtils::get_text_value(&cfg, "net", "host"), Some("new"));
}

// ---- Multiple values ----

#[test]
fn append_value() {
    let mut cfg = sample_config();
    assert!(FormUtils::append_value(&mut cfg, "net", "host", FormValue::Text("b".into())).is_ok());
    let vals = FormUtils::get_multiple_text_values(&cfg, "net", "host");
    assert_eq!(vals, vec!["localhost", "b"]);
}

#[test]
fn append_values() {
    let mut cfg = sample_config();
    assert!(FormUtils::append_values(
        &mut cfg,
        "net",
        "host",
        vec![FormValue::Text("b".into()), FormValue::Text("c".into())]
    )
    .is_ok());
    let vals = FormUtils::get_multiple_text_values(&cfg, "net", "host");
    assert_eq!(vals, vec!["localhost", "b", "c"]);
}

#[test]
fn set_multiple_values() {
    let mut cfg = sample_config();
    assert!(FormUtils::set_multiple_values(
        &mut cfg,
        "net",
        "host",
        vec![FormValue::Text("a".into()), FormValue::Text("b".into())]
    )
    .is_ok());
    let vals = FormUtils::get_multiple_text_values(&cfg, "net", "host");
    assert_eq!(vals, vec!["a", "b"]);
}

#[test]
fn get_multiple_integer_values() {
    let mut cfg = sample_config();
    FormUtils::append_value(&mut cfg, "net", "port", FormValue::Integer(9090)).unwrap();
    let vals = FormUtils::get_multiple_integer_values(&cfg, "net", "port");
    assert_eq!(vals, vec![8080, 9090]);
}

// ---- Group/field key lists ----

#[test]
fn get_group_fields() {
    let cfg = sample_config();
    let fields = FormUtils::get_group_fields(&cfg, "net").unwrap();
    assert_eq!(fields.len(), 4);
}

#[test]
fn get_field_keys() {
    let cfg = sample_config();
    let keys = FormUtils::get_field_keys(&cfg, "net");
    assert!(keys.contains(&"host".to_string()));
    assert!(keys.contains(&"port".to_string()));
}

#[test]
fn get_group_keys() {
    let cfg = sample_config();
    let keys = FormUtils::get_group_keys(&cfg);
    assert_eq!(keys, vec!["net".to_string(), "app".to_string()]);
}

// ---- Receive timeout ----

#[test]
fn get_receive_timeout_found() {
    let cfg = vec![make_form_group(
        "codec",
        vec![make_form_field_with_value(
            "with_receive_timeout",
            FieldType::IntegerInput,
            FormValue::Integer(100),
        )],
    )];
    assert_eq!(FormUtils::get_receive_timeout(&cfg), Duration::from_millis(100));
}

#[test]
fn get_receive_timeout_default() {
    let cfg = sample_config();
    assert_eq!(FormUtils::get_receive_timeout(&cfg), Duration::from_millis(25));
}

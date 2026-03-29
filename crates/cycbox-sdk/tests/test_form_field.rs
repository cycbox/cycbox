use cycbox_sdk::{ConditionOperator, FieldType, FormCondition, FormField, FormFieldOption, FormValue};

// ---- Constructor defaults ----

#[test]
fn new_field_defaults() {
    let f = FormField::new("my_key".to_string(), FieldType::TextInput);
    assert_eq!(f.label, "my-key-label");
    assert!(f.is_required);
    assert!(f.values.is_none());
    assert_eq!(f.span, 12);
    assert!(f.description.is_none());
    assert!(f.condition.is_none());
}

#[test]
fn with_description_constructor() {
    let f = FormField::with_description("my_key".to_string(), FieldType::TextInput);
    assert_eq!(f.description, Some("my-key-description".to_string()));
}

// ---- get/set typed values ----

#[test]
fn set_get_text_value() {
    let mut f = FormField::new("k".into(), FieldType::TextInput);
    f.set_text_value("hello".into());
    assert_eq!(f.get_text_value(), Some("hello"));
}

#[test]
fn set_get_integer_value() {
    let mut f = FormField::new("k".into(), FieldType::IntegerInput);
    f.set_integer_value(42);
    assert_eq!(f.get_integer_value(), Some(42));
}

#[test]
fn set_get_float_value() {
    let mut f = FormField::new("k".into(), FieldType::FloatInput);
    f.set_float_value(3.14);
    assert_eq!(f.get_float_value(), Some(3.14));
}

#[test]
fn set_get_boolean_value() {
    let mut f = FormField::new("k".into(), FieldType::BooleanInput);
    f.set_boolean_value(true);
    assert_eq!(f.get_boolean_value(), Some(true));
}

#[test]
fn get_value_none_when_empty() {
    let f = FormField::new("k".into(), FieldType::TextInput);
    assert_eq!(f.get_text_value(), None);
}

#[test]
fn type_mismatch_getter() {
    let mut f = FormField::new("k".into(), FieldType::IntegerInput);
    f.set_integer_value(42);
    assert_eq!(f.get_text_value(), None);
}

// ---- clear ----

#[test]
fn clear_value() {
    let mut f = FormField::new("k".into(), FieldType::TextInput);
    f.set_text_value("hello".into());
    f.clear_value();
    assert!(f.get_value().is_none());
}

// ---- Multiple values ----

#[test]
fn append_value_from_empty() {
    let mut f = FormField::new("k".into(), FieldType::TextInput);
    f.append_value(FormValue::Text("a".into()));
    assert_eq!(f.get_multiple_text_values(), vec!["a"]);
}

#[test]
fn append_value_accumulates() {
    let mut f = FormField::new("k".into(), FieldType::TextInput);
    f.append_value(FormValue::Text("a".into()));
    f.append_value(FormValue::Text("b".into()));
    assert_eq!(f.get_multiple_text_values(), vec!["a", "b"]);
}

#[test]
fn append_values_batch() {
    let mut f = FormField::new("k".into(), FieldType::TextInput);
    f.set_text_value("a".into());
    f.append_values(vec![
        FormValue::Text("b".into()),
        FormValue::Text("c".into()),
    ]);
    assert_eq!(f.get_multiple_text_values(), vec!["a", "b", "c"]);
}

#[test]
fn get_multiple_integer_values() {
    let mut f = FormField::new("k".into(), FieldType::IntegerInput);
    f.append_value(FormValue::Integer(1));
    f.append_value(FormValue::Integer(2));
    assert_eq!(f.get_multiple_integer_values(), vec![1, 2]);
}

#[test]
fn get_multiple_float_values() {
    let mut f = FormField::new("k".into(), FieldType::FloatInput);
    f.append_value(FormValue::Float(1.0));
    f.append_value(FormValue::Float(2.5));
    assert_eq!(f.get_multiple_float_values(), vec![1.0, 2.5]);
}

#[test]
fn get_multiple_boolean_values() {
    let mut f = FormField::new("k".into(), FieldType::BooleanInput);
    f.append_value(FormValue::Boolean(true));
    f.append_value(FormValue::Boolean(false));
    assert_eq!(f.get_multiple_boolean_values(), vec![true, false]);
}

#[test]
fn clear_multiple_values() {
    let mut f = FormField::new("k".into(), FieldType::TextInput);
    f.append_value(FormValue::Text("a".into()));
    f.clear_multiple_values();
    assert!(f.get_multiple_values().is_none());
}

// ---- Builder chain ----

#[test]
fn builder_chain_with_options() {
    let f = FormField::new("k".into(), FieldType::TextDropdown)
        .with_options(vec![FormFieldOption::new(
            "Opt".into(),
            FormValue::Text("opt".into()),
        )])
        .required(false)
        .with_span(6);
    assert!(!f.is_required);
    assert_eq!(f.span, 6);
    assert!(f.options.is_some());
}

#[test]
fn with_condition() {
    let f = FormField::new("k".into(), FieldType::TextInput).with_condition(FormCondition {
        field_key: "other".into(),
        operator: ConditionOperator::Equal,
        value: FormValue::Text("x".into()),
    });
    assert!(f.condition.is_some());
}

#[test]
fn with_default_value() {
    let f = FormField::new("k".into(), FieldType::TextInput)
        .with_default_value(FormValue::Text("default".into()));
    assert_eq!(f.get_text_value(), Some("default"));
}

#[test]
fn with_custom_label() {
    let f =
        FormField::new("k".into(), FieldType::TextInput).with_custom_label("Custom".to_string());
    assert_eq!(f.label, "Custom");
}

#[test]
fn with_span_clamping() {
    let f1 = FormField::new("k".into(), FieldType::TextInput).with_span(0);
    assert_eq!(f1.span, 1);
    let f2 = FormField::new("k".into(), FieldType::TextInput).with_span(20);
    assert_eq!(f2.span, 12);
}

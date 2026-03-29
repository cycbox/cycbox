use cycbox_sdk::message_input::{parse_hex_string, text_to_bytes, MessageInput, MessageInputRegistry};

// ---- parse_hex_string ----

#[test]
fn parse_hex_string_spaced() {
    let bytes = parse_hex_string("AA BB CC").unwrap();
    assert_eq!(bytes, vec![0xAA, 0xBB, 0xCC]);
}

#[test]
fn parse_hex_string_no_spaces() {
    let bytes = parse_hex_string("AABBCC").unwrap();
    assert_eq!(bytes, vec![0xAA, 0xBB, 0xCC]);
}

#[test]
fn parse_hex_string_mixed_case() {
    let bytes = parse_hex_string("aAbBcC").unwrap();
    assert_eq!(bytes, vec![0xAA, 0xBB, 0xCC]);
}

#[test]
fn parse_hex_string_empty() {
    let bytes = parse_hex_string("").unwrap();
    assert!(bytes.is_empty());
}

#[test]
fn parse_hex_string_invalid_char() {
    assert!(parse_hex_string("GG").is_err());
}

#[test]
fn parse_hex_string_odd_length() {
    // After bug fix, odd-length hex strings should error
    assert!(parse_hex_string("AAB").is_err());
}

// ---- text_to_bytes ----

#[test]
fn text_to_bytes_text_mode() {
    let bytes = text_to_bytes("hello", false).unwrap();
    assert_eq!(bytes, b"hello");
}

#[test]
fn text_to_bytes_hex_mode() {
    let bytes = text_to_bytes("AA BB", true).unwrap();
    assert_eq!(bytes, vec![0xAA, 0xBB]);
}

// ---- SimpleMessageInputConverter via registry ----

#[test]
fn registry_default_has_simple() {
    let registry = MessageInputRegistry::new();
    let json = serde_json::json!({
        "input_type": "simple",
        "id": "1",
        "name": "test",
        "connection_id": 0,
        "raw_value": "hello",
        "is_hex": false
    });
    let msgs = registry.convert(&json).unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].payload, b"hello");
    assert_eq!(msgs[0].message_type, "tx");
}

#[test]
fn simple_converter_hex() {
    let registry = MessageInputRegistry::new();
    let json = serde_json::json!({
        "input_type": "simple",
        "id": "1",
        "name": "test",
        "connection_id": 0,
        "raw_value": "AA BB",
        "is_hex": true
    });
    let msgs = registry.convert(&json).unwrap();
    assert_eq!(msgs[0].payload, vec![0xAA, 0xBB]);
}

#[test]
fn registry_unknown_type() {
    let registry = MessageInputRegistry::new();
    let json = serde_json::json!({"input_type": "mqtt"});
    let err = registry.convert(&json).unwrap_err();
    assert!(err.to_string().contains("mqtt"));
}

#[test]
fn registry_missing_input_type() {
    let registry = MessageInputRegistry::new();
    let json = serde_json::json!({"value": "hello"});
    assert!(registry.convert(&json).is_err());
}

#[test]
fn registry_batch_conversion() {
    let registry = MessageInputRegistry::new();
    let json = serde_json::json!({
        "input_type": "batch",
        "id": "b1",
        "name": "batch test",
        "repeat": false,
        "items": [
            {
                "message_input": {
                    "input_type": "simple",
                    "id": "1",
                    "name": "first",
                    "connection_id": 0,
                    "raw_value": "AA",
                    "is_hex": true
                },
                "delay_ms": 10.0
            },
            {
                "message_input": {
                    "input_type": "simple",
                    "id": "2",
                    "name": "second",
                    "connection_id": 0,
                    "raw_value": "BB",
                    "is_hex": true
                },
                "delay_ms": 20.0
            }
        ]
    });
    let msgs = registry.convert(&json).unwrap();
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].payload, vec![0xAA]);
    assert_eq!(msgs[1].payload, vec![0xBB]);
    // Both should have delay_us metadata
    assert!(msgs[0].metadata.iter().any(|v| v.id == "delay_us"));
    assert!(msgs[1].metadata.iter().any(|v| v.id == "delay_us"));
}

// ---- MessageInput wrapper ----

#[test]
fn message_input_wrapper() {
    let json = serde_json::json!({
        "input_type": "simple",
        "id": "msg1",
        "name": "My Message",
        "connection_id": 5
    });
    let input: MessageInput = serde_json::from_value(json).unwrap();
    assert_eq!(input.input_type(), "simple");
    assert_eq!(input.id(), Some("msg1"));
    assert_eq!(input.name(), Some("My Message"));
    assert_eq!(input.connection_id(), Some(5));
}

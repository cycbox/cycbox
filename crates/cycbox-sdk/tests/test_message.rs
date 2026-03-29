use cycbox_sdk::{
    Content, Message, MessageBuilder, PayloadType, Value,
    MESSAGE_TYPE_EVENT, MESSAGE_TYPE_REQUEST, MESSAGE_TYPE_RESPONSE, MESSAGE_TYPE_RX,
    MESSAGE_TYPE_TX,
};
use std::time::Duration;

const SYSTEM_CONNECTION_ID: u32 = 9999;
const UNKNOW_CONNECTION_ID: u32 = 9998;

// ---- Builder constructors ----

#[test]
fn builder_event() {
    let msg = MessageBuilder::event("my_event").build();
    assert_eq!(msg.message_type, MESSAGE_TYPE_EVENT);
    assert_eq!(msg.frame, b"my_event");
    assert_eq!(msg.connection_id, SYSTEM_CONNECTION_ID);
}

#[test]
fn builder_tx() {
    let msg = MessageBuilder::tx(1, PayloadType::Binary, vec![0xAA], vec![0xBB, 0xAA]).build();
    assert_eq!(msg.connection_id, 1);
    assert_eq!(msg.message_type, MESSAGE_TYPE_TX);
    assert_eq!(msg.payload, vec![0xAA]);
    assert_eq!(msg.frame, vec![0xBB, 0xAA]);
}

#[test]
fn builder_rx() {
    let msg = MessageBuilder::rx(PayloadType::Text, b"hello".to_vec(), b"frame".to_vec()).build();
    assert_eq!(msg.connection_id, UNKNOW_CONNECTION_ID);
    assert_eq!(msg.message_type, MESSAGE_TYPE_RX);
}

#[test]
fn builder_request() {
    let msg = MessageBuilder::request(42, "set_highlight", 1000, 5000).build();
    assert_eq!(msg.message_type, MESSAGE_TYPE_REQUEST);
    assert_eq!(msg.get_seq_id(), 42);
    assert_eq!(msg.get_command(), "set_highlight");
    assert_eq!(msg.timestamp, 1000);
}

#[test]
fn builder_response_success() {
    let req = MessageBuilder::request(42, "cmd", 1000, 5000).build();
    let resp = MessageBuilder::response_success(&req).build();
    assert_eq!(resp.message_type, MESSAGE_TYPE_RESPONSE);
    assert!(resp.is_success());
    assert_eq!(resp.error_message(), None);
    assert_eq!(resp.get_seq_id(), 42);
    assert_eq!(resp.get_command(), "cmd");
}

#[test]
fn builder_response_error() {
    let req = MessageBuilder::request(42, "cmd", 1000, 5000).build();
    let resp = MessageBuilder::response_error(&req, "timeout").build();
    assert!(!resp.is_success());
    assert_eq!(resp.error_message(), Some("timeout".to_string()));
}

// ---- seq_id ----

#[test]
fn seq_id_encoding_max() {
    let mut msg = MessageBuilder::new().build();
    msg.set_seq_id(u64::MAX);
    assert_eq!(msg.get_seq_id(), u64::MAX);
}

#[test]
fn seq_id_encoding_zero() {
    let mut msg = MessageBuilder::new().build();
    msg.set_seq_id(0);
    assert_eq!(msg.get_seq_id(), 0);
}

#[test]
fn seq_id_empty_payload() {
    let msg = MessageBuilder::new().build();
    // Empty payload → get_seq_id returns 0
    assert_eq!(msg.get_seq_id(), 0);
}

// ---- Metadata helpers ----

#[test]
fn timeout_from_metadata() {
    let msg = MessageBuilder::new()
        .add_metadata(Value::builder("timeout_ms").uint32(500))
        .build();
    assert_eq!(msg.timeout(), Some(Duration::from_millis(500)));
}

#[test]
fn timeout_absent() {
    let msg = MessageBuilder::new().build();
    assert_eq!(msg.timeout(), None);
}

#[test]
fn param_lookup() {
    let msg = MessageBuilder::new()
        .add_value(Value::builder("x").uint32(1))
        .add_value(Value::builder("y").uint32(2))
        .build();
    assert_eq!(msg.param("x").unwrap().as_u32(), Some(1));
    assert_eq!(msg.param("y").unwrap().as_u32(), Some(2));
    assert!(msg.param("z").is_none());
}

#[test]
fn metadata_value_lookup() {
    let msg = MessageBuilder::new()
        .add_metadata(Value::builder("topic").string("test/topic"))
        .build();
    assert_eq!(
        msg.metadata_value("topic").unwrap().as_string(),
        Some("test/topic".to_string())
    );
    assert!(msg.metadata_value("nonexistent").is_none());
}

// ---- Timestamps ----

#[test]
fn current_timestamp_reasonable() {
    let ts = Message::current_timestamp();
    // Should be after 2020-01-01 in microseconds
    assert!(ts > 1_577_836_800_000_000);
}

#[test]
fn builder_auto_timestamp() {
    let msg = MessageBuilder::new().message_type("test").build();
    assert!(msg.timestamp > 0);
}

#[test]
fn builder_explicit_timestamp() {
    let msg = MessageBuilder::new().timestamp(999).build();
    assert_eq!(msg.timestamp, 999);
}

// ---- Frame defaults to payload ----

#[test]
fn builder_frame_defaults_to_payload() {
    let msg = MessageBuilder::new()
        .payload(PayloadType::Binary, vec![1, 2, 3])
        .build();
    // When frame is empty, build() copies payload to frame
    assert_eq!(msg.frame, vec![1, 2, 3]);
}

// ---- Fluent chaining ----

#[test]
fn builder_fluent_chaining() {
    let msg = MessageBuilder::new()
        .connection_id(5)
        .message_type("test")
        .add_content(Content::plain(b"hello"))
        .add_value(Value::builder("v").uint8(1))
        .add_metadata(Value::builder("m").boolean(true))
        .highlighted(true)
        .display_hex(true)
        .add_hex_content(Content::plain(b"FF"))
        .build();
    assert_eq!(msg.connection_id, 5);
    assert!(msg.highlighted);
    assert!(msg.display_hex);
    assert_eq!(msg.contents.len(), 1);
    assert_eq!(msg.values.len(), 1);
    assert_eq!(msg.metadata.len(), 1);
    assert_eq!(msg.hex_contents.len(), 1);
}

// ---- seq_id builder method ----

#[test]
fn builder_seq_id_method() {
    let msg = MessageBuilder::new().seq_id(123).build();
    assert_eq!(msg.get_seq_id(), 123);
}

// ---- set/get command ----

#[test]
fn set_get_command() {
    let mut msg = MessageBuilder::new().build();
    msg.set_command("foo");
    assert_eq!(msg.get_command(), "foo");
}

// ---- refresh_timestamp ----

#[test]
fn refresh_timestamp() {
    let mut msg = MessageBuilder::new().timestamp(1).build();
    assert_eq!(msg.timestamp, 1);
    msg.refresh_timestamp();
    assert!(msg.timestamp > 1);
}

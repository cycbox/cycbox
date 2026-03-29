use async_trait::async_trait;
use cycbox_sdk::lua::LuaFunctionRegistrar;
use cycbox_sdk::message::lua_functions::MessageLuaHelper;
use cycbox_sdk::Message;
use std::sync::Arc;

struct MockLuaEngine;

#[async_trait]
impl cycbox_sdk::lua::LuaEngine for MockLuaEngine {
    async fn send_message(&self, _message: Message) {}
    fn debug(&self, _message: &str) {}
    fn info(&self, _message: &str) {}
    fn warn(&self, _message: &str) {}
    fn error(&self, _message: &str) {}
}

fn setup_lua() -> mlua::Lua {
    let lua = mlua::Lua::new();
    let engine: Arc<dyn cycbox_sdk::lua::LuaEngine> = Arc::new(MockLuaEngine);
    MessageLuaHelper.register(&lua, engine).unwrap();
    lua
}

// ---- read_u8 / read_i8 ----

#[test]
fn read_u8() {
    let lua = setup_lua();
    let result: i64 = lua
        .load("return read_u8(\"\\xff\", 1)")
        .eval()
        .unwrap();
    assert_eq!(result, 255);
}

#[test]
fn read_i8() {
    let lua = setup_lua();
    let result: i64 = lua
        .load("return read_i8(\"\\x80\", 1)")
        .eval()
        .unwrap();
    assert_eq!(result, -128);
}

// ---- read_u16 ----

#[test]
fn read_u16_be() {
    let lua = setup_lua();
    let result: i64 = lua
        .load("return read_u16_be(\"\\x01\\x00\", 1)")
        .eval()
        .unwrap();
    assert_eq!(result, 256);
}

#[test]
fn read_u16_le() {
    let lua = setup_lua();
    let result: i64 = lua
        .load("return read_u16_le(\"\\x00\\x01\", 1)")
        .eval()
        .unwrap();
    assert_eq!(result, 256);
}

// ---- read_i16 ----

#[test]
fn read_i16_be() {
    let lua = setup_lua();
    let result: i64 = lua
        .load("return read_i16_be(\"\\xff\\xfe\", 1)")
        .eval()
        .unwrap();
    assert_eq!(result, -2);
}

#[test]
fn read_i16_le() {
    let lua = setup_lua();
    let result: i64 = lua
        .load("return read_i16_le(\"\\xfe\\xff\", 1)")
        .eval()
        .unwrap();
    assert_eq!(result, -2);
}

// ---- read_u32 / read_i32 ----

#[test]
fn read_u32_be() {
    let lua = setup_lua();
    let result: i64 = lua
        .load("return read_u32_be(\"\\x00\\x00\\x01\\x00\", 1)")
        .eval()
        .unwrap();
    assert_eq!(result, 256);
}

#[test]
fn read_u32_le() {
    let lua = setup_lua();
    let result: i64 = lua
        .load("return read_u32_le(\"\\x00\\x01\\x00\\x00\", 1)")
        .eval()
        .unwrap();
    assert_eq!(result, 256);
}

#[test]
fn read_i32_be_negative() {
    let lua = setup_lua();
    let result: i64 = lua
        .load("return read_i32_be(\"\\xff\\xff\\xff\\xfe\", 1)")
        .eval()
        .unwrap();
    assert_eq!(result, -2);
}

#[test]
fn read_i32_le_negative() {
    let lua = setup_lua();
    let result: i64 = lua
        .load("return read_i32_le(\"\\xfe\\xff\\xff\\xff\", 1)")
        .eval()
        .unwrap();
    assert_eq!(result, -2);
}

// ---- read_float ----

#[test]
fn read_float_be() {
    let lua = setup_lua();
    // 3.14 as f32 BE bytes: 40 48 f5 c3
    let result: f64 = lua
        .load("return read_float_be(\"\\x40\\x48\\xf5\\xc3\", 1)")
        .eval()
        .unwrap();
    assert!((result - 3.14).abs() < 0.001);
}

#[test]
fn read_float_le() {
    let lua = setup_lua();
    // 3.14 as f32 LE bytes: c3 f5 48 40
    let result: f64 = lua
        .load("return read_float_le(\"\\xc3\\xf5\\x48\\x40\", 1)")
        .eval()
        .unwrap();
    assert!((result - 3.14).abs() < 0.001);
}

// ---- read_double ----

#[test]
fn read_double_be() {
    let lua = setup_lua();
    // 1.0 as f64 BE: 3F F0 00 00 00 00 00 00
    let result: f64 = lua
        .load("return read_double_be(\"\\x3f\\xf0\\x00\\x00\\x00\\x00\\x00\\x00\", 1)")
        .eval()
        .unwrap();
    assert_eq!(result, 1.0);
}

#[test]
fn read_double_le() {
    let lua = setup_lua();
    // 1.0 as f64 LE: 00 00 00 00 00 00 F0 3F
    let result: f64 = lua
        .load("return read_double_le(\"\\x00\\x00\\x00\\x00\\x00\\x00\\xf0\\x3f\", 1)")
        .eval()
        .unwrap();
    assert_eq!(result, 1.0);
}

// ---- Offset handling ----

#[test]
fn offset_1_indexed() {
    let lua = setup_lua();
    // Two bytes: 0xAA 0xBB. Offset 1 should read 0xAA, offset 2 should read 0xBB.
    let r1: i64 = lua
        .load("return read_u8(\"\\xaa\\xbb\", 1)")
        .eval()
        .unwrap();
    let r2: i64 = lua
        .load("return read_u8(\"\\xaa\\xbb\", 2)")
        .eval()
        .unwrap();
    assert_eq!(r1, 0xAA);
    assert_eq!(r2, 0xBB);
}

#[test]
fn offset_out_of_bounds() {
    let lua = setup_lua();
    let result: Result<i64, _> = lua.load("return read_u8(\"\\xaa\", 2)").eval();
    assert!(result.is_err());
}

#[test]
fn read_u16_insufficient_bytes() {
    let lua = setup_lua();
    let result: Result<i64, _> = lua.load("return read_u16_be(\"\\xaa\", 1)").eval();
    assert!(result.is_err());
}

#[test]
fn read_u32_at_end_of_buffer() {
    let lua = setup_lua();
    // 4 bytes exactly → read_u32 at offset 1 should work
    let result: Result<i64, _> = lua
        .load("return read_u32_be(\"\\x00\\x00\\x00\\x01\", 1)")
        .eval();
    assert!(result.is_ok());
    // offset 2 needs bytes 2-5, but only 4 bytes → fail
    let result2: Result<i64, _> = lua
        .load("return read_u32_be(\"\\x00\\x00\\x00\\x01\", 2)")
        .eval();
    assert!(result2.is_err());
}

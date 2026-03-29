use cycbox_sdk::{Value, ValueType};

// ---- Boolean ----

#[test]
fn builder_boolean_true() {
    let v = Value::builder("x").boolean(true);
    assert_eq!(v.as_bool(), Some(true));
    assert_eq!(v.value_type, ValueType::Boolean);
}

#[test]
fn builder_boolean_false() {
    let v = Value::builder("x").boolean(false);
    assert_eq!(v.as_bool(), Some(false));
}

// ---- Signed integers ----

#[test]
fn builder_int8_boundary() {
    for val in [i8::MIN, 0, i8::MAX] {
        let v = Value::builder("x").int8(val);
        assert_eq!(v.as_i8(), Some(val));
    }
}

#[test]
fn builder_int16_boundary() {
    for val in [i16::MIN, i16::MAX] {
        let v = Value::builder("x").int16(val);
        assert_eq!(v.as_i16(), Some(val));
        assert_eq!(v.value_payload.len(), 2);
    }
}

#[test]
fn builder_int32_boundary() {
    for val in [i32::MIN, i32::MAX] {
        let v = Value::builder("x").int32(val);
        assert_eq!(v.as_i32(), Some(val));
    }
}

#[test]
fn builder_int64_boundary() {
    for val in [i64::MIN, i64::MAX] {
        let v = Value::builder("x").int64(val);
        assert_eq!(v.as_i64(), Some(val));
    }
}

// ---- Unsigned integers ----

#[test]
fn builder_uint8() {
    for val in [0u8, u8::MAX] {
        let v = Value::builder("x").uint8(val);
        assert_eq!(v.as_u8(), Some(val));
    }
}

#[test]
fn builder_uint16() {
    let v = Value::builder("x").uint16(u16::MAX);
    assert_eq!(v.as_u16(), Some(u16::MAX));
}

#[test]
fn builder_uint32() {
    let v = Value::builder("x").uint32(u32::MAX);
    assert_eq!(v.as_u32(), Some(u32::MAX));
}

#[test]
fn builder_uint64() {
    let v = Value::builder("x").uint64(u64::MAX);
    assert_eq!(v.as_u64(), Some(u64::MAX));
}

// ---- Floats ----

#[test]
fn builder_float32() {
    let v = Value::builder("x").float32(-1.5f32);
    assert_eq!(v.as_f32(), Some(-1.5f32));
}

#[test]
fn builder_float32_nan() {
    let v = Value::builder("x").float32(f32::NAN);
    assert!(v.as_f32().unwrap().is_nan());
}

#[test]
fn builder_float32_infinity() {
    let v = Value::builder("x").float32(f32::INFINITY);
    assert_eq!(v.as_f32(), Some(f32::INFINITY));
}

#[test]
fn builder_float64() {
    let v = Value::builder("x").float64(3.14159265358979);
    assert_eq!(v.as_f64(), Some(3.14159265358979));
}

#[test]
fn builder_float64_nan() {
    let v = Value::builder("x").float64(f64::NAN);
    assert!(v.as_f64().unwrap().is_nan());
}

// ---- String ----

#[test]
fn builder_string() {
    let v = Value::builder("x").string("hello");
    assert_eq!(v.as_string(), Some("hello".to_string()));
}

#[test]
fn builder_string_unicode() {
    let v = Value::builder("x").string("你好世界");
    assert_eq!(v.as_string(), Some("你好世界".to_string()));
}

#[test]
fn builder_string_empty() {
    let v = Value::builder("x").string("");
    assert_eq!(v.as_string(), Some(String::new()));
}

// ---- Type mismatch returns None ----

#[test]
fn type_mismatch_returns_none() {
    let v = Value::builder("x").boolean(true);
    assert_eq!(v.as_i8(), None);
    assert_eq!(v.as_u32(), None);
    assert_eq!(v.as_f64(), None);
    assert_eq!(v.as_string(), None);
    assert_eq!(v.as_i16_array(), None);
}

// ---- Arrays ----

#[test]
fn builder_int8_array() {
    let v = Value::builder("x").int8_array(&[-128, 0, 127]);
    assert_eq!(v.as_i8_array(), Some(vec![-128, 0, 127]));
}

#[test]
fn builder_uint8_array() {
    let v = Value::builder("x").uint8_array(&[0, 128, 255]);
    assert_eq!(v.as_u8_array(), Some(vec![0, 128, 255]));
}

#[test]
fn builder_int16_array() {
    let v = Value::builder("x").int16_array(&[i16::MIN, 0, i16::MAX]);
    assert_eq!(v.as_i16_array(), Some(vec![i16::MIN, 0, i16::MAX]));
}

#[test]
fn builder_uint16_array() {
    let v = Value::builder("x").uint16_array(&[0, 1000, u16::MAX]);
    assert_eq!(v.as_u16_array(), Some(vec![0, 1000, u16::MAX]));
}

#[test]
fn builder_int32_array() {
    let v = Value::builder("x").int32_array(&[i32::MIN, i32::MAX]);
    assert_eq!(v.as_i32_array(), Some(vec![i32::MIN, i32::MAX]));
}

#[test]
fn builder_uint32_array() {
    let v = Value::builder("x").uint32_array(&[0, u32::MAX]);
    assert_eq!(v.as_u32_array(), Some(vec![0, u32::MAX]));
}

#[test]
fn builder_int64_array() {
    let v = Value::builder("x").int64_array(&[i64::MIN, i64::MAX]);
    assert_eq!(v.as_i64_array(), Some(vec![i64::MIN, i64::MAX]));
}

#[test]
fn builder_uint64_array() {
    let v = Value::builder("x").uint64_array(&[0, u64::MAX]);
    assert_eq!(v.as_u64_array(), Some(vec![0, u64::MAX]));
}

#[test]
fn builder_float32_array() {
    let v = Value::builder("x").float32_array(&[1.0, -2.5, 3.14]);
    let arr = v.as_f32_array().unwrap();
    assert_eq!(arr.len(), 3);
    assert!((arr[2] - 3.14).abs() < 0.001);
}

#[test]
fn builder_float64_array() {
    let v = Value::builder("x").float64_array(&[1.0, -2.5]);
    assert_eq!(v.as_f64_array(), Some(vec![1.0, -2.5]));
}

#[test]
fn empty_array() {
    let v = Value::builder("x").int16_array(&[]);
    assert_eq!(v.as_i16_array(), Some(vec![]));
}

#[test]
fn array_type_mismatch() {
    let v = Value::builder("x").int8_array(&[1, 2]);
    assert_eq!(v.as_u16_array(), None);
}

// ---- Timestamp ----

#[test]
fn builder_timestamp_explicit() {
    let v = Value::builder("x").timestamp(12345).uint8(1);
    assert_eq!(v.timestamp, 12345);
}

#[test]
fn builder_timestamp_auto() {
    let v = Value::builder("x").uint8(1);
    assert!(v.timestamp > 0);
}

// ---- Convenience constructors ----

#[test]
fn new_boolean_convenience() {
    let v = Value::new_boolean("b", true);
    assert_eq!(v.as_bool(), Some(true));
}

#[test]
fn new_u64_convenience() {
    let v = Value::new_u64("n", 42);
    assert_eq!(v.as_u64(), Some(42));
}

#[test]
fn new_string_convenience() {
    let v = Value::new_string("s", "hi");
    assert_eq!(v.as_string(), Some("hi".to_string()));
}

#[test]
fn new_u8_array_convenience() {
    let v = Value::new_u8_array("a", vec![1, 2, 3]);
    assert_eq!(v.as_u8_array(), Some(vec![1, 2, 3]));
}

// ---- as_bytes ----

#[test]
fn as_bytes_returns_payload() {
    let v = Value::builder("x").uint16(0x1234);
    let bytes = v.as_bytes();
    assert_eq!(bytes, 0x1234u16.to_le_bytes().to_vec());
}

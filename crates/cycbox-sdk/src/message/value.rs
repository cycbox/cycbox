use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ValueType {
    Boolean,

    Int8,
    Int16,
    Int32,
    Int64,

    UInt8,
    UInt16,
    UInt32,
    UInt64,

    Float32,
    Float64,

    Int8Array,
    UInt8Array,
    Int16Array,
    UInt16Array,
    Int32Array,
    UInt32Array,
    Int64Array,
    UInt64Array,
    Float32Array,
    Float64Array,

    String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Value {
    pub id: String, // unique identifier for the value in format "group:line_id", e.g., "temperature:sensor1", "modbus_rtu_1:coil_0"
    pub timestamp: u64, // timestamp in microseconds since epoch
    pub value_type: ValueType,
    pub value_payload: Vec<u8>,
}

impl Value {
    /// Create a new ValueBuilder with the specified id
    pub fn builder(id: impl Into<String>) -> ValueBuilder {
        ValueBuilder::new(id)
    }

    pub fn new_boolean(id: impl Into<String>, value: bool) -> Self {
        Value {
            id: id.into(),
            timestamp: 0,
            value_type: ValueType::Boolean,
            value_payload: vec![if value { 1 } else { 0 }],
        }
    }

    pub fn new_u8_array(id: impl Into<String>, values: impl Into<Vec<u8>>) -> Self {
        Value {
            id: id.into(),
            timestamp: 0,
            value_type: ValueType::UInt8Array,
            value_payload: values.into(),
        }
    }

    pub fn new_string(id: impl Into<String>, value: impl Into<String>) -> Self {
        Value {
            id: id.into(),
            timestamp: 0,
            value_type: ValueType::String,
            value_payload: value.into().into_bytes(),
        }
    }

    pub fn new_u64(id: impl Into<String>, value: u64) -> Self {
        Value {
            id: id.into(),
            timestamp: 0,
            value_type: ValueType::UInt64,
            value_payload: value.to_le_bytes().to_vec(),
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        if self.value_type == ValueType::Boolean && !self.value_payload.is_empty() {
            Some(self.value_payload[0] != 0)
        } else {
            None
        }
    }

    pub fn as_i8(&self) -> Option<i8> {
        if self.value_type == ValueType::Int8 && !self.value_payload.is_empty() {
            Some(self.value_payload[0] as i8)
        } else {
            None
        }
    }

    pub fn as_i16(&self) -> Option<i16> {
        if self.value_type == ValueType::Int16 && self.value_payload.len() >= 2 {
            Some(i16::from_le_bytes([
                self.value_payload[0],
                self.value_payload[1],
            ]))
        } else {
            None
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        if self.value_type == ValueType::Int32 && self.value_payload.len() >= 4 {
            Some(i32::from_le_bytes([
                self.value_payload[0],
                self.value_payload[1],
                self.value_payload[2],
                self.value_payload[3],
            ]))
        } else {
            None
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        if self.value_type == ValueType::Int64 && self.value_payload.len() >= 8 {
            Some(i64::from_le_bytes([
                self.value_payload[0],
                self.value_payload[1],
                self.value_payload[2],
                self.value_payload[3],
                self.value_payload[4],
                self.value_payload[5],
                self.value_payload[6],
                self.value_payload[7],
            ]))
        } else {
            None
        }
    }

    pub fn as_u8(&self) -> Option<u8> {
        if self.value_type == ValueType::UInt8 && !self.value_payload.is_empty() {
            Some(self.value_payload[0])
        } else {
            None
        }
    }

    pub fn as_u16(&self) -> Option<u16> {
        if self.value_type == ValueType::UInt16 && self.value_payload.len() >= 2 {
            Some(u16::from_le_bytes([
                self.value_payload[0],
                self.value_payload[1],
            ]))
        } else {
            None
        }
    }

    pub fn as_u32(&self) -> Option<u32> {
        if self.value_type == ValueType::UInt32 && self.value_payload.len() >= 4 {
            Some(u32::from_le_bytes([
                self.value_payload[0],
                self.value_payload[1],
                self.value_payload[2],
                self.value_payload[3],
            ]))
        } else {
            None
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        if self.value_type == ValueType::UInt64 && self.value_payload.len() >= 8 {
            Some(u64::from_le_bytes([
                self.value_payload[0],
                self.value_payload[1],
                self.value_payload[2],
                self.value_payload[3],
                self.value_payload[4],
                self.value_payload[5],
                self.value_payload[6],
                self.value_payload[7],
            ]))
        } else {
            None
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        if self.value_type == ValueType::Float32 && self.value_payload.len() >= 4 {
            Some(f32::from_le_bytes([
                self.value_payload[0],
                self.value_payload[1],
                self.value_payload[2],
                self.value_payload[3],
            ]))
        } else {
            None
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        if self.value_type == ValueType::Float64 && self.value_payload.len() >= 8 {
            Some(f64::from_le_bytes([
                self.value_payload[0],
                self.value_payload[1],
                self.value_payload[2],
                self.value_payload[3],
                self.value_payload[4],
                self.value_payload[5],
                self.value_payload[6],
                self.value_payload[7],
            ]))
        } else {
            None
        }
    }

    pub fn as_string(&self) -> Option<String> {
        if self.value_type == ValueType::String {
            Some(String::from_utf8_lossy(&self.value_payload).to_string())
        } else {
            None
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        self.value_payload.clone()
    }

    pub fn as_i8_array(&self) -> Option<Vec<i8>> {
        if self.value_type == ValueType::Int8Array {
            Some(self.value_payload.iter().map(|&b| b as i8).collect())
        } else {
            None
        }
    }

    pub fn as_u8_array(&self) -> Option<Vec<u8>> {
        if self.value_type == ValueType::UInt8Array {
            Some(self.value_payload.clone())
        } else {
            None
        }
    }

    pub fn as_i16_array(&self) -> Option<Vec<i16>> {
        if self.value_type == ValueType::Int16Array && self.value_payload.len().is_multiple_of(2) {
            Some(
                self.value_payload
                    .chunks_exact(2)
                    .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect(),
            )
        } else {
            None
        }
    }

    pub fn as_u16_array(&self) -> Option<Vec<u16>> {
        if self.value_type == ValueType::UInt16Array && self.value_payload.len().is_multiple_of(2) {
            Some(
                self.value_payload
                    .chunks_exact(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect(),
            )
        } else {
            None
        }
    }

    pub fn as_i32_array(&self) -> Option<Vec<i32>> {
        if self.value_type == ValueType::Int32Array && self.value_payload.len().is_multiple_of(4) {
            Some(
                self.value_payload
                    .chunks_exact(4)
                    .map(|chunk| i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect(),
            )
        } else {
            None
        }
    }

    pub fn as_u32_array(&self) -> Option<Vec<u32>> {
        if self.value_type == ValueType::UInt32Array && self.value_payload.len().is_multiple_of(4) {
            Some(
                self.value_payload
                    .chunks_exact(4)
                    .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect(),
            )
        } else {
            None
        }
    }

    pub fn as_i64_array(&self) -> Option<Vec<i64>> {
        if self.value_type == ValueType::Int64Array && self.value_payload.len().is_multiple_of(8) {
            Some(
                self.value_payload
                    .chunks_exact(8)
                    .map(|chunk| {
                        i64::from_le_bytes([
                            chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6],
                            chunk[7],
                        ])
                    })
                    .collect(),
            )
        } else {
            None
        }
    }

    pub fn as_u64_array(&self) -> Option<Vec<u64>> {
        if self.value_type == ValueType::UInt64Array && self.value_payload.len().is_multiple_of(8) {
            Some(
                self.value_payload
                    .chunks_exact(8)
                    .map(|chunk| {
                        u64::from_le_bytes([
                            chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6],
                            chunk[7],
                        ])
                    })
                    .collect(),
            )
        } else {
            None
        }
    }

    pub fn as_f32_array(&self) -> Option<Vec<f32>> {
        if self.value_type == ValueType::Float32Array && self.value_payload.len().is_multiple_of(4)
        {
            Some(
                self.value_payload
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect(),
            )
        } else {
            None
        }
    }

    pub fn as_f64_array(&self) -> Option<Vec<f64>> {
        if self.value_type == ValueType::Float64Array && self.value_payload.len().is_multiple_of(8)
        {
            Some(
                self.value_payload
                    .chunks_exact(8)
                    .map(|chunk| {
                        f64::from_le_bytes([
                            chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6],
                            chunk[7],
                        ])
                    })
                    .collect(),
            )
        } else {
            None
        }
    }
}

pub struct ValueBuilder {
    id: String,
    timestamp: u64,
}

impl ValueBuilder {
    /// Create a new ValueBuilder with the specified id and timestamp
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            timestamp: 0,
        }
    }

    pub fn timestamp(mut self, timestamp: u64) -> Self {
        self.timestamp = timestamp;
        self
    }

    fn get_timestamp(&self) -> u64 {
        if self.timestamp == 0 {
            // get current time in microseconds since epoch
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap();
            now.as_micros() as u64
        } else {
            self.timestamp
        }
    }

    pub fn boolean(self, value: bool) -> Value {
        let timestamp = self.get_timestamp();
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::Boolean,
            value_payload: vec![if value { 1 } else { 0 }],
        }
    }

    pub fn int8(self, value: i8) -> Value {
        let timestamp = self.get_timestamp();
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::Int8,
            value_payload: vec![value as u8],
        }
    }

    pub fn int16(self, value: i16) -> Value {
        let timestamp = self.get_timestamp();
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::Int16,
            value_payload: value.to_le_bytes().to_vec(),
        }
    }

    pub fn int32(self, value: i32) -> Value {
        let timestamp = self.get_timestamp();
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::Int32,
            value_payload: value.to_le_bytes().to_vec(),
        }
    }

    pub fn int64(self, value: i64) -> Value {
        let timestamp = self.get_timestamp();
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::Int64,
            value_payload: value.to_le_bytes().to_vec(),
        }
    }

    pub fn uint8(self, value: u8) -> Value {
        let timestamp = self.get_timestamp();
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::UInt8,
            value_payload: vec![value],
        }
    }

    pub fn uint16(self, value: u16) -> Value {
        let timestamp = self.get_timestamp();
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::UInt16,
            value_payload: value.to_le_bytes().to_vec(),
        }
    }

    pub fn uint32(self, value: u32) -> Value {
        let timestamp = self.get_timestamp();
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::UInt32,
            value_payload: value.to_le_bytes().to_vec(),
        }
    }

    pub fn uint64(self, value: u64) -> Value {
        let timestamp = self.get_timestamp();
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::UInt64,
            value_payload: value.to_le_bytes().to_vec(),
        }
    }

    pub fn float32(self, value: f32) -> Value {
        let timestamp = self.get_timestamp();
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::Float32,
            value_payload: value.to_le_bytes().to_vec(),
        }
    }

    pub fn float64(self, value: f64) -> Value {
        let timestamp = self.get_timestamp();
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::Float64,
            value_payload: value.to_le_bytes().to_vec(),
        }
    }

    pub fn string(self, value: impl Into<String>) -> Value {
        let timestamp = self.get_timestamp();
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::String,
            value_payload: value.into().into_bytes(),
        }
    }

    pub fn int8_array(self, values: &[i8]) -> Value {
        let timestamp = self.get_timestamp();
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::Int8Array,
            value_payload: values.iter().map(|&v| v as u8).collect(),
        }
    }

    pub fn uint8_array(self, values: &[u8]) -> Value {
        let timestamp = self.get_timestamp();
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::UInt8Array,
            value_payload: values.to_vec(),
        }
    }

    pub fn int16_array(self, values: &[i16]) -> Value {
        let timestamp = self.get_timestamp();
        let mut payload = Vec::with_capacity(values.len() * 2);
        for &value in values {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::Int16Array,
            value_payload: payload,
        }
    }

    pub fn uint16_array(self, values: &[u16]) -> Value {
        let timestamp = self.get_timestamp();
        let mut payload = Vec::with_capacity(values.len() * 2);
        for &value in values {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::UInt16Array,
            value_payload: payload,
        }
    }

    pub fn int32_array(self, values: &[i32]) -> Value {
        let timestamp = self.get_timestamp();
        let mut payload = Vec::with_capacity(values.len() * 4);
        for &value in values {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::Int32Array,
            value_payload: payload,
        }
    }

    pub fn uint32_array(self, values: &[u32]) -> Value {
        let timestamp = self.get_timestamp();
        let mut payload = Vec::with_capacity(values.len() * 4);
        for &value in values {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::UInt32Array,
            value_payload: payload,
        }
    }

    pub fn int64_array(self, values: &[i64]) -> Value {
        let timestamp = self.get_timestamp();
        let mut payload = Vec::with_capacity(values.len() * 8);
        for &value in values {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::Int64Array,
            value_payload: payload,
        }
    }

    pub fn uint64_array(self, values: &[u64]) -> Value {
        let timestamp = self.get_timestamp();
        let mut payload = Vec::with_capacity(values.len() * 8);
        for &value in values {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::UInt64Array,
            value_payload: payload,
        }
    }

    pub fn float32_array(self, values: &[f32]) -> Value {
        let timestamp = self.get_timestamp();
        let mut payload = Vec::with_capacity(values.len() * 4);
        for &value in values {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::Float32Array,
            value_payload: payload,
        }
    }

    pub fn float64_array(self, values: &[f64]) -> Value {
        let timestamp = self.get_timestamp();
        let mut payload = Vec::with_capacity(values.len() * 8);
        for &value in values {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        Value {
            id: self.id,
            timestamp,
            value_type: ValueType::Float64Array,
            value_payload: payload,
        }
    }
}

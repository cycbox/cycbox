use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "value_type", content = "value")]
#[serde(rename_all = "snake_case")]
pub enum FormValue {
    Text(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
}

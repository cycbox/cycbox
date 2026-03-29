use serde::{Deserialize, Serialize};

use super::form_value::FormValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Contains,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormCondition {
    pub field_key: String,
    pub operator: ConditionOperator,
    pub value: FormValue,
}

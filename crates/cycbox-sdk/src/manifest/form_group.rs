use serde::{Deserialize, Serialize};

use super::form_condition::FormCondition;
use super::form_field::FormField;

/// Corresponds to the WIT record `form-group`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormGroup {
    pub key: String,
    pub label: String,
    pub fields: Vec<FormField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<FormCondition>,
}

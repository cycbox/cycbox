use serde::{Deserialize, Serialize};

use super::form_value::FormValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormFieldOption {
    pub label: String,
    pub value: FormValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional icon identifier rendered next to the label by the UI.
    /// Known values: `"premium"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

impl FormFieldOption {
    /// Create a new FormFieldOption without description
    pub fn new(label: String, value: FormValue) -> Self {
        Self {
            label,
            value,
            description: None,
            icon: None,
        }
    }

    /// Create a new FormFieldOption with description
    pub fn with_description(label: String, value: FormValue, description: String) -> Self {
        Self {
            label,
            value,
            description: Some(description),
            icon: None,
        }
    }

    /// Attach an icon identifier (consumed by the UI to render a leading icon).
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

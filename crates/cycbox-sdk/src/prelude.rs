//! Prelude module for codec and transport plugin developers.
//!
//! Import everything you need with a single line:
//! ```rust
//! use cycbox_sdk::prelude::*;
//! ```

// Core traits
pub use crate::codec::Codec;
pub use crate::error::CycBoxError;
pub use crate::manifest::{Configurable, Manifestable};
pub use crate::transformer::Transformer;
pub use crate::transport::{CodecTransport, MessageTransport, Transport, TransportIO};

// Manifest and form schema
pub use crate::manifest::{
    ConditionOperator, FieldType, FormCondition, FormField, FormFieldOption, FormGroup, FormUtils,
    FormValue, Manifest, ManifestValues, PluginCategory, ValidationError,
};

pub use crate::message::{COMMAND_ID_CLEAR_HIGHLIGHT, COMMAND_ID_SET_HIGHLIGHT};

pub use crate::message::{
    Color, Content, ContentType, Decoration, Message, MessageBuilder, PayloadType, Value, ValueType,
};

pub use crate::message::{
    MESSAGE_TYPE_EVENT, MESSAGE_TYPE_LOG, MESSAGE_TYPE_REQUEST, MESSAGE_TYPE_RESPONSE,
    MESSAGE_TYPE_RX, MESSAGE_TYPE_TX,
};

pub use crate::run_mode::RunMode;
// Localization
pub use crate::l10n::{L10n, LocaleProvider, create_l10n_with_provider};
pub use crate::lua::{LuaEngine, LuaFunctionRegistrar, LuaFunctionRegistry};
pub use crate::message_input::{MessageInputGroup, MessageInputRegistry};

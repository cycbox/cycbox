use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    Text,
    RichText,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Color {
    // Primary colors
    Primary,
    OnPrimary,
    PrimaryContainer,
    OnPrimaryContainer,

    // Secondary colors
    Secondary,
    OnSecondary,
    SecondaryContainer,
    OnSecondaryContainer,

    // Tertiary colors
    Tertiary,
    OnTertiary,
    TertiaryContainer,
    OnTertiaryContainer,

    // Error colors
    Error,
    OnError,
    ErrorContainer,
    OnErrorContainer,

    // Surface colors
    Surface,
    OnSurface,
    SurfaceVariant,
    OnSurfaceVariant,
    SurfaceTint,
    SurfaceContainer,
    SurfaceContainerHigh,
    SurfaceContainerHighest,
    SurfaceContainerLow,
    SurfaceContainerLowest,

    // Background colors
    Background,
    OnBackground,

    // Outline colors
    Outline,
    OutlineVariant,

    // Special colors
    Transparent,
}

/// Style information for text content
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Decoration {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub color: Color,
    pub background: Color,
}

impl Default for Decoration {
    fn default() -> Self {
        Decoration {
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            color: Color::OnSurface,        // Material Design text color
            background: Color::Transparent, // Transparent background
        }
    }
}

impl Decoration {
    /// Check if this decoration is "empty" (all default values)
    pub fn is_empty(&self) -> bool {
        *self == Decoration::default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    pub content_type: ContentType,
    pub decoration: Decoration,
    pub payload: Vec<u8>,
    pub label: Option<String>,
}

impl Content {
    /// Create a Content instance with semantic coloring and styling
    ///
    /// # Parameters
    /// - `payload`: The text or data to display (accepts anything that converts to Vec<u8>)
    /// - `color`: The semantic color to apply
    /// - `bold`: Whether to apply bold styling
    ///
    /// ```
    pub fn styled(
        payload: impl Into<Vec<u8>>,
        color: Color,
        bold: bool,
        label: Option<String>,
    ) -> Self {
        Self {
            content_type: ContentType::RichText,
            decoration: Decoration {
                bold,
                color,
                ..Default::default()
            },
            payload: payload.into(),
            label,
        }
    }

    /// Create plain text content without decoration
    pub fn plain(payload: impl Into<Vec<u8>>) -> Self {
        Self {
            content_type: ContentType::Text,
            decoration: Decoration::default(),
            payload: payload.into(),
            label: None,
        }
    }

    // ========================================================================
    // Data Display Colors
    // ========================================================================

    /// Color for hexadecimal or binary data bytes (normal, non-highlighted)
    ///
    /// Used for: hex dumps, binary data representation, raw byte displays
    pub fn data(payload: impl Into<Vec<u8>>, label: Option<String>) -> Self {
        Self::styled(payload, Color::OnSurface, false, label)
    }

    // ========================================================================
    // Structural Element Colors
    // ========================================================================

    /// Color for memory addresses and byte offsets
    ///
    /// Prominent color to make addresses easily scannable
    pub fn address(payload: impl Into<Vec<u8>>, label: Option<String>) -> Self {
        Self::styled(payload, Color::Primary, false, label)
    }

    /// Color for separators and dividers between sections
    ///
    /// Used for: pipe characters (|), colons, section dividers
    pub fn separator(payload: impl Into<Vec<u8>>) -> Self {
        Self::styled(payload, Color::Outline, false, None)
    }

    /// Color for padding spaces and alignment characters
    pub fn padding(payload: impl Into<Vec<u8>>) -> Self {
        Self::styled(payload, Color::OutlineVariant, false, None)
    }

    /// Color for generic command identifiers
    ///
    /// Used in: AT commands, protocol-specific commands
    pub fn command(payload: impl Into<Vec<u8>>, label: Option<String>) -> Self {
        Self::styled(payload, Color::Primary, false, label)
    }

    /// Color for protocol headers and metadata sections
    ///
    /// Used for: header fields, protocol metadata, control information
    pub fn header(payload: impl Into<Vec<u8>>, label: Option<String>) -> Self {
        Self::styled(payload, Color::OnSurfaceVariant, false, label)
    }

    /// Color for length fields and size indicators
    ///
    /// Used for: packet length, data count, size fields
    pub fn length_field(payload: impl Into<Vec<u8>>, label: Option<String>) -> Self {
        Self::styled(payload, Color::OnSurfaceVariant, false, label)
    }

    // ========================================================================
    // Integrity & Status Colors
    // ========================================================================

    /// Color for checksum fields (generic, validity unknown)
    ///
    /// Used for: checksum bytes, hash fields when validation status is not yet determined
    pub fn checksum(payload: impl Into<Vec<u8>>, label: Option<String>) -> Self {
        Self::styled(payload, Color::Tertiary, false, label)
    }

    /// Color for invalid CRC/checksum (verification failed)
    ///
    /// Highlights integrity check failures
    pub fn checksum_invalid(payload: impl Into<Vec<u8>>, label: Option<String>) -> Self {
        Self::styled(payload, Color::Error, false, label)
    }

    /// Color for error indicators, exception codes, fault states
    ///
    /// Used for: Modbus exception responses, error flags, fault conditions
    pub fn error_indicator(payload: impl Into<Vec<u8>>, label: Option<String>) -> Self {
        Self::styled(payload, Color::Error, false, label)
    }

    /// Color for success/OK status indicators
    ///
    /// Used for: success flags, acknowledgments, positive status
    pub fn status_ok(payload: impl Into<Vec<u8>>, label: Option<String>) -> Self {
        Self::styled(payload, Color::Tertiary, false, label)
    }

    // ========================================================================
    // User Feedback Colors
    // ========================================================================

    /// Color for highlighted/searched content (user-initiated highlights)
    ///
    /// Should be bold and prominent to draw attention. Used for search matches.
    pub fn highlight(payload: impl Into<Vec<u8>>, label: Option<String>) -> Self {
        Self::styled(payload, Color::Error, true, label)
    }
}

// ============================================================================
// Protobuf conversions
// ============================================================================

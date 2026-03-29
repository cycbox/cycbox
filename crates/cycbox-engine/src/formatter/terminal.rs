use cycbox_sdk::{Color, Content, ContentType, Decoration, Message};
use encoding_rs::{Encoding, GB18030, UTF_8, UTF_16BE, UTF_16LE};

/// Format a message's frame as terminal text and populate the contents field
///
/// This utility function decodes binary data using the specified encoding and
/// optionally applies highlighting to matching byte sequences.
///
/// # Parameters
///
/// - `message`: The message to format (mutated to populate `contents` and `display_hex`)
/// - `encoding`: Character encoding to use (UTF-8, UTF-16, GB18030, etc.)
/// - `highlight_bytes`: Optional byte pattern to highlight in the output
///
/// # Behavior
///
/// - Returns early if `contents` is already populated
/// - Decodes `message.frame` using the specified encoding
/// - If decoding fails: sets `message.display_hex = true` and keeps `contents` empty
/// - If decoding succeeds: populates `contents` with decoded text
/// - Highlights matching byte sequences if `highlight_bytes` is provided
/// - Sets `message.highlighted = true` if any matches found
/// - Shows "(empty data)" if frame is empty
///
/// ```
pub fn format_terminal(
    message: &mut Message,
    encoding: &'static Encoding,
    highlight_bytes: Option<&[u8]>,
) {
    if !message.contents.is_empty() {
        return;
    }

    // Handle empty frame
    if message.frame.is_empty() {
        message.contents = vec![Content {
            content_type: ContentType::RichText,
            decoration: Decoration {
                color: Color::OnSurfaceVariant,
                italic: true,
                ..Default::default()
            },
            payload: b"(empty data)".to_vec(),
            label: None,
        }];
        message.highlighted = false;
        message.display_hex = false;
        return;
    }

    // Try to decode the frame
    let decode_result = decode_bytes(&message.frame, encoding);

    // If decoding failed, set display_hex and keep contents empty
    if decode_result.is_none() {
        message.display_hex = true;
        message.highlighted = false;
        return;
    }

    let text = decode_result.unwrap();
    // Strip single trailing newline to avoid empty lines in UI
    let text = text.strip_suffix('\n').unwrap_or(&text).to_string();

    // Check if text has too many non-viewable characters (>10%)
    if has_too_many_non_viewable_chars(&text) {
        message.display_hex = true;
        message.highlighted = false;
        return;
    }

    message.display_hex = false;

    // Check if highlighting is enabled
    if let Some(search_bytes) = highlight_bytes
        && !search_bytes.is_empty()
    {
        // Decode search bytes
        let search_text = if let Some(decoded) = decode_bytes(search_bytes, encoding) {
            decoded
        } else {
            // Can't decode search pattern, skip highlighting
            message.contents = vec![Content {
                content_type: ContentType::RichText,
                decoration: Decoration::default(),
                payload: text.as_bytes().to_vec(),
                label: None,
            }];
            message.highlighted = false;
            return;
        };

        // Find all occurrences of search bytes in the frame data
        let mut contents = Vec::new();
        let mut last_pos = 0;
        let mut has_highlighted_content = false;
        let mut search_start = 0;

        while let Some(pos) = text[search_start..].find(&search_text) {
            has_highlighted_content = true;
            let abs_pos = search_start + pos;

            // Add text before match (normal)
            if abs_pos > last_pos {
                contents.push(Content {
                    content_type: ContentType::RichText,
                    decoration: Decoration::default(),
                    payload: text.as_bytes()[last_pos..abs_pos].to_vec(),
                    label: None,
                });
            }

            // Add highlighted text
            let match_end = abs_pos + search_text.len();
            contents.push(Content {
                content_type: ContentType::RichText,
                decoration: Decoration {
                    color: Color::Error,
                    bold: true,
                    ..Default::default()
                },
                payload: text.as_bytes()[abs_pos..match_end].to_vec(),
                label: None,
            });

            last_pos = match_end;
            search_start = match_end;
        }

        // Add remaining text after last match
        if last_pos < text.len() {
            contents.push(Content {
                content_type: ContentType::RichText,
                decoration: Decoration::default(),
                payload: text.as_bytes()[last_pos..].to_vec(),
                label: None,
            });
        }

        message.contents = if contents.is_empty() {
            // No matches found, return normal text
            vec![Content {
                content_type: ContentType::RichText,
                decoration: Decoration::default(),
                payload: text.as_bytes().to_vec(),
                label: None,
            }]
        } else {
            contents
        };
        message.highlighted = has_highlighted_content;

        return;
    }

    // No highlighting - return normal text
    message.contents = vec![Content {
        content_type: ContentType::RichText,
        decoration: Decoration::default(),
        payload: text.as_bytes().to_vec(),
        label: None,
    }];
    message.highlighted = false;
}

/// Get encoding from name string
///
/// # Supported Encodings
///
/// - UTF-8, UTF-16LE, UTF-16BE
/// - GB18030 (Chinese)
/// - Big5 (Chinese Traditional)
/// - Shift_JIS, EUC-JP (Japanese)
/// - EUC-KR (Korean)
/// - ISO-8859-1 (Latin-1)
///
/// Returns UTF-8 as default for unknown encodings.
pub fn get_encoding_from_name(name: &str) -> &'static Encoding {
    match name.to_lowercase().as_str() {
        "utf-8" | "utf8" => UTF_8,
        "utf-16le" | "utf16le" => UTF_16LE,
        "utf-16be" | "utf16be" => UTF_16BE,
        "gb18030" => GB18030,
        "big5" => encoding_rs::BIG5,
        "shift_jis" | "sjis" => encoding_rs::SHIFT_JIS,
        "euc-jp" => encoding_rs::EUC_JP,
        "euc-kr" => encoding_rs::EUC_KR,
        "iso-8859-1" | "latin1" => encoding_rs::WINDOWS_1252,
        _ => UTF_8,
    }
}

// ============================================================================
// Helper Functions (Private)
// ============================================================================

/// Decode bytes to UTF-8 string using the configured encoding
/// Returns Some(decoded_string) if successful, None if decoding failed
fn decode_bytes(bytes: &[u8], encoding: &'static Encoding) -> Option<String> {
    // Fast path for UTF-8
    if encoding == UTF_8 {
        return match std::str::from_utf8(bytes) {
            Ok(s) => Some(normalize_line_endings(s)),
            Err(_) => None,
        };
    }

    // Decode using encoding_rs for other encodings
    let (decoded, _encoding, had_errors) = encoding.decode(bytes);

    if had_errors {
        None
    } else {
        Some(normalize_line_endings(&decoded))
    }
}

/// Normalize line endings: convert \r\n to \n and standalone \r to \n
fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Check if a character is viewable (printable or common whitespace)
fn is_viewable_char(c: char) -> bool {
    !c.is_control() || c == '\n' || c == '\r' || c == '\t'
}

/// Check if text has too many non-viewable characters (>10%)
/// Returns true if more than 10% of characters are non-viewable
fn has_too_many_non_viewable_chars(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }

    let total_chars = text.chars().count();
    let non_viewable_count = text.chars().filter(|c| !is_viewable_char(*c)).count();

    // If more than 10% are non-viewable, return true
    non_viewable_count * 10 > total_chars
}

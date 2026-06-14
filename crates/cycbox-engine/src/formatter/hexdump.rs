use cycbox_sdk::{Color, Content, Message};

/// Maximum number of frame bytes rendered as a hexdump. Some transports flush
/// very large frames in a single message, and a full hexdump of that builds hundreds of thousands of
/// Content nodes, enough to stall protobuf serialization and freeze the UI.
const MAX_HEXDUMP_BYTES: usize = 16 * 1024;

/// Format a message's frame as hexdump and populate the hex_contents field
///
/// This utility function formats binary data as a hexadecimal dump with an ASCII column,
/// similar to the output of the `hexdump -C` command. The formatted output is stored
/// in the message's `hex_contents` field.
///
/// # Format
///
/// ```text
/// 0000  41 42 43 44  45 46 47 48  |ABCDEFGH|
/// 0008  49 4a 4b 4c  4d 4e 4f 50  |IJKLMNOP|
/// ```
///
/// # Configuration (Hardcoded)
///
/// - **Bytes per line**: 16
/// - **Group size**: 8 (extra space every 8 bytes)
/// - **Hex case**: Lowercase
/// - **Colors**: Enabled (uses Content helper methods for semantic coloring)
///
/// # Parameters
///
/// - `message`: The message to format (mutated to populate `hex_contents`)
/// - `highlight_bytes`: Optional byte pattern to highlight in the output
///
/// # Behavior
///
/// - Clears any existing `hex_contents`
/// - Formats `message.frame` as hexdump
/// - Highlights matching byte sequences if `highlight_bytes` is provided
/// - Sets `message.highlighted = true` if any matches found
/// - Shows "(empty data)" if frame is empty
///
/// ```
pub fn format_hexdump(message: &mut Message, highlight_bytes: Option<Vec<u8>>) {
    // Configuration constants
    const BYTES_PER_LINE: usize = 16;
    const GROUP_SIZE: usize = 8;

    // Clear existing hex contents
    message.hex_contents.clear();
    let total_len = message.frame.len();
    let truncated = total_len > MAX_HEXDUMP_BYTES;
    let data = &message.frame[..total_len.min(MAX_HEXDUMP_BYTES)];
    let mut has_highlighted_content = false;

    // Handle empty data
    if data.is_empty() {
        message
            .hex_contents
            .push(Content::data("(empty data)", None));
        message.highlighted = false;
        return;
    }

    // Process data in chunks
    for (line_offset, chunk) in data.chunks(BYTES_PER_LINE).enumerate() {
        let byte_offset = line_offset * BYTES_PER_LINE;

        // Add address
        let address_str = format!("{byte_offset:04x}");
        message
            .hex_contents
            .push(Content::address(address_str.as_bytes(), None));

        // Build hex section with highlight support - batch consecutive bytes with same highlight state
        let mut hex_buffer = String::new();
        let mut is_buffer_highlighted = false;
        let mut first_byte_in_buffer = true;

        for (i, &byte) in chunk.iter().enumerate() {
            let abs_pos = byte_offset + i;
            let is_highlighted = is_highlighted(data, abs_pos, &highlight_bytes);

            // Track if any content is highlighted
            if is_highlighted {
                has_highlighted_content = true;
            }

            // If highlight state changes, flush buffer
            if !first_byte_in_buffer && is_highlighted != is_buffer_highlighted {
                let content = if is_buffer_highlighted {
                    Content::highlight(hex_buffer.as_bytes(), None)
                } else {
                    Content::data(hex_buffer.as_bytes(), None)
                };
                message.hex_contents.push(content);
                hex_buffer.clear();
            }

            // Add extra space between groups
            if i % GROUP_SIZE == 0 {
                hex_buffer.push_str("  ");
            }
            hex_buffer.push_str(&format!("{byte:02x} "));

            is_buffer_highlighted = is_highlighted;
            first_byte_in_buffer = false;
        }

        // Flush remaining hex buffer
        if !hex_buffer.is_empty() {
            let content = if is_buffer_highlighted {
                Content::highlight(hex_buffer.as_bytes(), None)
            } else {
                Content::data(hex_buffer.as_bytes(), None)
            };
            message.hex_contents.push(content);
        }

        // Pad with spaces if this line is shorter than bytes_per_line
        if chunk.len() < BYTES_PER_LINE {
            let missing_bytes = BYTES_PER_LINE - chunk.len();
            let missing_strings = "   ".repeat(missing_bytes);
            let missing_groups = missing_bytes / GROUP_SIZE;
            let missing_group_spaces = "  ".repeat(missing_groups);
            let padding = format!("{missing_strings}{missing_group_spaces}");
            message
                .hex_contents
                .push(Content::data(padding.as_bytes(), None));
        }

        // Add spacing before ASCII section
        message.hex_contents.push(Content::separator("  "));

        // Add ASCII section separator
        message.hex_contents.push(Content::separator("|"));

        // Build ASCII section - batch consecutive chars with same color/highlight state
        let mut ascii_buffer = String::new();
        let mut current_color = None;
        let mut current_is_highlighted = false;

        for (i, &byte) in chunk.iter().enumerate() {
            let abs_pos = byte_offset + i;
            let is_highlighted = is_highlighted(data, abs_pos, &highlight_bytes);
            let is_printable = is_printable_ascii(byte);

            let color = if is_highlighted {
                Color::Error // highlight color
            } else if is_printable {
                Color::Tertiary // ascii_printable color
            } else {
                Color::OutlineVariant // ascii_nonprintable color
            };

            // If color or highlight state changes, flush the buffer
            if let Some(prev_color) = current_color
                && (prev_color != color || current_is_highlighted != is_highlighted)
            {
                let content = Content::styled(
                    ascii_buffer.as_bytes(),
                    prev_color,
                    current_is_highlighted,
                    None,
                );
                message.hex_contents.push(content);
                ascii_buffer.clear();
            }

            current_color = Some(color);
            current_is_highlighted = is_highlighted;
            ascii_buffer.push(byte_to_ascii_char(byte));
        }

        // Flush remaining ASCII buffer
        if !ascii_buffer.is_empty()
            && let Some(color) = current_color
        {
            let content =
                Content::styled(ascii_buffer.as_bytes(), color, current_is_highlighted, None);
            message.hex_contents.push(content);
        }

        // Pad ASCII section if needed
        if chunk.len() < BYTES_PER_LINE {
            let padding = " ".repeat(BYTES_PER_LINE - chunk.len());
            message
                .hex_contents
                .push(Content::data(padding.as_bytes(), None));
        }

        message.hex_contents.push(Content::separator("|"));

        // Add newline (except for the very last line)
        let is_last_line = (line_offset + 1) * BYTES_PER_LINE >= data.len();
        if !is_last_line {
            message.hex_contents.push(Content::separator("\n"));
        }
    }

    // Note the truncation so the user knows the dump is a prefix, not the whole frame.
    if truncated {
        message.hex_contents.push(Content::separator("\n"));
        message.hex_contents.push(Content::data(
            format!("... {MAX_HEXDUMP_BYTES} of {total_len} bytes shown (hexdump truncated)"),
            None,
        ));
    }

    message.highlighted = has_highlighted_content;
}

/// Check if a byte at the given position in data is part of any highlight match
///
/// This method checks if the byte at `pos` is contained within any occurrence
/// of the search pattern. For a search pattern of length N, a byte at position `pos`
/// could be part of a match starting anywhere in [pos - N + 1, pos].
///
/// Example: searching for [0x41, 0x42] in data [0x41, 0x42, 0x43]
/// - pos=0: checks if match starts at 0 → YES (entire sequence [0x41, 0x42])
/// - pos=1: checks if match starts at 0 or 1 → YES (part of match starting at 0)
/// - pos=2: checks if match starts at 1 or 2 → NO
fn is_highlighted(data: &[u8], pos: usize, search_bytes: &Option<Vec<u8>>) -> bool {
    if let Some(search_bytes) = search_bytes {
        if search_bytes.is_empty() {
            return false;
        }

        let search_len = search_bytes.len();

        // Calculate the range of starting positions that could include this byte
        // A match starting at `start_pos` covers bytes [start_pos, start_pos + search_len)
        // For byte at `pos` to be included: start_pos <= pos < start_pos + search_len
        // Rearranging: pos - search_len + 1 <= start_pos <= pos
        let min_start = pos.saturating_sub(search_len - 1);

        // Check all possible starting positions that could include this byte
        for start_pos in min_start..=pos {
            if start_pos + search_len <= data.len()
                && &data[start_pos..start_pos + search_len] == search_bytes.as_slice()
            {
                return true;
            }
        }
    }
    false
}

fn is_printable_ascii(byte: u8) -> bool {
    (32..=126).contains(&byte)
}

fn byte_to_ascii_char(byte: u8) -> char {
    if is_printable_ascii(byte) {
        byte as char
    } else {
        '.'
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cycbox_sdk::MessageBuilder;

    fn dump_text(message: &Message) -> String {
        message
            .hex_contents
            .iter()
            .map(|c| String::from_utf8_lossy(&c.payload).into_owned())
            .collect()
    }

    #[test]
    fn small_frame_is_rendered_in_full() {
        let mut message = MessageBuilder::new().frame(b"hello world".to_vec()).build();
        format_hexdump(&mut message, None);

        let text = dump_text(&message);
        assert!(!text.contains("truncated"));
        // ASCII column reproduces the printable payload.
        assert!(text.contains("hello world"));
    }

    #[test]
    fn oversized_frame_is_truncated() {
        let total = MAX_HEXDUMP_BYTES * 2 + 7;
        let mut message = MessageBuilder::new().frame(vec![0xABu8; total]).build();
        format_hexdump(&mut message, None);

        let text = dump_text(&message);
        assert!(text.contains("hexdump truncated"));
        assert!(text.contains(&total.to_string()));

        // Only the capped prefix is dumped, so the node count is bounded by the
        // cap (a handful of nodes per 16-byte row) rather than the full frame.
        let capped_rows = MAX_HEXDUMP_BYTES / 16;
        assert!(message.hex_contents.len() < capped_rows * 16);
    }
}

mod hexdump;
mod terminal;

// Re-export terminal formatter functions and constants
pub use hexdump::format_hexdump;
pub use terminal::{format_terminal, get_encoding_from_name};

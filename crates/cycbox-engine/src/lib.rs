mod command;
mod connection;
pub mod delay;
mod engine;
mod error;
mod formatter;
pub mod l10n;
mod lua;
mod state;
mod tasks;
use once_cell::sync::Lazy;
use tokio::runtime::{Builder, Runtime};

pub use engine::Engine;
pub use error::EngineError;
pub use lua::DEFAULT_LUA_SCRIPT;

// Initialize rustls crypto provider for WebSocket TLS support
static RUSTLS_CRYPTO_PROVIDER: Lazy<()> = Lazy::new(|| {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
});

pub static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    // Ensure crypto provider is initialized
    let _ = &*RUSTLS_CRYPTO_PROVIDER;

    Builder::new_multi_thread()
        .thread_name("cycbox-engine")
        .enable_all() // Enable all I/O and time drivers
        .build()
        .expect("Failed to create Tokio runtime")
});

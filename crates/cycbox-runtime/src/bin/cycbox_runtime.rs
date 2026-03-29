use clap::Parser;
use cycbox_engine::Engine;
use cycbox_sdk::MESSAGE_TYPE_LOG;
use cycbox_sdk::manifest::ManifestValues;
use std::sync::Arc;

/// CycBox Runtime — headless engine for edge deployments.
///
/// Loads a Lua config script (with embedded JSON config block) and runs the
/// CycBox engine, printing all received messages to stdout.
#[derive(Parser)]
#[command(name = "cycbox-runtime", version, about)]
struct Cli {
    /// Path to the Lua config script (.lua).
    ///
    /// The script must contain a `--[[ ... ]]` block comment with embedded
    /// JSON ManifestValues (the format saved by the CycBox UI).
    #[arg(short, long, env = "CYCBOX_CONFIG")]
    config: String,

    /// Enable debug log output
    #[arg(short, long, default_value_t = false)]
    debug: bool,
}

const RUNTIME_MODULE: &str = "runtime";

fn main() {
    env_logger::init();

    let cli = Cli::parse();

    let values = ManifestValues::load_from_lua_file(&cli.config).unwrap_or_else(|e| {
        eprintln!("Failed to load config '{}': {e}", cli.config);
        std::process::exit(1);
    });

    let run_mode = Arc::new(cycbox_runtime::run_mode::RuntimeRunMode::new());
    let engine = Engine::new(run_mode.clone(), cli.debug);

    cycbox_engine::RUNTIME.block_on(async {
        // Build the manifest and merge saved values into it
        let base_manifest = engine.manifest("en").await;
        let manifest = values.merge_into_manifest(base_manifest);

        // Start the engine with the merged manifest
        if let Err(e) = engine.start(RUNTIME_MODULE, Some(manifest)).await {
            eprintln!("Engine failed to start: {e}");
            std::process::exit(1);
        }

        // Subscribe to all messages and print them
        let mut rx = engine.subscribe();
        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(msg) => {
                            if msg.message_type == MESSAGE_TYPE_LOG {
                                for content in &msg.contents {
                                    let text = String::from_utf8_lossy(&content.payload);
                                    println!("[LOG] {text}");
                                }
                            } else {
                                let payload_hex = msg
                                    .payload
                                    .iter()
                                    .map(|b| format!("{b:02X}"))
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                println!(
                                    "[{}] conn={} payload={}",
                                    msg.message_type, msg.connection_id, payload_hex
                                );
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            eprintln!("Warning: dropped {n} messages (channel lagged)");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    eprintln!("\nShutting down...");
                    if let Err(e) = engine.stop(RUNTIME_MODULE).await {
                        eprintln!("Error stopping engine: {e}");
                    }
                    break;
                }
            }
        }
    });
}

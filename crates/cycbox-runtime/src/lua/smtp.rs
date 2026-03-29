use anyhow::Result;
use lettre::message::{header, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use log::{debug, warn};
use mlua::Lua;

struct SmtpConfig {
    server: String,
    port: u16,
    tls_mode: String,
    username: Option<String>,
    password: Option<String>,
    from: String,
    to_addresses: Vec<String>,
    subject: String,
    text_body: Option<String>,
    html_body: Option<String>,
}

fn parse_smtp_config(config: mlua::Table) -> Result<SmtpConfig, String> {
    let server: String = config
        .get("server")
        .map_err(|_| "smtp: 'server' field is required".to_string())?;
    if server.is_empty() {
        return Err("smtp: 'server' cannot be empty".to_string());
    }

    let from: String = config
        .get("from")
        .map_err(|_| "smtp: 'from' field is required".to_string())?;

    let to_value: mlua::Value = config
        .get("to")
        .map_err(|_| "smtp: 'to' field is required".to_string())?;

    let subject: String = config
        .get("subject")
        .map_err(|_| "smtp: 'subject' field is required".to_string())?;

    let tls_mode: String = config.get("tls").unwrap_or_else(|_| "starttls".to_string());
    if !["starttls", "tls", "none"].contains(&tls_mode.as_str()) {
        return Err("smtp: 'tls' must be 'starttls', 'tls', or 'none'".to_string());
    }

    let port: u16 = config.get("port").unwrap_or(match tls_mode.as_str() {
        "tls" => 465,
        "none" => 25,
        _ => 587,
    });

    let username: Option<String> = config.get("username").ok();
    let password: Option<String> = config.get("password").ok();
    let text_body: Option<String> = config.get("text").ok();
    let html_body: Option<String> = config.get("html").ok();

    if text_body.is_none() && html_body.is_none() {
        return Err(
            "smtp: at least one of 'text' or 'html' body is required".to_string(),
        );
    }

    let to_addresses: Vec<String> = match to_value {
        mlua::Value::String(s) => {
            vec![s.to_str().map(|s| s.to_string()).unwrap_or_default()]
        }
        mlua::Value::Table(tbl) => tbl.sequence_values::<String>().flatten().collect(),
        _ => {
            return Err("smtp: 'to' must be a string or array of strings".to_string());
        }
    };

    if to_addresses.is_empty() {
        return Err("smtp: 'to' must contain at least one address".to_string());
    }

    Ok(SmtpConfig {
        server,
        port,
        tls_mode,
        username,
        password,
        from,
        to_addresses,
        subject,
        text_body,
        html_body,
    })
}

fn build_email(config: &SmtpConfig) -> Result<Message, String> {
    let from_mailbox: Mailbox = config
        .from
        .parse()
        .map_err(|e| format!("smtp: invalid 'from' address: {}", e))?;

    let mut builder = Message::builder().from(from_mailbox).subject(config.subject.clone());

    for to_addr in &config.to_addresses {
        let to_mailbox: Mailbox = to_addr
            .parse()
            .map_err(|e| format!("smtp: invalid 'to' address '{}': {}", to_addr, e))?;
        builder = builder.to(to_mailbox);
    }

    match (&config.text_body, &config.html_body) {
        (Some(text), Some(html)) => builder
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(header::ContentType::TEXT_PLAIN)
                            .body(text.clone()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(header::ContentType::TEXT_HTML)
                            .body(html.clone()),
                    ),
            )
            .map_err(|e| format!("smtp: failed to build message: {}", e)),
        (None, Some(html)) => builder
            .header(header::ContentType::TEXT_HTML)
            .body(html.clone())
            .map_err(|e| format!("smtp: failed to build HTML message: {}", e)),
        (Some(text), None) => builder
            .body(text.clone())
            .map_err(|e| format!("smtp: failed to build text message: {}", e)),
        (None, None) => unreachable!("Already validated that at least one body exists"),
    }
}

fn build_mailer(config: &SmtpConfig) -> Result<SmtpTransport, String> {
    let timeout = Some(std::time::Duration::from_secs(10));
    let username = config.username.clone();
    let password = config.password.clone();
    let port = config.port;

    let result = match config.tls_mode.as_str() {
        "tls" => SmtpTransport::relay(&config.server)
            .map(|b| b.port(port))
            .map(|b| {
                if let (Some(user), Some(pass)) = (username, password) {
                    b.credentials(Credentials::new(user, pass))
                } else {
                    b
                }
            })
            .map(|b| b.timeout(timeout))
            .map(|b| b.build()),
        "starttls" => SmtpTransport::starttls_relay(&config.server)
            .map(|b| b.port(port))
            .map(|b| {
                if let (Some(user), Some(pass)) = (username, password) {
                    b.credentials(Credentials::new(user, pass))
                } else {
                    b
                }
            })
            .map(|b| b.timeout(timeout))
            .map(|b| b.build()),
        "none" => Ok(SmtpTransport::builder_dangerous(&config.server)
            .port(port)
            .timeout(timeout)
            .build()),
        _ => unreachable!("Already validated TLS mode"),
    };

    result.map_err(|e| format!("smtp: failed to create SMTP transport: {}", e))
}

async fn do_smtp_send(config: mlua::Table) -> (bool, Option<String>) {
    let cfg = match parse_smtp_config(config) {
        Ok(c) => c,
        Err(e) => return (false, Some(e)),
    };

    let message = match build_email(&cfg) {
        Ok(m) => m,
        Err(e) => return (false, Some(e)),
    };

    let mailer = match build_mailer(&cfg) {
        Ok(m) => m,
        Err(e) => return (false, Some(e)),
    };

    let to_list = cfg.to_addresses.join(", ");
    match tokio::task::spawn_blocking(move || mailer.send(&message)).await {
        Ok(Ok(_)) => {
            debug!("[Lua] SMTP: email sent successfully to {}", to_list);
            (true, None)
        }
        Ok(Err(e)) => {
            warn!("[Lua] SMTP: failed to send email: {}", e);
            (false, Some(format!("SMTP send failed: {}", e)))
        }
        Err(e) => {
            warn!("[Lua] SMTP: task panicked: {}", e);
            (false, Some(format!("SMTP task panicked: {}", e)))
        }
    }
}

async fn do_smtp_send_async(config: mlua::Table) -> bool {
    let cfg = match parse_smtp_config(config) {
        Ok(c) => c,
        Err(e) => {
            warn!("[Lua] smtp_send_async: {}", e);
            return false;
        }
    };

    let message = match build_email(&cfg) {
        Ok(m) => m,
        Err(e) => {
            warn!("[Lua] smtp_send_async: {}", e);
            return false;
        }
    };

    let mailer = match build_mailer(&cfg) {
        Ok(m) => m,
        Err(e) => {
            warn!("[Lua] smtp_send_async: {}", e);
            return false;
        }
    };

    let to_list = cfg.to_addresses.join(", ");
    tokio::spawn(async move {
        match tokio::task::spawn_blocking(move || mailer.send(&message)).await {
            Ok(Ok(_)) => {
                debug!("[Lua] SMTP: email (async) sent successfully to {}", to_list);
            }
            Ok(Err(e)) => {
                warn!("[Lua] SMTP: email (async) send failed: {}", e);
            }
            Err(e) => {
                warn!("[Lua] SMTP: email (async) task panicked: {}", e);
            }
        }
    });

    true
}

/// Register SMTP helper functions in Lua
/// Provides functions for sending emails via SMTP.
/// Both blocking (with error return) and fire-and-forget (async) variants are provided.
///
/// TLS Support:
/// - STARTTLS: Upgrades connection to TLS after initial connection (port 587)
/// - TLS: Direct TLS connection (port 465)
/// - None: Unencrypted connection (port 25, not recommended)
pub(super) fn register_smtp_helpers(lua: &Lua) -> Result<()> {
    let globals = lua.globals();

    // Helper: SMTP send (blocking with error return)
    // ok, error = smtp_send({
    //     server = "smtp.example.com",   -- SMTP server hostname (required)
    //     port = 587,                     -- SMTP server port (default: 587 for STARTTLS, 465 for TLS, 25 for none)
    //     tls = "starttls",               -- TLS mode: "starttls", "tls", or "none" (default: "starttls")
    //     username = "user@example.com",  -- SMTP username (optional, for authenticated servers)
    //     password = "secret",            -- SMTP password (optional, for authenticated servers)
    //     from = "sender@example.com",    -- From address (required)
    //     to = "recipient@example.com",   -- To address (required, can be string or array)
    //     subject = "Test Email",         -- Subject line (required)
    //     text = "Plain text body",       -- Plain text body (optional if html is provided)
    //     html = "<h1>HTML body</h1>",    -- HTML body (optional if text is provided)
    // })
    // Returns (success: bool, error_msg: string|nil)
    // Timeout: 10 seconds
    let smtp_send = lua.create_async_function(|_lua, config: mlua::Table| async move {
        Ok(do_smtp_send(config).await)
    })?;
    globals.set("smtp_send", smtp_send)?;

    // Helper: SMTP send (fire-and-forget, non-blocking)
    // ok = smtp_send_async({ ... same config as smtp_send ... })
    // Returns true immediately if the request was queued, false on validation failure.
    // Errors are logged only.
    let smtp_send_async = lua.create_async_function(|_lua, config: mlua::Table| async move {
        Ok(do_smtp_send_async(config).await)
    })?;
    globals.set("smtp_send_async", smtp_send_async)?;

    Ok(())
}

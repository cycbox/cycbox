use anyhow::Result;
use log::{debug, warn};
use mlua::Lua;
use tokio::time::Duration;

struct NtfyConfig {
    url: String,
    topic: String,
    message: String,
    title: Option<String>,
    priority: Option<String>,
    tags: Option<String>,
    click: Option<String>,
    attach: Option<String>,
    actions: Option<String>,
    icon: Option<String>,
    email: Option<String>,
}

fn extract_config(config: &mlua::Table, caller: &str) -> Result<NtfyConfig, (bool, Option<String>)> {
    let topic: String = match config.get("topic") {
        Ok(t) => t,
        Err(_) => {
            return Err((false, Some(format!("{}: 'topic' field is required", caller))));
        }
    };

    if topic.is_empty() {
        return Err((false, Some(format!("{}: 'topic' cannot be empty", caller))));
    }

    let message: String = match config.get("message") {
        Ok(m) => m,
        Err(_) => {
            return Err((false, Some(format!("{}: 'message' field is required", caller))));
        }
    };

    let server: String = config
        .get("server")
        .unwrap_or_else(|_| "https://ntfy.sh".to_string());

    if server.is_empty() {
        return Err((false, Some(format!("{}: 'server' cannot be empty", caller))));
    }

    let url = format!("{}/{}", server.trim_end_matches('/'), topic);
    if reqwest::Url::parse(&url).is_err() {
        return Err((false, Some(format!("{}: invalid URL '{}'", caller, url))));
    }

    let title: Option<String> = config.get("title").ok();
    let priority: Option<String> = config.get("priority").ok();
    let click: Option<String> = config.get("click").ok();
    let attach: Option<String> = config.get("attach").ok();
    let actions: Option<String> = config.get("actions").ok();
    let icon: Option<String> = config.get("icon").ok();
    let email: Option<String> = config.get("email").ok();

    let tags: Option<String> = match config.get("tags") {
        Ok(mlua::Value::String(s)) => {
            let tag_str = s.to_str().map(|s| s.to_string()).unwrap_or_default();
            if tag_str.is_empty() { None } else { Some(tag_str) }
        }
        Ok(mlua::Value::Table(tbl)) => {
            let tag_list: Vec<String> = tbl.sequence_values::<String>().flatten().collect();
            if tag_list.is_empty() { None } else { Some(tag_list.join(",")) }
        }
        _ => None,
    };

    if let Some(ref p) = priority
        && !["max", "urgent", "high", "default", "low", "min"].contains(&p.as_str())
    {
        return Err((
            false,
            Some(format!(
                "{}: 'priority' must be one of: max, urgent, high, default, low, min",
                caller
            )),
        ));
    }

    Ok(NtfyConfig { url, topic, message, title, priority, tags, click, attach, actions, icon, email })
}

fn build_request(client: &reqwest::Client, cfg: &NtfyConfig) -> reqwest::RequestBuilder {
    let mut request = client.post(&cfg.url).body(cfg.message.clone());
    if let Some(ref t) = cfg.title    { request = request.header("Title", t); }
    if let Some(ref p) = cfg.priority { request = request.header("Priority", p); }
    if let Some(ref t) = cfg.tags     { request = request.header("Tags", t); }
    if let Some(ref c) = cfg.click    { request = request.header("Click", c); }
    if let Some(ref a) = cfg.attach   { request = request.header("Attach", a); }
    if let Some(ref a) = cfg.actions  { request = request.header("Actions", a); }
    if let Some(ref i) = cfg.icon     { request = request.header("Icon", i); }
    if let Some(ref e) = cfg.email    { request = request.header("Email", e); }
    request
}

async fn do_ntfy_send(config: mlua::Table) -> (bool, Option<String>) {
    let cfg = match extract_config(&config, "ntfy_send") {
        Ok(c) => c,
        Err(e) => return e,
    };

    let client = reqwest::Client::new();
    let request = build_request(&client, &cfg);

    match tokio::time::timeout(Duration::from_secs(10), request.send()).await {
        Ok(Ok(response)) => {
            let status = response.status().as_u16();
            if status == 200 {
                debug!("[Lua] ntfy: notification sent to topic '{}'", cfg.topic);
                (true, None)
            } else {
                let error_msg = format!("ntfy_send: server returned status {}", status);
                warn!("[Lua] ntfy: {}", error_msg);
                (false, Some(error_msg))
            }
        }
        Ok(Err(e)) => {
            let error_msg = format!("ntfy_send: request failed: {}", e);
            warn!("[Lua] ntfy: {}", error_msg);
            (false, Some(error_msg))
        }
        Err(_) => {
            let error_msg = "ntfy_send: request timed out (10s)".to_string();
            warn!("[Lua] ntfy: {}", error_msg);
            (false, Some(error_msg))
        }
    }
}

async fn do_ntfy_send_async(config: mlua::Table) -> bool {
    let cfg = match extract_config(&config, "ntfy_send_async") {
        Ok(c) => c,
        Err((_, msg)) => {
            warn!("[Lua] {}", msg.unwrap_or_default());
            return false;
        }
    };

    let topic = cfg.topic.clone();
    let client = reqwest::Client::new();
    let request = build_request(&client, &cfg);

    tokio::spawn(async move {
        match tokio::time::timeout(Duration::from_secs(10), request.send()).await {
            Ok(Ok(response)) => {
                let status = response.status().as_u16();
                if status == 200 {
                    debug!("[Lua] ntfy (async): notification sent to topic '{}'", topic);
                } else {
                    warn!(
                        "[Lua] ntfy (async): server returned status {} for topic '{}'",
                        status, topic
                    );
                }
            }
            Ok(Err(e)) => {
                warn!("[Lua] ntfy (async): request failed for topic '{}': {}", topic, e);
            }
            Err(_) => {
                warn!("[Lua] ntfy (async): request timed out (10s) for topic '{}'", topic);
            }
        }
    });

    true
}

/// Register ntfy helper functions in Lua
/// Provides functions for sending push notifications via ntfy.sh or self-hosted ntfy servers.
/// Both blocking (with error return) and fire-and-forget (async) variants are provided.
///
/// Documentation: https://docs.ntfy.sh/
pub(super) fn register_ntfy_helpers(lua: &Lua) -> Result<()> {
    let globals = lua.globals();

    // Helper: ntfy send (blocking with error return)
    // ok, error = ntfy_send({
    //     server = "https://ntfy.sh",      -- Server URL (default: "https://ntfy.sh")
    //     topic = "my_alerts",             -- Topic name (required)
    //     message = "Alert message",       -- Message body (required)
    //     title = "Alert Title",           -- Notification title (optional)
    //     priority = "urgent",             -- Priority: max, urgent, high, default, low, min (optional)
    //     tags = "warning,skull",          -- Tags as comma-separated string or array (optional)
    //     click = "https://example.com",   -- URL to open on click (optional)
    //     attach = "https://example.com/image.jpg",  -- Attachment URL (optional)
    //     actions = "view, ...",           -- Actions JSON (optional, see ntfy docs)
    //     icon = "https://example.com/icon.png",  -- Icon URL (optional)
    //     email = "user@example.com",      -- Email address to forward to (optional)
    // })
    // Returns (success: bool, error_msg: string|nil)
    // Timeout: 10 seconds
    let ntfy_send = lua.create_async_function(|_lua, config: mlua::Table| async move {
        Ok(do_ntfy_send(config).await)
    })?;
    globals.set("ntfy_send", ntfy_send)?;

    // Helper: ntfy send (fire-and-forget, non-blocking)
    // ok = ntfy_send_async({ ... same config as ntfy_send ... })
    // Returns true immediately if the request was queued, false on validation failure.
    // Errors are logged only.
    let ntfy_send_async = lua.create_async_function(|_lua, config: mlua::Table| async move {
        Ok(do_ntfy_send_async(config).await)
    })?;
    globals.set("ntfy_send_async", ntfy_send_async)?;

    Ok(())
}

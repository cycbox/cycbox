use anyhow::Result;
use log::{debug, warn};
use mlua::Lua;
use serde::Serialize;
use tokio::time::Duration;

/// Discord webhook payload structure
#[derive(Serialize)]
struct DiscordWebhookPayload {
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
}

fn validate_webhook_url(webhook_url: &str) -> Option<String> {
    if webhook_url.is_empty() {
        return Some("webhook_url cannot be empty".to_string());
    }
    if !webhook_url.starts_with("https://discord.com/api/webhooks/")
        && !webhook_url.starts_with("https://discordapp.com/api/webhooks/")
    {
        return Some("invalid Discord webhook URL".to_string());
    }
    None
}

fn validate_content(content: &str) -> Option<String> {
    if content.is_empty() {
        return Some("content cannot be empty".to_string());
    }
    if content.len() > 2000 {
        return Some(format!(
            "content exceeds 2000 characters (got {})",
            content.len()
        ));
    }
    None
}

fn validate_username(username: &Option<String>) -> Option<String> {
    if let Some(user) = username
        && user.len() > 80
    {
        return Some(format!(
            "username exceeds 80 characters (got {})",
            user.len()
        ));
    }
    None
}

async fn do_discord_send(
    webhook_url: String,
    content: String,
    username: Option<String>,
    avatar_url: Option<String>,
) -> (bool, Option<String>) {
    if let Some(err) = validate_webhook_url(&webhook_url) {
        return (false, Some(format!("discord_send: {}", err)));
    }
    if let Some(err) = validate_content(&content) {
        return (false, Some(format!("discord_send: {}", err)));
    }
    if let Some(err) = validate_username(&username) {
        return (false, Some(format!("discord_send: {}", err)));
    }

    let payload = DiscordWebhookPayload {
        content,
        username,
        avatar_url,
    };

    let json_body = match serde_json::to_string(&payload) {
        Ok(json) => json,
        Err(e) => {
            return (
                false,
                Some(format!("discord_send: failed to serialize payload: {}", e)),
            );
        }
    };

    let client = reqwest::Client::new();
    match tokio::time::timeout(
        Duration::from_secs(5),
        client
            .post(&webhook_url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(json_body)
            .send(),
    )
    .await
    {
        Ok(Ok(response)) => {
            let status = response.status().as_u16();
            if status == 204 {
                debug!("[Lua] Discord: webhook sent successfully");
                (true, None)
            } else {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "unknown error".to_string());
                warn!(
                    "[Lua] Discord: webhook failed with status={}, error={}",
                    status, error_text
                );
                (
                    false,
                    Some(format!(
                        "Discord webhook returned status {}: {}",
                        status, error_text
                    )),
                )
            }
        }
        Ok(Err(e)) => {
            warn!("[Lua] Discord: webhook request failed: {}", e);
            (false, Some(format!("Discord webhook request failed: {}", e)))
        }
        Err(_) => {
            warn!("[Lua] Discord: webhook timed out (5s)");
            (false, Some("Discord webhook timed out (5s)".to_string()))
        }
    }
}

async fn do_discord_send_async(
    webhook_url: String,
    content: String,
    username: Option<String>,
    avatar_url: Option<String>,
) -> bool {
    if let Some(err) = validate_webhook_url(&webhook_url) {
        warn!("[Lua] discord_send_async: {}", err);
        return false;
    }
    if let Some(err) = validate_content(&content) {
        warn!("[Lua] discord_send_async: {}", err);
        return false;
    }
    if let Some(err) = validate_username(&username) {
        warn!("[Lua] discord_send_async: {}", err);
        return false;
    }

    let payload = DiscordWebhookPayload {
        content,
        username,
        avatar_url,
    };

    let json_body = match serde_json::to_string(&payload) {
        Ok(json) => json,
        Err(e) => {
            warn!(
                "[Lua] discord_send_async: failed to serialize payload: {}",
                e
            );
            return false;
        }
    };

    tokio::spawn(async move {
        let client = reqwest::Client::new();
        match tokio::time::timeout(
            Duration::from_secs(5),
            client
                .post(&webhook_url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(json_body)
                .send(),
        )
        .await
        {
            Ok(Ok(response)) => {
                let status = response.status().as_u16();
                if status == 204 {
                    debug!("[Lua] Discord: webhook (async) sent successfully");
                } else {
                    let error_text = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "unknown error".to_string());
                    warn!(
                        "[Lua] Discord: webhook (async) failed with status={}, error={}",
                        status, error_text
                    );
                }
            }
            Ok(Err(e)) => {
                warn!("[Lua] Discord: webhook (async) request failed: {}", e);
            }
            Err(_) => {
                warn!("[Lua] Discord: webhook (async) timed out (5s)");
            }
        }
    });

    true
}

/// Register Discord webhook helper functions in Lua
/// Provides functions for sending messages to Discord via webhooks.
/// Both blocking (with error return) and fire-and-forget (async) variants are provided.
///
/// Proxy Support:
/// Automatically uses HTTP_PROXY, HTTPS_PROXY, ALL_PROXY, and NO_PROXY environment variables.
/// Supports both uppercase and lowercase variants (e.g., http_proxy, https_proxy).
pub(super) fn register_discord_helpers(lua: &Lua) -> Result<()> {
    let globals = lua.globals();

    // Helper: Discord webhook send (blocking with error return)
    // ok, error = discord_send(webhook_url, content, username, avatar_url)
    // webhook_url: Discord webhook URL (required)
    // content: Message text (required, max 2000 characters)
    // username: Optional username override (max 80 characters)
    // avatar_url: Optional avatar URL override
    // Returns (success: bool, error_msg: string|nil)
    // Timeout: 5 seconds
    let discord_send = lua.create_async_function(
        |_lua,
         (webhook_url, content, username, avatar_url): (
            String,
            String,
            Option<String>,
            Option<String>,
        )| async move { Ok(do_discord_send(webhook_url, content, username, avatar_url).await) },
    )?;
    globals.set("discord_send", discord_send)?;

    // Helper: Discord webhook send (fire-and-forget, non-blocking)
    // ok = discord_send_async(webhook_url, content, username, avatar_url)
    // Returns true immediately if the request was queued, false on validation failure.
    // Errors and non-204 status codes are logged only.
    //
    // Proxy: Automatically uses HTTP_PROXY/HTTPS_PROXY/ALL_PROXY environment variables.
    let discord_send_async = lua.create_async_function(
        |_lua,
         (webhook_url, content, username, avatar_url): (
            String,
            String,
            Option<String>,
            Option<String>,
        )| async move {
            Ok(do_discord_send_async(webhook_url, content, username, avatar_url).await)
        },
    )?;
    globals.set("discord_send_async", discord_send_async)?;

    Ok(())
}

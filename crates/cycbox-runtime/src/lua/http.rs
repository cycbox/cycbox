use anyhow::Result;
use log::{debug, warn};
use mlua::Lua;
use tokio::time::Duration;

async fn http_post_impl(
    url: String,
    body_bytes: Vec<u8>,
    custom_headers: Vec<(String, String)>,
) {
    let client = reqwest::Client::new();
    let mut request = client.post(&url).body(body_bytes);

    // Default Content-Type if not provided by caller
    if !custom_headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("content-type"))
    {
        request =
            request.header(reqwest::header::CONTENT_TYPE, "application/octet-stream");
    }
    for (k, v) in &custom_headers {
        request = request.header(k.as_str(), v.as_str());
    }

    match tokio::time::timeout(Duration::from_secs(5), request.send()).await {
        Ok(Ok(response)) => {
            let status = response.status().as_u16();
            if status == 200 {
                debug!("[Lua] HTTP: POST '{}' - status={}", url, status);
            } else {
                warn!("[Lua] HTTP: POST '{}' - status={}", url, status);
            }
        }
        Ok(Err(e)) => {
            warn!("[Lua] HTTP: POST '{}' - request failed: {}", url, e);
        }
        Err(_) => {
            warn!("[Lua] HTTP: POST '{}' - timed out (5s)", url);
        }
    }
}

/// Register HTTP helper functions in Lua
/// Provides a fire-and-forget POST for outbound data reporting.
/// Non-200 responses and errors are logged only; nothing is returned to Lua.
///
/// Proxy Support:
/// Automatically uses HTTP_PROXY, HTTPS_PROXY, ALL_PROXY, and NO_PROXY environment variables.
/// Supports both uppercase and lowercase variants (e.g., http_proxy, https_proxy).
pub(super) fn register_http_helpers(lua: &Lua) -> Result<()> {
    let globals = lua.globals();

    // Helper: HTTP POST (fire-and-forget)
    // ok = http_post(url, body, headers)
    // body: string (raw bytes to send)
    // headers: optional table of {[string] = string} pairs.
    //          Content-Type defaults to "application/octet-stream" if not provided.
    // Returns true immediately if the request was queued, false on validation failure.
    // Errors and non-200 status codes are logged only. Timeout: 5 seconds.
    //
    // Proxy: Automatically uses HTTP_PROXY/HTTPS_PROXY/ALL_PROXY environment variables.
    let http_post = lua.create_async_function(
        |_lua, (url, body, headers): (String, mlua::String, Option<mlua::Table>)| async move {
            if url.is_empty() {
                warn!("[Lua] http_post: URL cannot be empty");
                return Ok(false);
            }

            if reqwest::Url::parse(&url).is_err() {
                warn!("[Lua] http_post: Invalid URL '{}'", url);
                return Ok(false);
            }

            let body_bytes = body.as_bytes().to_vec();

            // Extract headers into a Send-safe Vec before spawning
            let mut custom_headers: Vec<(String, String)> = Vec::new();
            if let Some(tbl) = headers {
                for (k, v) in tbl.pairs::<String, String>().flatten() {
                    custom_headers.push((k, v));
                }
            }

            tokio::spawn(http_post_impl(url, body_bytes, custom_headers));

            Ok(true)
        },
    )?;
    globals.set("http_post", http_post)?;

    Ok(())
}

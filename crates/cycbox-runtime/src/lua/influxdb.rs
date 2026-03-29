use anyhow::Result;
use log::{debug, warn};
use mlua::Lua;

/// Valid precisions for the v2 `/api/v2/write` compatibility endpoint.
const V2_PRECISIONS: &[&str] = &["ns", "us", "ms", "s", "m", "h"];

/// Valid precisions for the v3 `/api/v3/write_lp` endpoint.
const V3_PRECISIONS: &[&str] = &["auto", "nanosecond", "microsecond", "millisecond", "second"];

// ─── URL builders ────────────────────────────────────────────────────────────

fn build_v2_url(
    base_url: &str,
    org: &str,
    bucket: &str,
    precision: &str,
) -> Result<reqwest::Url, mlua::Error> {
    let base = base_url.trim_end_matches('/');
    let mut url = reqwest::Url::parse(&format!("{}/api/v2/write", base)).map_err(|e| {
        mlua::Error::RuntimeError(format!("invalid URL '{}': {}", base_url, e))
    })?;
    url.query_pairs_mut()
        .append_pair("org", org)
        .append_pair("bucket", bucket)
        .append_pair("precision", precision);
    Ok(url)
}

fn build_v3_url(
    base_url: &str,
    db: &str,
    precision: &str,
    accept_partial: bool,
    no_sync: bool,
) -> Result<reqwest::Url, mlua::Error> {
    let base = base_url.trim_end_matches('/');
    let mut url = reqwest::Url::parse(&format!("{}/api/v3/write_lp", base)).map_err(|e| {
        mlua::Error::RuntimeError(format!("invalid URL '{}': {}", base_url, e))
    })?;
    url.query_pairs_mut()
        .append_pair("db", db)
        .append_pair("precision", precision)
        .append_pair("accept_partial", &accept_partial.to_string())
        .append_pair("no_sync", &no_sync.to_string());
    Ok(url)
}

// ─── Shared HTTP sender ─────────────────────────────────────────────────────

/// POST a line-protocol payload with `Token` authentication.
async fn post_line_protocol(
    url: &reqwest::Url,
    token: &str,
    body: Vec<u8>,
) -> Result<u16, mlua::Error> {
    let client = reqwest::Client::new();
    let response = client
        .post(url.clone())
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Token {}", token),
        )
        .header(reqwest::header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(body)
        .send()
        .await
        .map_err(|e| mlua::Error::RuntimeError(format!("request failed: {}", e)))?;
    Ok(response.status().as_u16())
}

// ─── Table helper ────────────────────────────────────────────────────────────

/// Drain a Lua 1-based array of strings into a single `\n`-separated byte buffer.
fn table_to_line_bytes(table: &mlua::Table, fn_name: &str) -> Result<Vec<u8>, mlua::Error> {
    let len = table.raw_len();
    if len == 0 {
        return Err(mlua::Error::RuntimeError(format!(
            "{}: lines table cannot be empty",
            fn_name
        )));
    }
    let mut bytes = Vec::new();
    for i in 1..=len {
        let s: mlua::String = table.raw_get(i).map_err(|e| {
            mlua::Error::RuntimeError(format!("{}: lines[{}]: {}", fn_name, i, e))
        })?;
        let s_bytes = s.as_bytes();
        if s_bytes.is_empty() {
            return Err(mlua::Error::RuntimeError(format!(
                "{}: lines[{}] is empty",
                fn_name, i
            )));
        }
        if !bytes.is_empty() {
            bytes.push(b'\n');
        }
        bytes.extend_from_slice(&s_bytes);
    }
    Ok(bytes)
}

// ─── v2 impl functions ───────────────────────────────────────────────────────

async fn influxdb_write_v2_impl(
    url: String,
    token: String,
    org: String,
    bucket: String,
    line_bytes: Vec<u8>,
    precision: Option<String>,
) -> Result<u16, mlua::Error> {
    if url.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "influxdb_write_v2: url cannot be empty".into(),
        ));
    }
    if token.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "influxdb_write_v2: token cannot be empty".into(),
        ));
    }
    if org.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "influxdb_write_v2: org cannot be empty".into(),
        ));
    }
    if bucket.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "influxdb_write_v2: bucket cannot be empty".into(),
        ));
    }
    if line_bytes.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "influxdb_write_v2: line_data cannot be empty".into(),
        ));
    }

    let prec = precision.unwrap_or_else(|| "ns".into());
    if !V2_PRECISIONS.contains(&prec.as_str()) {
        return Err(mlua::Error::RuntimeError(format!(
            "influxdb_write_v2: invalid precision '{}' (must be ns, us, ms, s, m, or h)",
            prec
        )));
    }

    let write_url = build_v2_url(&url, &org, &bucket, &prec)?;
    debug!(
        "[Lua] InfluxDB v2: write bucket='{}' org='{}' precision='{}' {} bytes",
        bucket,
        org,
        prec,
        line_bytes.len()
    );

    let status = post_line_protocol(&write_url, &token, line_bytes).await?;
    debug!("[Lua] InfluxDB v2: status={} url={}", status, write_url);
    Ok(status)
}

async fn influxdb_batch_write_v2_impl(
    url: String,
    token: String,
    org: String,
    bucket: String,
    line_bytes: Vec<u8>,
    line_count: usize,
    precision: Option<String>,
) -> Result<u16, mlua::Error> {
    if url.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "influxdb_batch_write_v2: url cannot be empty".into(),
        ));
    }
    if token.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "influxdb_batch_write_v2: token cannot be empty".into(),
        ));
    }
    if org.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "influxdb_batch_write_v2: org cannot be empty".into(),
        ));
    }
    if bucket.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "influxdb_batch_write_v2: bucket cannot be empty".into(),
        ));
    }

    let prec = precision.unwrap_or_else(|| "ns".into());
    if !V2_PRECISIONS.contains(&prec.as_str()) {
        return Err(mlua::Error::RuntimeError(format!(
            "influxdb_batch_write_v2: invalid precision '{}' (must be ns, us, ms, s, m, or h)",
            prec
        )));
    }

    let write_url = build_v2_url(&url, &org, &bucket, &prec)?;
    debug!(
        "[Lua] InfluxDB v2: batch write bucket='{}' org='{}' precision='{}' {} lines {} bytes",
        bucket, org, prec, line_count, line_bytes.len()
    );

    let status = post_line_protocol(&write_url, &token, line_bytes).await?;
    debug!("[Lua] InfluxDB v2: status={} url={}", status, write_url);
    Ok(status)
}

async fn influxdb_write_v2_async_impl(
    url: String,
    token: String,
    org: String,
    bucket: String,
    line_bytes: Vec<u8>,
    precision: Option<String>,
) -> bool {
    if url.is_empty() {
        warn!("[Lua] influxdb_write_v2_async: url cannot be empty");
        return false;
    }
    if token.is_empty() {
        warn!("[Lua] influxdb_write_v2_async: token cannot be empty");
        return false;
    }
    if org.is_empty() {
        warn!("[Lua] influxdb_write_v2_async: org cannot be empty");
        return false;
    }
    if bucket.is_empty() {
        warn!("[Lua] influxdb_write_v2_async: bucket cannot be empty");
        return false;
    }
    if line_bytes.is_empty() {
        warn!("[Lua] influxdb_write_v2_async: line_data cannot be empty");
        return false;
    }

    let prec = precision.unwrap_or_else(|| "ns".into());
    if !V2_PRECISIONS.contains(&prec.as_str()) {
        warn!(
            "[Lua] influxdb_write_v2_async: invalid precision '{}'",
            prec
        );
        return false;
    }

    let write_url = match build_v2_url(&url, &org, &bucket, &prec) {
        Ok(u) => u,
        Err(e) => {
            warn!("[Lua] influxdb_write_v2_async: {}", e);
            return false;
        }
    };

    tokio::spawn(async move {
        match post_line_protocol(&write_url, &token, line_bytes).await {
            Ok(status) => {
                debug!(
                    "[Lua] InfluxDB v2 (async): status={} url={}",
                    status, write_url
                )
            }
            Err(e) => warn!("[Lua] InfluxDB v2 (async): {}", e),
        }
    });

    true
}

async fn influxdb_batch_write_v2_async_impl(
    url: String,
    token: String,
    org: String,
    bucket: String,
    line_bytes: Vec<u8>,
    precision: Option<String>,
) -> bool {
    if url.is_empty() {
        warn!("[Lua] influxdb_batch_write_v2_async: url cannot be empty");
        return false;
    }
    if token.is_empty() {
        warn!("[Lua] influxdb_batch_write_v2_async: token cannot be empty");
        return false;
    }
    if org.is_empty() {
        warn!("[Lua] influxdb_batch_write_v2_async: org cannot be empty");
        return false;
    }
    if bucket.is_empty() {
        warn!("[Lua] influxdb_batch_write_v2_async: bucket cannot be empty");
        return false;
    }

    let prec = precision.unwrap_or_else(|| "ns".into());
    if !V2_PRECISIONS.contains(&prec.as_str()) {
        warn!(
            "[Lua] influxdb_batch_write_v2_async: invalid precision '{}'",
            prec
        );
        return false;
    }

    let write_url = match build_v2_url(&url, &org, &bucket, &prec) {
        Ok(u) => u,
        Err(e) => {
            warn!("[Lua] influxdb_batch_write_v2_async: {}", e);
            return false;
        }
    };

    tokio::spawn(async move {
        match post_line_protocol(&write_url, &token, line_bytes).await {
            Ok(status) => {
                debug!(
                    "[Lua] InfluxDB v2 (async batch): status={} url={}",
                    status, write_url
                )
            }
            Err(e) => warn!("[Lua] InfluxDB v2 (async batch): {}", e),
        }
    });

    true
}

// ─── v3 impl functions ───────────────────────────────────────────────────────

async fn influxdb_write_v3_impl(
    url: String,
    token: String,
    db: String,
    line_bytes: Vec<u8>,
    precision: Option<String>,
    accept_partial: Option<bool>,
    no_sync: Option<bool>,
) -> Result<u16, mlua::Error> {
    if url.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "influxdb_write_v3: url cannot be empty".into(),
        ));
    }
    if token.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "influxdb_write_v3: token cannot be empty".into(),
        ));
    }
    if db.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "influxdb_write_v3: db cannot be empty".into(),
        ));
    }
    if line_bytes.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "influxdb_write_v3: line_data cannot be empty".into(),
        ));
    }

    let prec = precision.unwrap_or_else(|| "auto".into());
    if !V3_PRECISIONS.contains(&prec.as_str()) {
        return Err(mlua::Error::RuntimeError(format!(
            "influxdb_write_v3: invalid precision '{}' (must be auto, nanosecond, microsecond, millisecond, or second)",
            prec
        )));
    }
    let ap = accept_partial.unwrap_or(true);
    let ns = no_sync.unwrap_or(false);

    let write_url = build_v3_url(&url, &db, &prec, ap, ns)?;
    debug!(
        "[Lua] InfluxDB v3: write db='{}' precision='{}' accept_partial={} no_sync={} {} bytes",
        db, prec, ap, ns, line_bytes.len()
    );

    let status = post_line_protocol(&write_url, &token, line_bytes).await?;
    debug!("[Lua] InfluxDB v3: status={} url={}", status, write_url);
    Ok(status)
}

#[allow(clippy::too_many_arguments)]
async fn influxdb_batch_write_v3_impl(
    url: String,
    token: String,
    db: String,
    line_bytes: Vec<u8>,
    line_count: usize,
    precision: Option<String>,
    accept_partial: Option<bool>,
    no_sync: Option<bool>,
) -> Result<u16, mlua::Error> {
    if url.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "influxdb_batch_write_v3: url cannot be empty".into(),
        ));
    }
    if token.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "influxdb_batch_write_v3: token cannot be empty".into(),
        ));
    }
    if db.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "influxdb_batch_write_v3: db cannot be empty".into(),
        ));
    }

    let prec = precision.unwrap_or_else(|| "auto".into());
    if !V3_PRECISIONS.contains(&prec.as_str()) {
        return Err(mlua::Error::RuntimeError(format!(
            "influxdb_batch_write_v3: invalid precision '{}' (must be auto, nanosecond, microsecond, millisecond, or second)",
            prec
        )));
    }
    let ap = accept_partial.unwrap_or(true);
    let ns = no_sync.unwrap_or(false);

    let write_url = build_v3_url(&url, &db, &prec, ap, ns)?;
    debug!(
        "[Lua] InfluxDB v3: batch write db='{}' precision='{}' accept_partial={} no_sync={} {} lines {} bytes",
        db, prec, ap, ns, line_count, line_bytes.len()
    );

    let status = post_line_protocol(&write_url, &token, line_bytes).await?;
    debug!("[Lua] InfluxDB v3: status={} url={}", status, write_url);
    Ok(status)
}

async fn influxdb_write_v3_async_impl(
    url: String,
    token: String,
    db: String,
    line_bytes: Vec<u8>,
    precision: Option<String>,
    accept_partial: Option<bool>,
    no_sync: Option<bool>,
) -> bool {
    if url.is_empty() {
        warn!("[Lua] influxdb_write_v3_async: url cannot be empty");
        return false;
    }
    if token.is_empty() {
        warn!("[Lua] influxdb_write_v3_async: token cannot be empty");
        return false;
    }
    if db.is_empty() {
        warn!("[Lua] influxdb_write_v3_async: db cannot be empty");
        return false;
    }
    if line_bytes.is_empty() {
        warn!("[Lua] influxdb_write_v3_async: line_data cannot be empty");
        return false;
    }

    let prec = precision.unwrap_or_else(|| "auto".into());
    if !V3_PRECISIONS.contains(&prec.as_str()) {
        warn!(
            "[Lua] influxdb_write_v3_async: invalid precision '{}'",
            prec
        );
        return false;
    }
    let ap = accept_partial.unwrap_or(true);
    let ns = no_sync.unwrap_or(false);

    let write_url = match build_v3_url(&url, &db, &prec, ap, ns) {
        Ok(u) => u,
        Err(e) => {
            warn!("[Lua] influxdb_write_v3_async: {}", e);
            return false;
        }
    };

    tokio::spawn(async move {
        match post_line_protocol(&write_url, &token, line_bytes).await {
            Ok(status) => {
                debug!(
                    "[Lua] InfluxDB v3 (async): status={} url={}",
                    status, write_url
                )
            }
            Err(e) => warn!("[Lua] InfluxDB v3 (async): {}", e),
        }
    });

    true
}

async fn influxdb_batch_write_v3_async_impl(
    url: String,
    token: String,
    db: String,
    line_bytes: Vec<u8>,
    precision: Option<String>,
    accept_partial: Option<bool>,
    no_sync: Option<bool>,
) -> bool {
    if url.is_empty() {
        warn!("[Lua] influxdb_batch_write_v3_async: url cannot be empty");
        return false;
    }
    if token.is_empty() {
        warn!("[Lua] influxdb_batch_write_v3_async: token cannot be empty");
        return false;
    }
    if db.is_empty() {
        warn!("[Lua] influxdb_batch_write_v3_async: db cannot be empty");
        return false;
    }

    let prec = precision.unwrap_or_else(|| "auto".into());
    if !V3_PRECISIONS.contains(&prec.as_str()) {
        warn!(
            "[Lua] influxdb_batch_write_v3_async: invalid precision '{}'",
            prec
        );
        return false;
    }
    let ap = accept_partial.unwrap_or(true);
    let ns = no_sync.unwrap_or(false);

    let write_url = match build_v3_url(&url, &db, &prec, ap, ns) {
        Ok(u) => u,
        Err(e) => {
            warn!("[Lua] influxdb_batch_write_v3_async: {}", e);
            return false;
        }
    };

    tokio::spawn(async move {
        match post_line_protocol(&write_url, &token, line_bytes).await {
            Ok(status) => {
                debug!(
                    "[Lua] InfluxDB v3 (async batch): status={} url={}",
                    status, write_url
                )
            }
            Err(e) => warn!("[Lua] InfluxDB v3 (async batch): {}", e),
        }
    });

    true
}

// ─── Registration ───────────────────────────────────────────────────────────

/// Register InfluxDB helper functions in Lua.
///
/// #### v2 compatibility – `POST /api/v2/write`
/// - `influxdb_write_v2(url, token, org, bucket, line_data [, precision])`
/// - `influxdb_batch_write_v2(url, token, org, bucket, lines [, precision])`
/// - `influxdb_write_v2_async(...)` / `influxdb_batch_write_v2_async(...)`
///
/// #### v3 native – `POST /api/v3/write_lp`
/// - `influxdb_write_v3(url, token, db, line_data [, precision [, accept_partial [, no_sync]]])`
/// - `influxdb_batch_write_v3(url, token, db, lines [, precision [, accept_partial [, no_sync]]])`
/// - `influxdb_write_v3_async(...)` / `influxdb_batch_write_v3_async(...)`
pub(super) fn register_influxdb_helpers(lua: &Lua) -> Result<()> {
    let globals = lua.globals();

    // ── influxdb_write_v2 ────────────────────────────────────────────────
    //   status_code = influxdb_write_v2(url, token, org, bucket, line_data [, precision])
    //   precision: "ns"|"us"|"ms"|"s"|"m"|"h"  (default "ns")
    //   Returns HTTP status code.  Raises on validation / network failure.
    globals.set(
        "influxdb_write_v2",
        lua.create_async_function(
            |_lua,
             (url, token, org, bucket, line_data, precision): (
                String,
                String,
                String,
                String,
                mlua::String,
                Option<String>,
            )| async move {
                let line_bytes = line_data.as_bytes().to_vec();
                influxdb_write_v2_impl(url, token, org, bucket, line_bytes, precision).await
            },
        )?,
    )?;

    // ── influxdb_batch_write_v2 ──────────────────────────────────────────
    //   status_code = influxdb_batch_write_v2(url, token, org, bucket, lines [, precision])
    //   lines: Lua table – 1-based array of line-protocol strings
    //   Returns HTTP status code.  Raises on validation / network failure.
    globals.set(
        "influxdb_batch_write_v2",
        lua.create_async_function(
            |_lua,
             (url, token, org, bucket, lines, precision): (
                String,
                String,
                String,
                String,
                mlua::Table,
                Option<String>,
            )| async move {
                let line_count = lines.raw_len();
                let line_bytes = table_to_line_bytes(&lines, "influxdb_batch_write_v2")?;
                influxdb_batch_write_v2_impl(
                    url, token, org, bucket, line_bytes, line_count, precision,
                )
                .await
            },
        )?,
    )?;

    // ── influxdb_write_v2_async ──────────────────────────────────────────
    //   ok = influxdb_write_v2_async(url, token, org, bucket, line_data [, precision])
    //   Fire-and-forget.  Returns true if the request was queued, false on
    //   validation failure.  Network errors are logged only.
    globals.set(
        "influxdb_write_v2_async",
        lua.create_async_function(
            |_lua,
             (url, token, org, bucket, line_data, precision): (
                String,
                String,
                String,
                String,
                mlua::String,
                Option<String>,
            )| async move {
                let line_bytes = line_data.as_bytes().to_vec();
                Ok(influxdb_write_v2_async_impl(url, token, org, bucket, line_bytes, precision)
                    .await)
            },
        )?,
    )?;

    // ── influxdb_batch_write_v2_async ────────────────────────────────────
    //   ok = influxdb_batch_write_v2_async(url, token, org, bucket, lines [, precision])
    //   Fire-and-forget batch variant.  Same return semantics as the single-
    //   line async helper above.
    globals.set(
        "influxdb_batch_write_v2_async",
        lua.create_async_function(
            |_lua,
             (url, token, org, bucket, lines, precision): (
                String,
                String,
                String,
                String,
                mlua::Table,
                Option<String>,
            )| async move {
                let line_bytes =
                    match table_to_line_bytes(&lines, "influxdb_batch_write_v2_async") {
                        Ok(b) => b,
                        Err(e) => {
                            warn!("[Lua] {}", e);
                            return Ok(false);
                        }
                    };
                Ok(
                    influxdb_batch_write_v2_async_impl(url, token, org, bucket, line_bytes, precision)
                        .await,
                )
            },
        )?,
    )?;

    // ── influxdb_write_v3 ────────────────────────────────────────────────
    //   status_code = influxdb_write_v3(url, token, db, line_data [, precision [, accept_partial [, no_sync]]])
    //   precision:      "auto"|"nanosecond"|"microsecond"|"millisecond"|"second"  (default "auto")
    //   accept_partial: boolean (default true)
    //   no_sync:        boolean (default false)
    //   Returns HTTP status code.  Raises on validation / network failure.
    globals.set(
        "influxdb_write_v3",
        lua.create_async_function(
            |_lua,
             (url, token, db, line_data, precision, accept_partial, no_sync): (
                String,
                String,
                String,
                mlua::String,
                Option<String>,
                Option<bool>,
                Option<bool>,
            )| async move {
                let line_bytes = line_data.as_bytes().to_vec();
                influxdb_write_v3_impl(url, token, db, line_bytes, precision, accept_partial, no_sync)
                    .await
            },
        )?,
    )?;

    // ── influxdb_batch_write_v3 ──────────────────────────────────────────
    //   status_code = influxdb_batch_write_v3(url, token, db, lines [, precision [, accept_partial [, no_sync]]])
    //   lines: Lua table – 1-based array of line-protocol strings
    //   Returns HTTP status code.  Raises on validation / network failure.
    globals.set(
        "influxdb_batch_write_v3",
        lua.create_async_function(
            |_lua,
             (url, token, db, lines, precision, accept_partial, no_sync): (
                String,
                String,
                String,
                mlua::Table,
                Option<String>,
                Option<bool>,
                Option<bool>,
            )| async move {
                let line_count = lines.raw_len();
                let line_bytes = table_to_line_bytes(&lines, "influxdb_batch_write_v3")?;
                influxdb_batch_write_v3_impl(
                    url, token, db, line_bytes, line_count, precision, accept_partial, no_sync,
                )
                .await
            },
        )?,
    )?;

    // ── influxdb_write_v3_async ──────────────────────────────────────────
    //   ok = influxdb_write_v3_async(url, token, db, line_data [, precision [, accept_partial [, no_sync]]])
    //   Fire-and-forget.  Returns true if the request was queued, false on
    //   validation failure.  Network errors are logged only.
    globals.set(
        "influxdb_write_v3_async",
        lua.create_async_function(
            |_lua,
             (url, token, db, line_data, precision, accept_partial, no_sync): (
                String,
                String,
                String,
                mlua::String,
                Option<String>,
                Option<bool>,
                Option<bool>,
            )| async move {
                let line_bytes = line_data.as_bytes().to_vec();
                Ok(
                    influxdb_write_v3_async_impl(
                        url, token, db, line_bytes, precision, accept_partial, no_sync,
                    )
                    .await,
                )
            },
        )?,
    )?;

    // ── influxdb_batch_write_v3_async ────────────────────────────────────
    //   ok = influxdb_batch_write_v3_async(url, token, db, lines [, precision [, accept_partial [, no_sync]]])
    //   Fire-and-forget batch variant.  Same return semantics as the single-
    //   line async helper above.
    globals.set(
        "influxdb_batch_write_v3_async",
        lua.create_async_function(
            |_lua,
             (url, token, db, lines, precision, accept_partial, no_sync): (
                String,
                String,
                String,
                mlua::Table,
                Option<String>,
                Option<bool>,
                Option<bool>,
            )| async move {
                let line_bytes =
                    match table_to_line_bytes(&lines, "influxdb_batch_write_v3_async") {
                        Ok(b) => b,
                        Err(e) => {
                            warn!("[Lua] {}", e);
                            return Ok(false);
                        }
                    };
                Ok(
                    influxdb_batch_write_v3_async_impl(
                        url, token, db, line_bytes, precision, accept_partial, no_sync,
                    )
                    .await,
                )
            },
        )?,
    )?;

    Ok(())
}

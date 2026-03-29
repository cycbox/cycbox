use anyhow::Result;
use log::{debug, warn};
use mlua::{AnyUserData, Lua, UserData};
use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration;

/// Redis connection wrapper stored in Lua registry
struct RedisConnection {
    connection: Arc<Mutex<ConnectionManager>>,
}

impl UserData for RedisConnection {}

const REDIS_CONNECTION_KEY: &str = "_REDIS_CONNECTION";

/// Helper function to get Redis connection from Lua registry
async fn get_connection(lua: &Lua) -> Result<Arc<Mutex<ConnectionManager>>, String> {
    let userdata: AnyUserData = lua
        .named_registry_value(REDIS_CONNECTION_KEY)
        .map_err(|_| "Not connected to Redis. Call redis_connect() first.".to_string())?;

    let redis_conn = userdata
        .borrow::<RedisConnection>()
        .map_err(|e| format!("Failed to borrow Redis connection: {}", e))?;

    Ok(redis_conn.connection.clone())
}

async fn lua_redis_connect(
    lua: Lua,
    url: String,
    database: Option<u8>,
    username: Option<String>,
    password: Option<String>,
) -> mlua::Result<(bool, Option<String>)> {
    if url.is_empty() {
        return Ok((
            false,
            Some("redis_connect: URL cannot be empty".to_string()),
        ));
    }

    let db = database.unwrap_or(0);

    // Build connection URL with auth if provided
    let connection_url = if let (Some(user), Some(pass)) = (&username, &password) {
        // ACL authentication (Redis 6.0+)
        url.replace("redis://", &format!("redis://{}:{}@", user, pass))
    } else if let Some(pass) = &password {
        // Legacy password-only authentication
        url.replace("redis://", &format!("redis://:{}@", pass))
    } else {
        url.clone()
    };

    // Create Redis client
    let client = match redis::Client::open(connection_url.as_str()) {
        Ok(c) => c,
        Err(e) => {
            return Ok((false, Some(format!("Failed to create Redis client: {}", e))));
        }
    };

    // Connect with timeout to avoid blocking the Lua engine
    let connection = match tokio::time::timeout(
        Duration::from_millis(500),
        client.get_connection_manager(),
    )
    .await
    {
        Ok(Ok(conn)) => conn,
        Ok(Err(e)) => {
            return Ok((false, Some(format!("Failed to connect to Redis: {}", e))));
        }
        Err(_) => {
            return Ok((
                false,
                Some(
                    "Failed to connect to Redis: connection timed out (500ms)".to_string(),
                ),
            ));
        }
    };

    // Select database if not 0
    if db != 0 {
        let mut conn = connection.clone();
        if let Err(e) = redis::cmd("SELECT")
            .arg(db)
            .query_async::<()>(&mut conn)
            .await
        {
            return Ok((
                false,
                Some(format!("Failed to select database {}: {}", db, e)),
            ));
        }
    }

    debug!("[Lua] Redis: Connected to {} (database: {})", url, db);

    // Store connection in Lua registry
    let redis_conn = RedisConnection {
        connection: Arc::new(Mutex::new(connection)),
    };
    lua.set_named_registry_value(REDIS_CONNECTION_KEY, redis_conn)?;

    Ok((true, None::<String>))
}

async fn lua_redis_get(lua: Lua, key: String) -> mlua::Result<(Option<String>, Option<String>)> {
    if key.is_empty() {
        return Ok((
            None::<String>,
            Some("redis_get: key cannot be empty".to_string()),
        ));
    }

    let connection = match get_connection(&lua).await {
        Ok(conn) => conn,
        Err(e) => return Ok((None::<String>, Some(e))),
    };

    match tokio::time::timeout(Duration::from_millis(100), async {
        let mut conn = connection.lock().await;
        let result: Result<Option<String>, redis::RedisError> = conn.get(&key).await;
        result
    })
    .await
    {
        Ok(Ok(Some(value))) => {
            debug!("[Lua] Redis: GET key='{}' -> {} bytes", key, value.len());
            Ok((Some(value), None::<String>))
        }
        Ok(Ok(None)) => {
            debug!("[Lua] Redis: GET key='{}' -> not found", key);
            Ok((None::<String>, None::<String>))
        }
        Ok(Err(e)) => {
            debug!("[Lua] Redis: GET key='{}' -> error: {}", key, e);
            Ok((None::<String>, Some(format!("Redis GET failed: {}", e))))
        }
        Err(_) => {
            warn!("[Lua] Redis: GET key='{}' -> timed out (100ms)", key);
            Ok((
                None::<String>,
                Some("Redis GET timed out (100ms)".to_string()),
            ))
        }
    }
}

async fn lua_redis_set(
    lua: Lua,
    key: String,
    value: String,
    ttl_seconds: Option<u64>,
) -> mlua::Result<(bool, Option<String>)> {
    if key.is_empty() {
        return Ok((false, Some("redis_set: key cannot be empty".to_string())));
    }

    let connection = match get_connection(&lua).await {
        Ok(conn) => conn,
        Err(e) => return Ok((false, Some(e))),
    };

    match tokio::time::timeout(Duration::from_millis(100), async {
        let mut conn = connection.lock().await;
        let result: Result<(), redis::RedisError> = if let Some(ttl) = ttl_seconds {
            conn.set_ex(&key, &value, ttl).await
        } else {
            conn.set(&key, &value).await
        };
        result
    })
    .await
    {
        Ok(Ok(_)) => {
            debug!("[Lua] Redis: SET key='{}' ttl={:?}", key, ttl_seconds);
            Ok((true, None::<String>))
        }
        Ok(Err(e)) => {
            debug!("[Lua] Redis: SET key='{}' -> error: {}", key, e);
            Ok((false, Some(format!("Redis SET failed: {}", e))))
        }
        Err(_) => {
            warn!("[Lua] Redis: SET key='{}' -> timed out (100ms)", key);
            Ok((false, Some("Redis SET timed out (100ms)".to_string())))
        }
    }
}

async fn lua_redis_set_async(
    lua: Lua,
    key: String,
    value: String,
    ttl_seconds: Option<u64>,
) -> mlua::Result<bool> {
    if key.is_empty() {
        warn!("[Lua] redis_set_async: key cannot be empty");
        return Ok(false);
    }

    let connection = match get_connection(&lua).await {
        Ok(conn) => conn,
        Err(e) => {
            warn!("[Lua] redis_set_async: {}", e);
            return Ok(false);
        }
    };

    tokio::spawn(async move {
        match tokio::time::timeout(Duration::from_millis(1000), async {
            let mut conn = connection.lock().await;
            let result: Result<(), redis::RedisError> = if let Some(ttl) = ttl_seconds {
                conn.set_ex(&key, &value, ttl).await
            } else {
                conn.set(&key, &value).await
            };
            result
        })
        .await
        {
            Ok(Ok(_)) => {
                debug!(
                    "[Lua] Redis: SET (async) key='{}' ttl={:?}",
                    key, ttl_seconds
                )
            }
            Ok(Err(e)) => {
                warn!("[Lua] Redis: SET (async) key='{}' -> error: {}", key, e)
            }
            Err(_) => {
                warn!(
                    "[Lua] Redis: SET (async) key='{}' -> timed out (1000ms)",
                    key
                )
            }
        }
    });

    Ok(true)
}

async fn lua_redis_del(lua: Lua, key: String) -> mlua::Result<(u32, Option<String>)> {
    if key.is_empty() {
        return Ok((0u32, Some("redis_del: key cannot be empty".to_string())));
    }

    let connection = match get_connection(&lua).await {
        Ok(conn) => conn,
        Err(e) => return Ok((0u32, Some(e))),
    };

    match tokio::time::timeout(Duration::from_millis(1000), async {
        let mut conn = connection.lock().await;
        let result: Result<u32, redis::RedisError> = conn.del(&key).await;
        result
    })
    .await
    {
        Ok(Ok(count)) => {
            debug!("[Lua] Redis: DEL key='{}' -> deleted {}", key, count);
            Ok((count, None::<String>))
        }
        Ok(Err(e)) => {
            debug!("[Lua] Redis: DEL key='{}' -> error: {}", key, e);
            Ok((0u32, Some(format!("Redis DEL failed: {}", e))))
        }
        Err(_) => {
            warn!("[Lua] Redis: DEL key='{}' -> timed out (1000ms)", key);
            Ok((0u32, Some("Redis DEL timed out (1000ms)".to_string())))
        }
    }
}

async fn lua_redis_del_async(lua: Lua, key: String) -> mlua::Result<bool> {
    if key.is_empty() {
        warn!("[Lua] redis_del_async: key cannot be empty");
        return Ok(false);
    }

    let connection = match get_connection(&lua).await {
        Ok(conn) => conn,
        Err(e) => {
            warn!("[Lua] redis_del_async: {}", e);
            return Ok(false);
        }
    };

    tokio::spawn(async move {
        match tokio::time::timeout(Duration::from_millis(100), async {
            let mut conn = connection.lock().await;
            let result: Result<u32, redis::RedisError> = conn.del(&key).await;
            result
        })
        .await
        {
            Ok(Ok(count)) => {
                debug!(
                    "[Lua] Redis: DEL (async) key='{}' -> deleted {}",
                    key, count
                )
            }
            Ok(Err(e)) => {
                warn!("[Lua] Redis: DEL (async) key='{}' -> error: {}", key, e)
            }
            Err(_) => {
                warn!(
                    "[Lua] Redis: DEL (async) key='{}' -> timed out (100ms)",
                    key
                )
            }
        }
    });

    Ok(true)
}

async fn lua_redis_exists(lua: Lua, key: String) -> mlua::Result<(bool, Option<String>)> {
    if key.is_empty() {
        return Ok((false, Some("redis_exists: key cannot be empty".to_string())));
    }

    let connection = match get_connection(&lua).await {
        Ok(conn) => conn,
        Err(e) => return Ok((false, Some(e))),
    };

    match tokio::time::timeout(Duration::from_millis(100), async {
        let mut conn = connection.lock().await;
        let result: Result<bool, redis::RedisError> = conn.exists(&key).await;
        result
    })
    .await
    {
        Ok(Ok(exists)) => {
            debug!("[Lua] Redis: EXISTS key='{}' -> {}", key, exists);
            Ok((exists, None::<String>))
        }
        Ok(Err(e)) => {
            debug!("[Lua] Redis: EXISTS key='{}' -> error: {}", key, e);
            Ok((false, Some(format!("Redis EXISTS failed: {}", e))))
        }
        Err(_) => {
            warn!("[Lua] Redis: EXISTS key='{}' -> timed out (100ms)", key);
            Ok((false, Some("Redis EXISTS timed out (100ms)".to_string())))
        }
    }
}

async fn lua_redis_disconnect(lua: Lua) -> mlua::Result<(bool, Option<String>)> {
    match lua.named_registry_value::<AnyUserData>(REDIS_CONNECTION_KEY) {
        Ok(_) => {
            lua.set_named_registry_value(REDIS_CONNECTION_KEY, mlua::Nil)?;
            debug!("[Lua] Redis: Disconnected");
            Ok((true, None::<String>))
        }
        Err(_) => Ok((false, Some("Not connected to Redis".to_string()))),
    }
}

/// Register Redis helper functions in Lua
/// Provides functions for direct Redis operations (GET, SET, DEL, EXISTS, XADD)
/// Returns results directly without using the message system
pub(super) fn register_redis_helpers(lua: &Lua) -> Result<()> {
    let globals = lua.globals();

    // redis_connect(url, database, username, password)
    let redis_connect = lua.create_async_function(
        |lua,
         (url, database, username, password): (
            String,
            Option<u8>,
            Option<String>,
            Option<String>,
        )| { lua_redis_connect(lua, url, database, username, password) },
    )?;
    globals.set("redis_connect", redis_connect)?;

    // value, error = redis_get(key)
    let redis_get = lua
        .create_async_function(|lua, key: String| lua_redis_get(lua, key))?;
    globals.set("redis_get", redis_get)?;

    // ok, error = redis_set(key, value, ttl_seconds)
    let redis_set = lua.create_async_function(
        |lua, (key, value, ttl_seconds): (String, String, Option<u64>)| {
            lua_redis_set(lua, key, value, ttl_seconds)
        },
    )?;
    globals.set("redis_set", redis_set)?;

    // ok = redis_set_async(key, value, ttl_seconds)
    let redis_set_async = lua.create_async_function(
        |lua, (key, value, ttl_seconds): (String, String, Option<u64>)| {
            lua_redis_set_async(lua, key, value, ttl_seconds)
        },
    )?;
    globals.set("redis_set_async", redis_set_async)?;

    // deleted_count, error = redis_del(key)
    let redis_del = lua
        .create_async_function(|lua, key: String| lua_redis_del(lua, key))?;
    globals.set("redis_del", redis_del)?;

    // ok = redis_del_async(key)
    let redis_del_async = lua
        .create_async_function(|lua, key: String| lua_redis_del_async(lua, key))?;
    globals.set("redis_del_async", redis_del_async)?;

    // exists, error = redis_exists(key)
    let redis_exists = lua
        .create_async_function(|lua, key: String| lua_redis_exists(lua, key))?;
    globals.set("redis_exists", redis_exists)?;

    // redis_disconnect()
    let redis_disconnect = lua
        .create_async_function(|lua, _: ()| lua_redis_disconnect(lua))?;
    globals.set("redis_disconnect", redis_disconnect)?;

    Ok(())
}

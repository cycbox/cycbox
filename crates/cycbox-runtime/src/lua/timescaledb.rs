use anyhow::Result;
use bytes::BytesMut;
use deadpool_postgres::{Manager, Pool};
use log::{debug, info, warn};
use mlua::{AnyUserData, Lua, UserData};
use std::error::Error as StdError;
use tokio::time::Duration;
use tokio_postgres::{
    NoTls,
    types::{self, IsNull, ToSql, Type},
};

// ─── Constants ────────────────────────────────────────────────────────────────

/// PostgreSQL hard-caps bind-parameters at 65 535.
const MAX_PARAMS: usize = 65_535;

/// Default connection pool size.
const DEFAULT_POOL_SIZE: usize = 5;

/// Timeout (ms) for the initial connection verification in `timescaledb_connect`.
const CONNECT_TIMEOUT_MS: u64 = 5_000;

// ─── Parameter type enum ──────────────────────────────────────────────────────

/// Enum to hold different SQL parameter types while preserving type information.
/// This is needed because `Box<dyn ToSql>` loses type information required for
/// PostgreSQL serialization.
#[derive(Debug, Clone)]
enum SqlParam {
    Null,
    Text(String),
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl ToSql for SqlParam {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn StdError + Sync + Send>> {
        match self {
            SqlParam::Null => Ok(IsNull::Yes),

            SqlParam::Text(s) => {
                // Handle different text types based on PostgreSQL column type
                match *ty {
                    Type::TEXT | Type::VARCHAR | Type::CHAR | Type::BPCHAR | Type::NAME => {
                        s.to_sql(ty, out)
                    }
                    _ => Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("Cannot convert text to PostgreSQL type {:?}", ty),
                    )) as Box<dyn StdError + Sync + Send>),
                }
            }

            SqlParam::Int(i) => {
                // Handle different integer types based on PostgreSQL column type
                match *ty {
                    Type::INT2 => {
                        let val = i16::try_from(*i).map_err(|_| {
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                format!("Value {} out of range for INT2 (smallint)", i),
                            )) as Box<dyn StdError + Sync + Send>
                        })?;
                        val.to_sql(ty, out)
                    }
                    Type::INT4 => {
                        let val = i32::try_from(*i).map_err(|_| {
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                format!("Value {} out of range for INT4 (integer)", i),
                            )) as Box<dyn StdError + Sync + Send>
                        })?;
                        val.to_sql(ty, out)
                    }
                    Type::INT8 => i.to_sql(ty, out),
                    Type::NUMERIC => i.to_sql(ty, out),
                    _ => Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("Cannot convert integer to PostgreSQL type {:?}", ty),
                    )) as Box<dyn StdError + Sync + Send>),
                }
            }

            SqlParam::Float(f) => {
                // Handle different float types based on PostgreSQL column type
                match *ty {
                    Type::FLOAT4 => {
                        // f64 to f32 conversion with range check
                        if f.is_nan() || f.is_infinite() {
                            (*f as f32).to_sql(ty, out)
                        } else if *f > f32::MAX as f64 || *f < f32::MIN as f64 {
                            Err(Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                format!("Value {} out of range for FLOAT4 (real)", f),
                            ))
                                as Box<dyn StdError + Sync + Send>)
                        } else {
                            (*f as f32).to_sql(ty, out)
                        }
                    }
                    Type::FLOAT8 => f.to_sql(ty, out),
                    Type::NUMERIC => f.to_sql(ty, out),
                    _ => Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("Cannot convert float to PostgreSQL type {:?}", ty),
                    )) as Box<dyn StdError + Sync + Send>),
                }
            }

            SqlParam::Bool(b) => {
                // Boolean type validation
                match *ty {
                    Type::BOOL => b.to_sql(ty, out),
                    _ => Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("Cannot convert boolean to PostgreSQL type {:?}", ty),
                    )) as Box<dyn StdError + Sync + Send>),
                }
            }
        }
    }

    fn accepts(ty: &Type) -> bool {
        // Accept all types that SqlParam can handle
        matches!(
            *ty,
            Type::BOOL
                | Type::INT2
                | Type::INT4
                | Type::INT8
                | Type::FLOAT4
                | Type::FLOAT8
                | Type::TEXT
                | Type::VARCHAR
                | Type::CHAR
                | Type::BPCHAR
                | Type::NAME
                | Type::NUMERIC
        )
    }

    types::to_sql_checked!();
}

// ─── Pool wrapper stored in Lua registry ─────────────────────────────────────

struct TimescaleDbPool {
    pool: Pool,
}

impl UserData for TimescaleDbPool {}

const TSDB_POOL_KEY: &str = "_TIMESCALEDB_POOL";

/// Retrieve a clone of the pool from the Lua registry.
/// Cloning a `deadpool::Pool` is cheap – it just bumps an `Arc`.
fn get_pool(lua: &Lua) -> Result<Pool, mlua::Error> {
    let userdata: AnyUserData = lua.named_registry_value(TSDB_POOL_KEY).map_err(|_| {
        mlua::Error::RuntimeError(
            "Not connected to TimescaleDB. Call timescaledb_connect() first.".into(),
        )
    })?;

    let wrapper = userdata.borrow::<TimescaleDbPool>().map_err(|e| {
        mlua::Error::RuntimeError(format!("Failed to borrow TimescaleDB pool: {}", e))
    })?;

    Ok(wrapper.pool.clone())
}

// ─── Parameter extraction ────────────────────────────────────────────────────

/// Extract a 1-based Lua array into SQL bind parameters.
fn extract_params(table: &mlua::Table, fn_name: &str) -> Result<Vec<SqlParam>, mlua::Error> {
    let len = table.raw_len();
    let mut params: Vec<SqlParam> = Vec::with_capacity(len);
    for i in 1..=len {
        let val: mlua::Value = table
            .raw_get(i)
            .map_err(|e| mlua::Error::RuntimeError(format!("{}: params[{}]: {}", fn_name, i, e)))?;
        debug!("[Lua] Extracting param {}: {:?}", i, val);
        let p = match val {
            mlua::Value::Nil => SqlParam::Null,
            mlua::Value::String(s) => {
                let rust_string = s
                    .to_str()
                    .map_err(|e| {
                        mlua::Error::RuntimeError(format!(
                            "{}: params[{}]: invalid UTF-8: {}",
                            fn_name, i, e
                        ))
                    })?
                    .to_string();
                SqlParam::Text(rust_string)
            }
            mlua::Value::Integer(n) => SqlParam::Int(n),
            mlua::Value::Number(n) => SqlParam::Float(n),
            mlua::Value::Boolean(b) => SqlParam::Bool(b),
            _ => {
                return Err(mlua::Error::RuntimeError(format!(
                    "{}: params[{}]: unsupported type (use string, number, boolean, or nil)",
                    fn_name, i
                )));
            }
        };
        params.push(p);
    }
    Ok(params)
}

/// Convert a single Lua value to SqlParam.
fn value_to_sql_param(
    val: mlua::Value,
    fn_name: &str,
    context: &str,
) -> Result<SqlParam, mlua::Error> {
    match val {
        mlua::Value::Nil => Ok(SqlParam::Null),
        mlua::Value::String(s) => {
            let rust_string = s
                .to_str()
                .map_err(|e| {
                    mlua::Error::RuntimeError(format!(
                        "{}: {}: invalid UTF-8: {}",
                        fn_name, context, e
                    ))
                })?
                .to_string();
            Ok(SqlParam::Text(rust_string))
        }
        mlua::Value::Integer(n) => Ok(SqlParam::Int(n)),
        mlua::Value::Number(n) => Ok(SqlParam::Float(n)),
        mlua::Value::Boolean(b) => Ok(SqlParam::Bool(b)),
        _ => Err(mlua::Error::RuntimeError(format!(
            "{}: {}: unsupported type (use string, number, boolean, or nil)",
            fn_name, context
        ))),
    }
}

/// Build INSERT statement and extract parameters for a single row.
/// Returns (sql, params).
fn build_single_insert(
    table_name: &str,
    columns: &mlua::Table,
    values: &mlua::Table,
    fn_name: &str,
) -> Result<(String, Vec<SqlParam>), mlua::Error> {
    let col_count = columns.raw_len();
    let val_count = values.raw_len();

    if col_count == 0 {
        return Err(mlua::Error::RuntimeError(format!(
            "{}: columns array cannot be empty",
            fn_name
        )));
    }

    if col_count != val_count {
        return Err(mlua::Error::RuntimeError(format!(
            "{}: column count ({}) != value count ({})",
            fn_name, col_count, val_count
        )));
    }

    // Extract column names
    let mut col_names = Vec::with_capacity(col_count);
    for i in 1..=col_count {
        let col: String = columns.raw_get(i).map_err(|e| {
            mlua::Error::RuntimeError(format!("{}: columns[{}]: {}", fn_name, i, e))
        })?;
        col_names.push(col);
    }

    // Extract values
    let mut params = Vec::with_capacity(val_count);
    for i in 1..=val_count {
        let val: mlua::Value = values
            .raw_get(i)
            .map_err(|e| mlua::Error::RuntimeError(format!("{}: values[{}]: {}", fn_name, i, e)))?;
        params.push(value_to_sql_param(val, fn_name, &format!("values[{}]", i))?);
    }

    // Build SQL: INSERT INTO table (col1, col2) VALUES ($1, $2)
    let cols_str = col_names.join(", ");
    let placeholders: Vec<String> = (1..=col_count).map(|i| format!("${}", i)).collect();
    let placeholders_str = placeholders.join(", ");
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        table_name, cols_str, placeholders_str
    );

    Ok((sql, params))
}

/// Build INSERT statement and extract parameters for batch rows.
/// Returns (sql, params).
fn build_batch_insert(
    table_name: &str,
    columns: &mlua::Table,
    rows: &mlua::Table,
    fn_name: &str,
) -> Result<(String, Vec<SqlParam>), mlua::Error> {
    let col_count = columns.raw_len();
    let row_count = rows.raw_len();

    if col_count == 0 {
        return Err(mlua::Error::RuntimeError(format!(
            "{}: columns array cannot be empty",
            fn_name
        )));
    }

    if row_count == 0 {
        return Err(mlua::Error::RuntimeError(format!(
            "{}: rows array cannot be empty",
            fn_name
        )));
    }

    // Extract column names
    let mut col_names = Vec::with_capacity(col_count);
    for i in 1..=col_count {
        let col: String = columns.raw_get(i).map_err(|e| {
            mlua::Error::RuntimeError(format!("{}: columns[{}]: {}", fn_name, i, e))
        })?;
        col_names.push(col);
    }

    // Check parameter count limit
    let total_params = col_count * row_count;
    if total_params > MAX_PARAMS {
        return Err(mlua::Error::RuntimeError(format!(
            "{}: {} columns × {} rows = {} params exceeds maximum of {}",
            fn_name, col_count, row_count, total_params, MAX_PARAMS
        )));
    }

    // Extract all row values
    let mut params = Vec::with_capacity(total_params);
    for row_idx in 1..=row_count {
        let row: mlua::Table = rows.raw_get(row_idx).map_err(|e| {
            mlua::Error::RuntimeError(format!("{}: rows[{}]: {}", fn_name, row_idx, e))
        })?;

        let row_len = row.raw_len();
        if row_len != col_count {
            return Err(mlua::Error::RuntimeError(format!(
                "{}: rows[{}]: column count ({}) != expected ({})",
                fn_name, row_idx, row_len, col_count
            )));
        }

        for col_idx in 1..=col_count {
            let val: mlua::Value = row.raw_get(col_idx).map_err(|e| {
                mlua::Error::RuntimeError(format!(
                    "{}: rows[{}][{}]: {}",
                    fn_name, row_idx, col_idx, e
                ))
            })?;
            params.push(value_to_sql_param(
                val,
                fn_name,
                &format!("rows[{}][{}]", row_idx, col_idx),
            )?);
        }
    }

    // Build SQL: INSERT INTO table (col1, col2) VALUES ($1, $2), ($3, $4), ...
    let cols_str = col_names.join(", ");
    let mut value_sets = Vec::with_capacity(row_count);
    for row_idx in 0..row_count {
        let start = row_idx * col_count + 1;
        let placeholders: Vec<String> = (start..start + col_count)
            .map(|i| format!("${}", i))
            .collect();
        value_sets.push(format!("({})", placeholders.join(", ")));
    }
    let values_str = value_sets.join(", ");
    let sql = format!(
        "INSERT INTO {} ({}) VALUES {}",
        table_name, cols_str, values_str
    );

    Ok((sql, params))
}

// ─── Query execution ─────────────────────────────────────────────────────────

/// Execute a user-supplied SQL statement with bind parameters.
/// Returns the number of rows affected.
async fn execute_sql(
    pool: &Pool,
    sql: &str,
    params: &[SqlParam],
    fn_name: &str,
) -> Result<u64, mlua::Error> {
    if params.len() > MAX_PARAMS {
        return Err(mlua::Error::RuntimeError(format!(
            "{}: {} params exceeds maximum of {}",
            fn_name,
            params.len(),
            MAX_PARAMS
        )));
    }

    let client = pool.get().await.map_err(|e| {
        mlua::Error::RuntimeError(format!("{}: failed to acquire connection: {}", fn_name, e))
    })?;

    // Convert &[SqlParam] to Vec<&(dyn ToSql + Sync)>
    let params_ref: Vec<&(dyn ToSql + Sync)> =
        params.iter().map(|p| p as &(dyn ToSql + Sync)).collect();
    info!(
        "[Lua] Executing SQL: {} with {} params",
        sql,
        params_ref.len()
    );
    let affected = client
        .execute(sql, &params_ref)
        .await
        .map_err(|e| mlua::Error::RuntimeError(format!("{}: query failed: {}", fn_name, e)))?;

    Ok(affected)
}

// ─── Lua function implementations ────────────────────────────────────────────

async fn timescaledb_connect_impl(
    lua: Lua,
    (connstr, pool_size): (String, Option<usize>),
) -> mlua::Result<(bool, Option<String>)> {
    if connstr.is_empty() {
        return Ok((
            false,
            Some("timescaledb_connect: connstr cannot be empty".to_string()),
        ));
    }

    let pool_size = pool_size.unwrap_or(DEFAULT_POOL_SIZE);
    if pool_size == 0 {
        return Ok((
            false,
            Some("timescaledb_connect: pool_size must be >= 1".to_string()),
        ));
    }

    let pg_config = match connstr.parse::<tokio_postgres::Config>() {
        Ok(c) => c,
        Err(e) => {
            return Ok((
                false,
                Some(format!(
                    "timescaledb_connect: invalid connection string: {}",
                    e
                )),
            ));
        }
    };

    let mgr = Manager::new(pg_config, NoTls);
    let pool = match Pool::builder(mgr).max_size(pool_size).build() {
        Ok(p) => p,
        Err(e) => return Ok((false, Some(format!("timescaledb_connect: {}", e)))),
    };

    // Verify the server is reachable before storing the pool.
    match tokio::time::timeout(Duration::from_millis(CONNECT_TIMEOUT_MS), pool.get()).await {
        Ok(Ok(_)) => {} // client dropped → returned to pool
        Ok(Err(e)) => return Ok((false, Some(format!("timescaledb_connect: {}", e)))),
        Err(_) => {
            return Ok((
                false,
                Some(format!(
                    "timescaledb_connect: timed out after {}ms",
                    CONNECT_TIMEOUT_MS
                )),
            ));
        }
    }

    lua.set_named_registry_value(TSDB_POOL_KEY, TimescaleDbPool { pool })?;

    debug!("[Lua] TimescaleDB: Connected (pool_size={})", pool_size);
    Ok((true, None::<String>))
}

async fn timescaledb_disconnect_impl(lua: Lua, _: ()) -> mlua::Result<(bool, Option<String>)> {
    match lua.named_registry_value::<AnyUserData>(TSDB_POOL_KEY) {
        Ok(ud) => {
            {
                let wrapper = ud.borrow::<TimescaleDbPool>().map_err(|e| {
                    mlua::Error::RuntimeError(format!("timescaledb_disconnect: {}", e))
                })?;
                wrapper.pool.close();
            }
            lua.set_named_registry_value(TSDB_POOL_KEY, mlua::Nil)?;
            debug!("[Lua] TimescaleDB: Disconnected");
            Ok((true, None::<String>))
        }
        Err(_) => Ok((false, Some("Not connected to TimescaleDB".to_string()))),
    }
}

async fn timescaledb_execute_impl(
    lua: Lua,
    (sql, params): (String, Option<mlua::Table>),
) -> mlua::Result<u64> {
    if sql.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "timescaledb_execute: sql cannot be empty".into(),
        ));
    }

    let params = match params {
        Some(t) => extract_params(&t, "timescaledb_execute")?,
        None => Vec::new(),
    };

    let pool = get_pool(&lua)?;
    let affected = execute_sql(&pool, &sql, &params, "timescaledb_execute").await?;

    debug!(
        "[Lua] TimescaleDB: execute params={} affected={}",
        params.len(),
        affected
    );
    Ok(affected)
}

async fn timescaledb_execute_async_impl(
    lua: Lua,
    (sql, params): (String, Option<mlua::Table>),
) -> mlua::Result<bool> {
    if sql.is_empty() {
        warn!("[Lua] timescaledb_execute_async: sql cannot be empty");
        return Ok(false);
    }

    let params = match params {
        Some(t) => match extract_params(&t, "timescaledb_execute_async") {
            Ok(p) => p,
            Err(e) => {
                warn!("[Lua] {}", e);
                return Ok(false);
            }
        },
        None => Vec::new(),
    };

    let pool = match get_pool(&lua) {
        Ok(p) => p,
        Err(e) => {
            warn!("[Lua] {}", e);
            return Ok(false);
        }
    };

    tokio::spawn(async move {
        match execute_sql(&pool, &sql, &params, "timescaledb_execute_async").await {
            Ok(affected) => debug!(
                "[Lua] TimescaleDB (async): execute params={} affected={}",
                params.len(),
                affected
            ),
            Err(e) => warn!("[Lua] TimescaleDB (async): {}", e),
        }
    });

    Ok(true)
}

async fn timescaledb_insert_impl(
    lua: Lua,
    (table_name, columns, values): (String, mlua::Table, mlua::Table),
) -> mlua::Result<u64> {
    if table_name.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "timescaledb_insert: table_name cannot be empty".into(),
        ));
    }

    let (sql, params) = build_single_insert(&table_name, &columns, &values, "timescaledb_insert")?;

    let pool = get_pool(&lua)?;
    let affected = execute_sql(&pool, &sql, &params, "timescaledb_insert").await?;

    debug!(
        "[Lua] TimescaleDB: insert into {} affected={}",
        table_name, affected
    );
    Ok(affected)
}

async fn timescaledb_insert_async_impl(
    lua: Lua,
    (table_name, columns, values): (String, mlua::Table, mlua::Table),
) -> mlua::Result<bool> {
    if table_name.is_empty() {
        warn!("[Lua] timescaledb_insert_async: table_name cannot be empty");
        return Ok(false);
    }

    let (sql, params) =
        match build_single_insert(&table_name, &columns, &values, "timescaledb_insert_async") {
            Ok(result) => result,
            Err(e) => {
                warn!("[Lua] {}", e);
                return Ok(false);
            }
        };

    let pool = match get_pool(&lua) {
        Ok(p) => p,
        Err(e) => {
            warn!("[Lua] {}", e);
            return Ok(false);
        }
    };

    tokio::spawn(async move {
        match execute_sql(&pool, &sql, &params, "timescaledb_insert_async").await {
            Ok(affected) => debug!(
                "[Lua] TimescaleDB (async): insert into {} affected={}",
                table_name, affected
            ),
            Err(e) => warn!("[Lua] TimescaleDB (async): {}", e),
        }
    });

    Ok(true)
}

async fn timescaledb_insert_batch_impl(
    lua: Lua,
    (table_name, columns, rows): (String, mlua::Table, mlua::Table),
) -> mlua::Result<u64> {
    if table_name.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "timescaledb_insert_batch: table_name cannot be empty".into(),
        ));
    }

    let (sql, params) =
        build_batch_insert(&table_name, &columns, &rows, "timescaledb_insert_batch")?;

    let pool = get_pool(&lua)?;
    let affected = execute_sql(&pool, &sql, &params, "timescaledb_insert_batch").await?;

    debug!(
        "[Lua] TimescaleDB: batch insert into {} affected={}",
        table_name, affected
    );
    Ok(affected)
}

async fn timescaledb_insert_batch_async_impl(
    lua: Lua,
    (table_name, columns, rows): (String, mlua::Table, mlua::Table),
) -> mlua::Result<bool> {
    if table_name.is_empty() {
        warn!("[Lua] timescaledb_insert_batch_async: table_name cannot be empty");
        return Ok(false);
    }

    let (sql, params) = match build_batch_insert(
        &table_name,
        &columns,
        &rows,
        "timescaledb_insert_batch_async",
    ) {
        Ok(result) => result,
        Err(e) => {
            warn!("[Lua] {}", e);
            return Ok(false);
        }
    };

    let pool = match get_pool(&lua) {
        Ok(p) => p,
        Err(e) => {
            warn!("[Lua] {}", e);
            return Ok(false);
        }
    };

    tokio::spawn(async move {
        match execute_sql(&pool, &sql, &params, "timescaledb_insert_batch_async").await {
            Ok(affected) => debug!(
                "[Lua] TimescaleDB (async): batch insert into {} affected={}",
                table_name, affected
            ),
            Err(e) => warn!("[Lua] TimescaleDB (async): {}", e),
        }
    });

    Ok(true)
}

// ─── Registration ───────────────────────────────────────────────────────────

/// Register TimescaleDB helper functions in Lua.
///
/// All helpers share a single `deadpool` connection pool created by
/// `timescaledb_connect`.  **TLS is not currently supported** – pass
/// `sslmode=disable` in the connection string if the server requires an
/// explicit mode.
///
/// #### Connection management
/// - `timescaledb_connect(connstr, pool_size?) → (success, error_msg)`
/// - `timescaledb_disconnect() → (success, error_msg)`
///
/// #### Synchronous (awaited) – raise Lua error on failure
/// - `timescaledb_execute(sql, params?) → rows_affected`
/// - `timescaledb_insert(table, columns, values) → rows_affected`
/// - `timescaledb_insert_batch(table, columns, rows) → rows_affected`
///
/// #### Fire-and-forget (async) – log errors, never raise
/// - `timescaledb_execute_async(sql, params?) → true|false`
/// - `timescaledb_insert_async(table, columns, values) → true|false`
/// - `timescaledb_insert_batch_async(table, columns, rows) → true|false`
pub(super) fn register_timescaledb_helpers(lua: &Lua) -> Result<()> {
    let globals = lua.globals();

    globals.set(
        "timescaledb_connect",
        lua.create_async_function(timescaledb_connect_impl)?,
    )?;

    globals.set(
        "timescaledb_disconnect",
        lua.create_async_function(timescaledb_disconnect_impl)?,
    )?;

    globals.set(
        "timescaledb_execute",
        lua.create_async_function(timescaledb_execute_impl)?,
    )?;

    globals.set(
        "timescaledb_execute_async",
        lua.create_async_function(timescaledb_execute_async_impl)?,
    )?;

    globals.set(
        "timescaledb_insert",
        lua.create_async_function(timescaledb_insert_impl)?,
    )?;

    globals.set(
        "timescaledb_insert_async",
        lua.create_async_function(timescaledb_insert_async_impl)?,
    )?;

    globals.set(
        "timescaledb_insert_batch",
        lua.create_async_function(timescaledb_insert_batch_impl)?,
    )?;

    globals.set(
        "timescaledb_insert_batch_async",
        lua.create_async_function(timescaledb_insert_batch_async_impl)?,
    )?;

    Ok(())
}

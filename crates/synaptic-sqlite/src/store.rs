use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rusqlite::Connection;
use serde_json::Value;
use synaptic_core::{encode_namespace, now_iso, Item, SynapticError};

/// Configuration for [`SqliteStore`].
#[derive(Debug, Clone)]
pub struct SqliteStoreConfig {
    /// Path to the SQLite database file. Use `":memory:"` for an in-memory database.
    pub path: String,
}

impl SqliteStoreConfig {
    /// Create a new configuration with a file path.
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    /// Create a configuration for an in-memory SQLite database.
    pub fn in_memory() -> Self {
        Self {
            path: ":memory:".to_string(),
        }
    }
}

/// SQLite-backed implementation of the [`Store`](synaptic_core::Store) trait.
///
/// Uses a main table for key-value storage and an FTS5 virtual table for
/// full-text search. The FTS5 table is manually synchronized on `put()` and
/// `delete()` operations.
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    /// Create a new `SqliteStore` from the given configuration.
    ///
    /// Opens (or creates) the SQLite database and initializes the store
    /// and FTS5 tables if they do not already exist.
    pub fn new(config: SqliteStoreConfig) -> Result<Self, SynapticError> {
        let conn = Connection::open(&config.path)
            .map_err(|e| SynapticError::Store(format!("SQLite open error: {e}")))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS synaptic_store (
                namespace TEXT NOT NULL,
                key       TEXT NOT NULL,
                value     TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (namespace, key)
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS synaptic_store_fts USING fts5(
                key, value, namespace UNINDEXED
            );",
        )
        .map_err(|e| SynapticError::Store(format!("SQLite create table error: {e}")))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

type RowTuple = (String, String, String, String, String);

/// Helper to query rows from a prepared statement.
fn collect_rows(
    stmt: &mut rusqlite::Statement<'_>,
    params: &[&dyn rusqlite::types::ToSql],
) -> Result<Vec<RowTuple>, SynapticError> {
    let rows = stmt
        .query_map(params, |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| SynapticError::Store(format!("SQLite query error: {e}")))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| SynapticError::Store(format!("SQLite row error: {e}")))?;
    Ok(rows)
}

fn rows_to_items(rows: Vec<RowTuple>) -> Vec<Item> {
    rows.into_iter()
        .map(|(ns_str, k, val_str, created_at, updated_at)| {
            let value: Value = serde_json::from_str(&val_str).unwrap_or(Value::Null);
            Item {
                namespace: ns_str.split(':').map(String::from).collect(),
                key: k,
                value,
                created_at,
                updated_at,
                score: None,
            }
        })
        .collect()
}

#[async_trait]
impl synaptic_core::Store for SqliteStore {
    async fn get(&self, namespace: &[&str], key: &str) -> Result<Option<Item>, SynapticError> {
        let conn = self.conn.clone();
        let ns = encode_namespace(namespace);
        let key = key.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SynapticError::Store(format!("lock error: {e}")))?;

            let mut stmt = conn
                .prepare(
                    "SELECT namespace, key, value, created_at, updated_at
                     FROM synaptic_store WHERE namespace = ?1 AND key = ?2",
                )
                .map_err(|e| SynapticError::Store(format!("SQLite prepare error: {e}")))?;

            let result = stmt.query_row(rusqlite::params![ns, key], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            });

            match result {
                Ok((ns_str, k, val_str, created_at, updated_at)) => {
                    let value: Value = serde_json::from_str(&val_str).map_err(|e| {
                        SynapticError::Store(format!("JSON deserialize error: {e}"))
                    })?;
                    Ok(Some(Item {
                        namespace: ns_str.split(':').map(String::from).collect(),
                        key: k,
                        value,
                        created_at,
                        updated_at,
                        score: None,
                    }))
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(SynapticError::Store(format!("SQLite query error: {e}"))),
            }
        })
        .await
        .map_err(|e| SynapticError::Store(format!("spawn_blocking error: {e}")))?
    }

    async fn search(
        &self,
        namespace: &[&str],
        query: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Item>, SynapticError> {
        let conn = self.conn.clone();
        let ns = encode_namespace(namespace);
        let query = query.map(String::from);
        let limit = limit as i64;

        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SynapticError::Store(format!("lock error: {e}")))?;

            let rows: Vec<RowTuple> = match &query {
                Some(q) => {
                    // Try FTS5 MATCH first, fall back to LIKE on failure.
                    let fts_result: Result<Vec<RowTuple>, rusqlite::Error> = (|| {
                        let mut stmt = conn.prepare(
                            "SELECT s.namespace, s.key, s.value, s.created_at, s.updated_at
                             FROM synaptic_store s
                             JOIN synaptic_store_fts f ON s.namespace = f.namespace AND s.key = f.key
                             WHERE f.namespace = ?1 AND synaptic_store_fts MATCH ?2
                             LIMIT ?3",
                        )?;
                        let rows = stmt
                            .query_map(rusqlite::params![ns, q, limit], |row| {
                                Ok((
                                    row.get::<_, String>(0)?,
                                    row.get::<_, String>(1)?,
                                    row.get::<_, String>(2)?,
                                    row.get::<_, String>(3)?,
                                    row.get::<_, String>(4)?,
                                ))
                            })?
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok(rows)
                    })();

                    match fts_result {
                        Ok(rows) => rows,
                        Err(_) => {
                            // Fall back to LIKE
                            let like_pattern = format!("%{q}%");
                            let mut stmt = conn
                                .prepare(
                                    "SELECT namespace, key, value, created_at, updated_at
                                     FROM synaptic_store
                                     WHERE namespace = ?1 AND (key LIKE ?2 OR value LIKE ?2)
                                     LIMIT ?3",
                                )
                                .map_err(|e| {
                                    SynapticError::Store(format!("SQLite prepare error: {e}"))
                                })?;
                            collect_rows(
                                &mut stmt,
                                &[
                                    &ns as &dyn rusqlite::types::ToSql,
                                    &like_pattern,
                                    &limit,
                                ],
                            )?
                        }
                    }
                }
                None => {
                    let mut stmt = conn
                        .prepare(
                            "SELECT namespace, key, value, created_at, updated_at
                             FROM synaptic_store WHERE namespace = ?1 LIMIT ?2",
                        )
                        .map_err(|e| SynapticError::Store(format!("SQLite prepare error: {e}")))?;
                    collect_rows(
                        &mut stmt,
                        &[&ns as &dyn rusqlite::types::ToSql, &limit],
                    )?
                }
            };

            Ok(rows_to_items(rows))
        })
        .await
        .map_err(|e| SynapticError::Store(format!("spawn_blocking error: {e}")))?
    }

    async fn put(&self, namespace: &[&str], key: &str, value: Value) -> Result<(), SynapticError> {
        let conn = self.conn.clone();
        let ns = encode_namespace(namespace);
        let key = key.to_string();
        let value_str = serde_json::to_string(&value)
            .map_err(|e| SynapticError::Store(format!("JSON serialize error: {e}")))?;

        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SynapticError::Store(format!("lock error: {e}")))?;

            // Check existing to preserve created_at
            let existing_created_at: Option<String> = conn
                .prepare("SELECT created_at FROM synaptic_store WHERE namespace = ?1 AND key = ?2")
                .and_then(|mut stmt| stmt.query_row(rusqlite::params![ns, key], |row| row.get(0)))
                .ok();

            let now = now_iso();
            let created_at = existing_created_at.unwrap_or_else(|| now.clone());

            conn.execute(
                "INSERT OR REPLACE INTO synaptic_store (namespace, key, value, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![ns, key, value_str, created_at, now],
            )
            .map_err(|e| SynapticError::Store(format!("SQLite insert error: {e}")))?;

            // Sync FTS: delete old entry then insert new
            conn.execute(
                "DELETE FROM synaptic_store_fts WHERE namespace = ?1 AND key = ?2",
                rusqlite::params![ns, key],
            )
            .map_err(|e| SynapticError::Store(format!("SQLite FTS delete error: {e}")))?;

            conn.execute(
                "INSERT INTO synaptic_store_fts (key, value, namespace) VALUES (?1, ?2, ?3)",
                rusqlite::params![key, value_str, ns],
            )
            .map_err(|e| SynapticError::Store(format!("SQLite FTS insert error: {e}")))?;

            Ok(())
        })
        .await
        .map_err(|e| SynapticError::Store(format!("spawn_blocking error: {e}")))?
    }

    async fn delete(&self, namespace: &[&str], key: &str) -> Result<(), SynapticError> {
        let conn = self.conn.clone();
        let ns = encode_namespace(namespace);
        let key = key.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SynapticError::Store(format!("lock error: {e}")))?;

            conn.execute(
                "DELETE FROM synaptic_store WHERE namespace = ?1 AND key = ?2",
                rusqlite::params![ns, key],
            )
            .map_err(|e| SynapticError::Store(format!("SQLite delete error: {e}")))?;

            conn.execute(
                "DELETE FROM synaptic_store_fts WHERE namespace = ?1 AND key = ?2",
                rusqlite::params![ns, key],
            )
            .map_err(|e| SynapticError::Store(format!("SQLite FTS delete error: {e}")))?;

            Ok(())
        })
        .await
        .map_err(|e| SynapticError::Store(format!("spawn_blocking error: {e}")))?
    }

    async fn list_namespaces(&self, prefix: &[&str]) -> Result<Vec<Vec<String>>, SynapticError> {
        let conn = self.conn.clone();
        let prefix_str = if prefix.is_empty() {
            String::new()
        } else {
            prefix.join(":")
        };

        tokio::task::spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SynapticError::Store(format!("lock error: {e}")))?;

            let raw_namespaces: Vec<String> = if prefix_str.is_empty() {
                let mut stmt = conn
                    .prepare("SELECT DISTINCT namespace FROM synaptic_store")
                    .map_err(|e| SynapticError::Store(format!("SQLite prepare error: {e}")))?;
                let v: Vec<String> = stmt
                    .query_map([], |row| row.get::<_, String>(0))
                    .map_err(|e| SynapticError::Store(format!("SQLite query error: {e}")))?
                    .filter_map(|r| r.ok())
                    .collect();
                v
            } else {
                let like_pattern = format!("{prefix_str}%");
                let mut stmt = conn
                    .prepare(
                        "SELECT DISTINCT namespace FROM synaptic_store WHERE namespace LIKE ?1",
                    )
                    .map_err(|e| SynapticError::Store(format!("SQLite prepare error: {e}")))?;
                let v: Vec<String> = stmt
                    .query_map(rusqlite::params![like_pattern], |row| {
                        row.get::<_, String>(0)
                    })
                    .map_err(|e| SynapticError::Store(format!("SQLite query error: {e}")))?
                    .filter_map(|r| r.ok())
                    .collect();
                v
            };

            let namespaces: Vec<Vec<String>> = raw_namespaces
                .into_iter()
                .map(|ns| ns.split(':').map(String::from).collect())
                .collect();

            Ok(namespaces)
        })
        .await
        .map_err(|e| SynapticError::Store(format!("spawn_blocking error: {e}")))?
    }
}

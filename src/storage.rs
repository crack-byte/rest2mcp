use std::{path::Path, sync::{Arc, Mutex}};

#[cfg(not(test))]
use std::env;

use rusqlite::{params, Connection, OptionalExtension};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeLogEntry {
    pub timestamp: String,
    pub request: String,
    pub tool: String,
    pub backend: String,
    pub outcome: String,
    pub http_status: Option<u16>,
    pub duration_ms: u128,
}

#[derive(Debug, Clone)]
pub struct StoredSpec {
    pub name: String,
    pub raw: String,
    pub tool_count: usize,
    pub version: String,
    pub updated_at: String,
    pub api_base_url: String,
}

#[derive(Clone)]
pub struct SqliteStore {
    connection: Arc<Mutex<Connection>>,
    saved_specs_limit: usize,
    runtime_log_limit: usize,
}

impl SqliteStore {
    #[cfg(test)]
    pub fn open(path: impl AsRef<Path>) -> Result<Self, rusqlite::Error> {
        Self::open_with_limits(path, 10, 1000)
    }

    pub fn open_with_limits(
        path: impl AsRef<Path>,
        saved_specs_limit: usize,
        runtime_log_limit: usize,
    ) -> Result<Self, rusqlite::Error> {
        let path = path.as_ref();
        if path != Path::new(":memory:") {
            if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)
                    .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            }
        }
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS specs (
                 name TEXT PRIMARY KEY NOT NULL,
                 raw TEXT NOT NULL,
                 tool_count INTEGER NOT NULL,
                 version TEXT NOT NULL,
                 updated_at TEXT NOT NULL,
                 api_base_url TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS settings (
                 key TEXT PRIMARY KEY NOT NULL,
                 value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS runtime_logs (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 timestamp TEXT NOT NULL,
                 request TEXT NOT NULL,
                 tool TEXT NOT NULL,
                 backend TEXT NOT NULL,
                 outcome TEXT NOT NULL,
                 http_status INTEGER,
                 duration_ms INTEGER NOT NULL
             );",
        )?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            saved_specs_limit: saved_specs_limit.max(1),
            runtime_log_limit: runtime_log_limit.max(1),
        })
    }

    pub fn open_default(saved_specs_limit: usize, runtime_log_limit: usize) -> Result<Self, rusqlite::Error> {
        #[cfg(test)]
        let path = ":memory:".to_string();
        #[cfg(not(test))]
        let path = env::var("REST2MCP_DB_PATH").unwrap_or_else(|_| "rest2mcp.sqlite3".to_string());
        Self::open_with_limits(path, saved_specs_limit, runtime_log_limit)
    }

    pub fn save_spec(&self, spec: &StoredSpec) -> Result<(), rusqlite::Error> {
        let connection = self.connection.lock().expect("SQLite connection lock poisoned");
        connection.execute(
            "INSERT INTO specs (name, raw, tool_count, version, updated_at, api_base_url)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(name) DO UPDATE SET raw=excluded.raw, tool_count=excluded.tool_count,
               version=excluded.version, updated_at=excluded.updated_at, api_base_url=excluded.api_base_url",
            params![spec.name, spec.raw, spec.tool_count, spec.version, spec.updated_at, spec.api_base_url],
        )?;
        connection.execute(
            "DELETE FROM specs WHERE name NOT IN (SELECT name FROM specs ORDER BY updated_at DESC LIMIT ?1)",
            [self.saved_specs_limit as i64],
        )?;
        Ok(())
    }

    pub fn list_specs(&self) -> Result<Vec<StoredSpec>, rusqlite::Error> {
        let connection = self.connection.lock().expect("SQLite connection lock poisoned");
        let mut statement = connection.prepare(
            "SELECT name, raw, tool_count, version, updated_at, api_base_url FROM specs ORDER BY updated_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(StoredSpec {
                name: row.get(0)?,
                raw: row.get(1)?,
                tool_count: row.get::<_, i64>(2)?.max(0) as usize,
                version: row.get(3)?,
                updated_at: row.get(4)?,
                api_base_url: row.get(5)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_spec(&self, name: &str) -> Result<Option<StoredSpec>, rusqlite::Error> {
        let connection = self.connection.lock().expect("SQLite connection lock poisoned");
        connection.query_row(
            "SELECT name, raw, tool_count, version, updated_at, api_base_url FROM specs WHERE name=?1",
            [name],
            |row| Ok(StoredSpec {
                name: row.get(0)?,
                raw: row.get(1)?,
                tool_count: row.get::<_, i64>(2)?.max(0) as usize,
                version: row.get(3)?,
                updated_at: row.get(4)?,
                api_base_url: row.get(5)?,
            }),
        ).optional()
    }

    pub fn delete_spec(&self, name: &str) -> Result<bool, rusqlite::Error> {
        let connection = self.connection.lock().expect("SQLite connection lock poisoned");
        Ok(connection.execute("DELETE FROM specs WHERE name=?1", [name])? > 0)
    }

    pub fn set_active_spec(&self, name: &str) -> Result<(), rusqlite::Error> {
        let connection = self.connection.lock().expect("SQLite connection lock poisoned");
        connection.execute(
            "INSERT INTO settings (key, value) VALUES ('active_spec', ?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [name],
        )?;
        Ok(())
    }

    pub fn active_spec(&self) -> Result<Option<String>, rusqlite::Error> {
        let connection = self.connection.lock().expect("SQLite connection lock poisoned");
        connection.query_row("SELECT value FROM settings WHERE key='active_spec'", [], |row| row.get(0)).optional()
    }

    pub fn add_log(&self, entry: &RuntimeLogEntry) -> Result<(), rusqlite::Error> {
        let connection = self.connection.lock().expect("SQLite connection lock poisoned");
        connection.execute(
            "INSERT INTO runtime_logs (timestamp, request, tool, backend, outcome, http_status, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![entry.timestamp, entry.request, entry.tool, entry.backend, entry.outcome,
                entry.http_status, entry.duration_ms.min(i64::MAX as u128) as i64],
        )?;
        connection.execute(
            "DELETE FROM runtime_logs WHERE id NOT IN (SELECT id FROM runtime_logs ORDER BY id DESC LIMIT ?1)",
            [self.runtime_log_limit.min(i64::MAX as usize) as i64],
        )?;
        Ok(())
    }

    pub fn recent_logs(&self, limit: usize) -> Result<Vec<RuntimeLogEntry>, rusqlite::Error> {
        let connection = self.connection.lock().expect("SQLite connection lock poisoned");
        let mut statement = connection.prepare(
              "SELECT timestamp, request, tool, backend, outcome, http_status, duration_ms
               FROM runtime_logs ORDER BY id DESC LIMIT ?1",
        )?;
           let rows = statement.query_map([limit.min(self.runtime_log_limit).min(i64::MAX as usize) as i64], |row| {
            Ok(RuntimeLogEntry {
                timestamp: row.get(0)?,
                request: row.get(1)?,
                tool: row.get(2)?,
                backend: row.get(3)?,
                outcome: row.get(4)?,
                http_status: row.get(5)?,
                duration_ms: row.get::<_, i64>(6)?.max(0) as u128,
            })
        })?;
        rows.collect()
    }

    pub fn clear_logs(&self) -> Result<usize, rusqlite::Error> {
        let connection = self.connection.lock().expect("SQLite connection lock poisoned");
        Ok(connection.execute("DELETE FROM runtime_logs", [])?)
    }
}
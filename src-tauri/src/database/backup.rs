//! SQLite backup, SQL transfer, and restore support.

use super::{lock_conn, Database, SCHEMA_VERSION};
use crate::config::get_app_config_dir;
use crate::error::AppError;
use chrono::{Local, Utc};
use rusqlite::{backup::Backup, types::ValueRef, Connection};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

const SQL_EXPORT_HEADER: &str = "-- CC Switch SQLite export";

// Runtime and machine-local data is not transferred between devices.
const SYNC_SKIP_TABLES: &[&str] = &[
    "managed_auth_accounts",
    "proxy_request_logs",
    "provider_health",
    "usage_daily_rollups",
    "session_log_sync",
    "stream_check_logs",
];
const SYNC_PRESERVE_TABLES: &[&str] = &[
    "managed_auth_accounts",
    "proxy_request_logs",
    "usage_daily_rollups",
    "session_log_sync",
    "stream_check_logs",
];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub filename: String,
    pub size_bytes: u64,
    pub created_at: String,
}

impl Database {
    pub fn export_sql_string(&self) -> Result<String, AppError> {
        let snapshot = self.snapshot_to_memory()?;
        Self::dump_sql(&snapshot, &[])
    }

    pub fn export_sql_string_for_sync(&self) -> Result<String, AppError> {
        let snapshot = self.snapshot_to_memory()?;
        Self::dump_sql(&snapshot, SYNC_SKIP_TABLES)
    }

    pub fn export_sql(&self, target_path: &Path) -> Result<(), AppError> {
        let dump = self.export_sql_string()?;
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| AppError::io(parent, error))?;
        }
        crate::config::atomic_write(target_path, dump.as_bytes())
    }

    pub fn import_sql(&self, source_path: &Path) -> Result<String, AppError> {
        if !source_path.is_file() {
            return Err(AppError::InvalidInput(
                "SQL backup does not exist".to_string(),
            ));
        }
        let sql =
            fs::read_to_string(source_path).map_err(|error| AppError::io(source_path, error))?;
        self.import_sql_string(&sql)
    }

    pub fn import_sql_string(&self, sql: &str) -> Result<String, AppError> {
        self.import_sql_string_inner(sql, &[])
    }

    pub(crate) fn import_sql_string_for_sync(&self, sql: &str) -> Result<String, AppError> {
        self.import_sql_string_inner(sql, SYNC_PRESERVE_TABLES)
    }

    fn import_sql_string_inner(
        &self,
        sql: &str,
        preserve_tables: &[&str],
    ) -> Result<String, AppError> {
        let _auto_sync_guard = crate::webdav_auto_sync::AutoSyncSuppressionGuard::new();
        let sql = sql.trim_start_matches('\u{feff}');
        Self::validate_sql_header(sql)?;

        let safety_backup = self.backup_database_file()?;
        let local_snapshot = if preserve_tables.is_empty() {
            None
        } else {
            Some(self.snapshot_to_memory()?)
        };

        let temp_file = NamedTempFile::new().map_err(|source| AppError::IoContext {
            context: "Failed to create temporary database".to_string(),
            source,
        })?;
        let temp_conn = Connection::open(temp_file.path())
            .map_err(|error| AppError::Database(error.to_string()))?;
        temp_conn.execute_batch(sql).map_err(|error| {
            AppError::Database(format!("Failed to execute SQL import: {error}"))
        })?;

        Self::prepare_imported_connection(&temp_conn)?;
        if let Some(local) = local_snapshot.as_ref() {
            Self::restore_tables(local, &temp_conn, preserve_tables)?;
        }
        Self::replace_main_from_connection(self, &temp_conn)?;

        Ok(Self::backup_id(safety_backup))
    }

    pub(crate) fn snapshot_to_memory(&self) -> Result<Connection, AppError> {
        let conn = lock_conn!(self.conn);
        let mut snapshot =
            Connection::open_in_memory().map_err(|error| AppError::Database(error.to_string()))?;
        let backup = Backup::new(&conn, &mut snapshot)
            .map_err(|error| AppError::Database(error.to_string()))?;
        backup
            .step(-1)
            .map_err(|error| AppError::Database(error.to_string()))?;
        drop(backup);
        Ok(snapshot)
    }

    pub fn backup_database_file(&self) -> Result<Option<PathBuf>, AppError> {
        let Some(db_path) = self.connection_database_path()? else {
            return Ok(None);
        };
        if !db_path.is_file() {
            return Ok(None);
        }

        let backup_dir = Self::backup_dir()?;
        fs::create_dir_all(&backup_dir).map_err(|error| AppError::io(&backup_dir, error))?;
        let base = format!("db_backup_{}", Local::now().format("%Y%m%d_%H%M%S"));
        let mut suffix = 0u32;
        let backup_path = loop {
            let filename = if suffix == 0 {
                format!("{base}.db")
            } else {
                format!("{base}_{suffix}.db")
            };
            let candidate = backup_dir.join(filename);
            if !candidate.exists() {
                break candidate;
            }
            suffix += 1;
        };

        {
            let conn = lock_conn!(self.conn);
            let mut destination = Connection::open(&backup_path)
                .map_err(|error| AppError::Database(error.to_string()))?;
            let backup = Backup::new(&conn, &mut destination)
                .map_err(|error| AppError::Database(error.to_string()))?;
            backup
                .step(-1)
                .map_err(|error| AppError::Database(error.to_string()))?;
        }

        Self::cleanup_db_backups(&backup_dir)?;
        Ok(Some(backup_path))
    }

    pub fn periodic_backup_if_needed(&self) -> Result<(), AppError> {
        let interval_hours = crate::settings::effective_backup_interval_hours();
        if interval_hours == 0 || self.connection_database_path()?.is_none() {
            return Ok(());
        }

        let backup_dir = Self::backup_dir()?;
        let latest = fs::read_dir(&backup_dir).ok().and_then(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "db"))
                .filter_map(|entry| entry.metadata().ok()?.modified().ok())
                .max()
        });
        let due = latest.map_or(true, |modified| {
            modified.elapsed().unwrap_or_default().as_secs() >= u64::from(interval_hours) * 60 * 60
        });
        if due {
            self.backup_database_file()?;
        }
        Ok(())
    }

    pub fn list_backups() -> Result<Vec<BackupEntry>, AppError> {
        let backup_dir = Self::backup_dir()?;
        if !backup_dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = fs::read_dir(&backup_dir)
            .map_err(|error| AppError::io(&backup_dir, error))?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "db"))
            .filter_map(|entry| {
                let metadata = entry.metadata().ok()?;
                let modified = metadata.modified().ok()?;
                let created_at: chrono::DateTime<Utc> = modified.into();
                Some(BackupEntry {
                    filename: entry.file_name().to_string_lossy().into_owned(),
                    size_bytes: metadata.len(),
                    created_at: created_at.to_rfc3339(),
                })
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(entries)
    }

    pub fn restore_from_backup(&self, filename: &str) -> Result<String, AppError> {
        Self::validate_backup_filename(filename)?;
        let backup_path = Self::backup_dir()?.join(filename);
        if !backup_path.is_file() {
            return Err(AppError::InvalidInput(format!(
                "Backup file not found: {filename}"
            )));
        }

        // Work on a temporary copy so migrations never mutate the selected backup.
        let source = Connection::open(&backup_path)
            .map_err(|error| AppError::Database(error.to_string()))?;
        let temp_file = NamedTempFile::new().map_err(|source| AppError::IoContext {
            context: "Failed to create temporary restore database".to_string(),
            source,
        })?;
        let mut temp_conn = Connection::open(temp_file.path())
            .map_err(|error| AppError::Database(error.to_string()))?;
        {
            let backup = Backup::new(&source, &mut temp_conn)
                .map_err(|error| AppError::Database(error.to_string()))?;
            backup
                .step(-1)
                .map_err(|error| AppError::Database(error.to_string()))?;
        }
        Self::prepare_imported_connection(&temp_conn)?;

        let safety_backup = self.backup_database_file()?;
        Self::replace_main_from_connection(self, &temp_conn)?;
        Ok(Self::backup_id(safety_backup))
    }

    pub fn rename_backup(old_filename: &str, new_name: &str) -> Result<String, AppError> {
        Self::validate_backup_filename(old_filename)?;
        let trimmed = new_name.trim().trim_end_matches(".db");
        if trimmed.is_empty() || trimmed.len() > 100 {
            return Err(AppError::InvalidInput(
                "Backup name must contain 1 to 100 characters".to_string(),
            ));
        }
        if trimmed.contains("..")
            || trimmed.contains('/')
            || trimmed.contains('\\')
            || trimmed.contains('\0')
        {
            return Err(AppError::InvalidInput(
                "Backup name contains invalid characters".to_string(),
            ));
        }

        let new_filename = format!("{trimmed}.db");
        let backup_dir = Self::backup_dir()?;
        let old_path = backup_dir.join(old_filename);
        let new_path = backup_dir.join(&new_filename);
        if !old_path.is_file() {
            return Err(AppError::InvalidInput(format!(
                "Backup file not found: {old_filename}"
            )));
        }
        if new_path.exists() {
            return Err(AppError::InvalidInput(format!(
                "Backup already exists: {new_filename}"
            )));
        }
        fs::rename(&old_path, &new_path).map_err(|error| AppError::io(&old_path, error))?;
        Ok(new_filename)
    }

    pub fn delete_backup(filename: &str) -> Result<(), AppError> {
        Self::validate_backup_filename(filename)?;
        let path = Self::backup_dir()?.join(filename);
        if !path.is_file() {
            return Err(AppError::InvalidInput(format!(
                "Backup file not found: {filename}"
            )));
        }
        fs::remove_file(&path).map_err(|error| AppError::io(&path, error))
    }

    fn prepare_imported_connection(conn: &Connection) -> Result<(), AppError> {
        let version: i32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|error| AppError::Database(error.to_string()))?;
        if version > SCHEMA_VERSION {
            return Err(AppError::Database(format!(
                "Backup schema version {version} is newer than supported {SCHEMA_VERSION}"
            )));
        }
        Self::create_tables_on_conn(conn)?;
        Self::apply_schema_migrations_on_conn(conn)?;
        Self::seed_model_pricing_on_conn(conn)?;
        Self::validate_connection(conn)
    }

    fn validate_connection(conn: &Connection) -> Result<(), AppError> {
        let integrity: String = conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(|error| AppError::Database(error.to_string()))?;
        if integrity != "ok" {
            return Err(AppError::Database(format!(
                "Imported database integrity check failed: {integrity}"
            )));
        }
        for table in ["providers", "mcp_servers", "settings"] {
            if !Self::backup_table_exists(conn, table)? {
                return Err(AppError::Database(format!(
                    "Imported database is missing required table: {table}"
                )));
            }
        }
        Ok(())
    }

    fn replace_main_from_connection(&self, source: &Connection) -> Result<(), AppError> {
        let mut main = lock_conn!(self.conn);
        let backup = Backup::new(source, &mut main)
            .map_err(|error| AppError::Database(error.to_string()))?;
        backup
            .step(-1)
            .map_err(|error| AppError::Database(error.to_string()))?;
        Ok(())
    }

    fn restore_tables(
        source: &Connection,
        target: &Connection,
        tables: &[&str],
    ) -> Result<(), AppError> {
        for table in tables {
            if !Self::backup_table_exists(source, table)?
                || !Self::backup_table_exists(target, table)?
            {
                continue;
            }
            let columns = Self::get_table_columns(target, table)?;
            if columns.is_empty() {
                continue;
            }
            target
                .execute(&format!("DELETE FROM \"{table}\""), [])
                .map_err(|error| AppError::Database(error.to_string()))?;
            let quoted = columns
                .iter()
                .map(|column| format!("\"{column}\""))
                .collect::<Vec<_>>()
                .join(", ");
            let placeholders = (1..=columns.len())
                .map(|index| format!("?{index}"))
                .collect::<Vec<_>>()
                .join(", ");
            let mut select = source
                .prepare(&format!("SELECT {quoted} FROM \"{table}\""))
                .map_err(|error| AppError::Database(error.to_string()))?;
            let mut rows = select
                .query([])
                .map_err(|error| AppError::Database(error.to_string()))?;
            while let Some(row) = rows
                .next()
                .map_err(|error| AppError::Database(error.to_string()))?
            {
                let values = (0..columns.len())
                    .map(|index| row.get::<_, rusqlite::types::Value>(index))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| AppError::Database(error.to_string()))?;
                target
                    .execute(
                        &format!("INSERT INTO \"{table}\" ({quoted}) VALUES ({placeholders})"),
                        rusqlite::params_from_iter(values.iter()),
                    )
                    .map_err(|error| AppError::Database(error.to_string()))?;
            }
        }
        Ok(())
    }

    fn dump_sql(conn: &Connection, skip_tables: &[&str]) -> Result<String, AppError> {
        let user_version: i32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap_or_default();
        let mut output = format!(
            "{SQL_EXPORT_HEADER}\n-- Generated at: {}\n-- user_version: {user_version}\nPRAGMA foreign_keys=OFF;\nPRAGMA user_version={user_version};\nBEGIN TRANSACTION;\n",
            Utc::now().format("%Y-%m-%d %H:%M:%S")
        );
        let mut statement = conn
            .prepare(
                "SELECT type, name, sql FROM sqlite_master
                 WHERE sql IS NOT NULL AND type IN ('table','index','trigger','view')
                 ORDER BY CASE type WHEN 'table' THEN 0 WHEN 'index' THEN 1
                          WHEN 'trigger' THEN 2 ELSE 3 END, name",
            )
            .map_err(|error| AppError::Database(error.to_string()))?;
        let objects = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|error| AppError::Database(error.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| AppError::Database(error.to_string()))?;

        let mut tables = Vec::new();
        for (object_type, name, sql) in objects {
            if name.starts_with("sqlite_") {
                continue;
            }
            output.push_str(&sql);
            output.push_str(";\n");
            if object_type == "table" {
                tables.push(name);
            }
        }
        for table in tables {
            if skip_tables.contains(&table.as_str()) {
                continue;
            }
            let columns = Self::get_table_columns(conn, &table)?;
            let quoted = columns
                .iter()
                .map(|column| format!("\"{column}\""))
                .collect::<Vec<_>>()
                .join(", ");
            let mut statement = conn
                .prepare(&format!("SELECT * FROM \"{table}\""))
                .map_err(|error| AppError::Database(error.to_string()))?;
            let mut rows = statement
                .query([])
                .map_err(|error| AppError::Database(error.to_string()))?;
            while let Some(row) = rows
                .next()
                .map_err(|error| AppError::Database(error.to_string()))?
            {
                let values = (0..columns.len())
                    .map(|index| row.get_ref(index).map(Self::format_sql_value))
                    .collect::<Result<Result<Vec<_>, _>, _>>()
                    .map_err(|error| AppError::Database(error.to_string()))??;
                output.push_str(&format!(
                    "INSERT INTO \"{table}\" ({quoted}) VALUES ({});\n",
                    values.join(", ")
                ));
            }
        }
        output.push_str("COMMIT;\nPRAGMA foreign_keys=ON;\n");
        Ok(output)
    }

    fn get_table_columns(conn: &Connection, table: &str) -> Result<Vec<String>, AppError> {
        let mut statement = conn
            .prepare(&format!("PRAGMA table_info(\"{table}\")"))
            .map_err(|error| AppError::Database(error.to_string()))?;
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|error| AppError::Database(error.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| AppError::Database(error.to_string()))?;
        Ok(columns)
    }

    fn format_sql_value(value: ValueRef<'_>) -> Result<String, AppError> {
        match value {
            ValueRef::Null => Ok("NULL".to_string()),
            ValueRef::Integer(value) => Ok(value.to_string()),
            ValueRef::Real(value) => Ok(value.to_string()),
            ValueRef::Text(value) => {
                let value = std::str::from_utf8(value).map_err(|error| {
                    AppError::Database(format!("Database text is not valid UTF-8: {error}"))
                })?;
                Ok(format!("'{}'", value.replace('\'', "''")))
            }
            ValueRef::Blob(value) => Ok(format!("X'{}'", hex::encode_upper(value))),
        }
    }

    fn validate_sql_header(sql: &str) -> Result<(), AppError> {
        if sql.trim_start().starts_with(SQL_EXPORT_HEADER) {
            return Ok(());
        }
        Err(AppError::InvalidInput(
            "Only SQL backups exported by CC Switch are supported".to_string(),
        ))
    }

    fn validate_backup_filename(filename: &str) -> Result<(), AppError> {
        if filename.is_empty()
            || filename.contains("..")
            || filename.contains('/')
            || filename.contains('\\')
            || !filename.ends_with(".db")
        {
            return Err(AppError::InvalidInput(
                "Invalid backup filename".to_string(),
            ));
        }
        Ok(())
    }

    fn backup_table_exists(conn: &Connection, table: &str) -> Result<bool, AppError> {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            [table],
            |row| row.get(0),
        )
        .map_err(|error| AppError::Database(error.to_string()))
    }

    fn connection_database_path(&self) -> Result<Option<PathBuf>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut statement = conn
            .prepare("PRAGMA database_list")
            .map_err(|error| AppError::Database(error.to_string()))?;
        let mut rows = statement
            .query([])
            .map_err(|error| AppError::Database(error.to_string()))?;
        while let Some(row) = rows
            .next()
            .map_err(|error| AppError::Database(error.to_string()))?
        {
            let name: String = row
                .get(1)
                .map_err(|error| AppError::Database(error.to_string()))?;
            if name == "main" {
                let path: String = row
                    .get(2)
                    .map_err(|error| AppError::Database(error.to_string()))?;
                return Ok((!path.is_empty()).then(|| PathBuf::from(path)));
            }
        }
        Ok(None)
    }

    fn backup_dir() -> Result<PathBuf, AppError> {
        Ok(get_app_config_dir()?.join("backups"))
    }

    fn backup_id(path: Option<PathBuf>) -> String {
        path.and_then(|path| {
            path.file_stem()
                .map(|value| value.to_string_lossy().into_owned())
        })
        .unwrap_or_default()
    }

    fn cleanup_db_backups(directory: &Path) -> Result<(), AppError> {
        let retain = crate::settings::effective_backup_retain_count();
        let mut entries = match fs::read_dir(directory) {
            Ok(entries) => entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "db"))
                .collect::<Vec<_>>(),
            Err(_) => return Ok(()),
        };
        entries.sort_by_key(|entry| entry.metadata().and_then(|value| value.modified()).ok());
        let remove_count = entries.len().saturating_sub(retain);
        for entry in entries.into_iter().take(remove_count) {
            let path = entry.path();
            if let Err(error) = fs::remove_file(&path) {
                log::warn!(
                    "Failed to remove old database backup {}: {error}",
                    path.display()
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn insert_provider(db: &Database, id: &str) -> Result<(), AppError> {
        let conn = lock_conn!(db.conn);
        conn.execute(
            "INSERT INTO providers (id, app_type, name, settings_config, meta)
             VALUES (?1, 'claude', ?1, '{}', '{}')",
            [id],
        )
        .map_err(|error| AppError::Database(error.to_string()))?;
        Ok(())
    }

    fn insert_stream_check_log(db: &Database, provider_id: &str) -> Result<(), AppError> {
        let conn = lock_conn!(db.conn);
        conn.execute(
            "INSERT INTO stream_check_logs (
                provider_id, provider_name, app_type, status, success, message,
                model_used, tested_at
             ) VALUES (?1, ?1, 'claude', 'success', 1, 'local diagnostic',
                       'test-model', 1)",
            [provider_id],
        )
        .map_err(|error| AppError::Database(error.to_string()))?;
        Ok(())
    }

    fn insert_managed_auth_account(
        db: &Database,
        id: &str,
        access_token: &str,
        refresh_token: &str,
    ) -> Result<(), AppError> {
        let conn = lock_conn!(db.conn);
        conn.execute(
            "INSERT INTO managed_auth_accounts (
                id, provider, label, is_default, created_at, updated_at,
                access_token, refresh_token, status
             ) VALUES (?1, 'codex_oauth', ?1, 1, '2026-07-13T00:00:00Z',
                       '2026-07-13T00:00:00Z', ?2, ?3, 'active')",
            rusqlite::params![id, access_token, refresh_token],
        )
        .map_err(|error| AppError::Database(error.to_string()))?;
        Ok(())
    }

    #[test]
    fn sql_export_import_round_trip() -> Result<(), AppError> {
        let source = Database::memory()?;
        insert_provider(&source, "remote")?;
        let sql = source.export_sql_string()?;
        assert!(sql.starts_with(SQL_EXPORT_HEADER));

        let target = Database::memory()?;
        insert_provider(&target, "local")?;
        target.import_sql_string(&sql)?;
        let conn = lock_conn!(target.conn);
        let remote: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM providers WHERE id='remote'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| AppError::Database(error.to_string()))?;
        let local: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM providers WHERE id='local'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| AppError::Database(error.to_string()))?;
        assert_eq!((remote, local), (1, 0));
        Ok(())
    }

    #[test]
    fn sync_import_preserves_local_usage_data() -> Result<(), AppError> {
        let source = Database::memory()?;
        insert_provider(&source, "remote")?;
        insert_managed_auth_account(
            &source,
            "remote-account",
            "remote-access-token-must-not-sync",
            "remote-refresh-token-must-not-sync",
        )?;
        let sql = source.export_sql_string_for_sync()?;
        assert!(!sql.contains("VALUES ('request-local'"));
        assert!(!sql.contains("remote-access-token-must-not-sync"));
        assert!(!sql.contains("remote-refresh-token-must-not-sync"));
        assert!(!sql.contains("INSERT INTO \"managed_auth_accounts\""));

        let target = Database::memory()?;
        insert_provider(&target, "local")?;
        insert_stream_check_log(&target, "check-local")?;
        insert_managed_auth_account(
            &target,
            "local-account",
            "local-access-token",
            "local-refresh-token",
        )?;
        {
            let conn = lock_conn!(target.conn);
            conn.execute(
                "INSERT INTO proxy_request_logs (
                    request_id, provider_id, app_type, model, latency_ms,
                    status_code, created_at
                 ) VALUES ('request-local', 'local', 'claude', 'test', 1, 200, 1)",
                [],
            )
            .map_err(|error| AppError::Database(error.to_string()))?;
        }
        target.import_sql_string_for_sync(&sql)?;
        let conn = lock_conn!(target.conn);
        let logs: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM proxy_request_logs WHERE request_id='request-local'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| AppError::Database(error.to_string()))?;
        assert_eq!(logs, 1);
        let stream_checks: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM stream_check_logs WHERE provider_id='check-local'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| AppError::Database(error.to_string()))?;
        assert_eq!(stream_checks, 1);
        let local_auth: (String, String) = conn
            .query_row(
                "SELECT access_token, refresh_token FROM managed_auth_accounts
                 WHERE provider='codex_oauth' AND id='local-account'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|error| AppError::Database(error.to_string()))?;
        assert_eq!(
            local_auth,
            (
                "local-access-token".to_string(),
                "local-refresh-token".to_string()
            )
        );
        Ok(())
    }

    #[test]
    fn sync_import_migrates_v6_and_preserves_local_stream_check_history() -> Result<(), AppError> {
        let source = Database::memory()?;
        insert_provider(&source, "remote-v6")?;
        {
            let conn = lock_conn!(source.conn);
            conn.execute_batch(
                "DROP TABLE stream_check_logs;
                 PRAGMA user_version = 6;",
            )
            .map_err(|error| AppError::Database(error.to_string()))?;
        }
        let sql = source.export_sql_string_for_sync()?;
        assert!(sql.contains("PRAGMA user_version=6"));
        assert!(!sql.contains("CREATE TABLE stream_check_logs"));

        let target = Database::memory()?;
        insert_stream_check_log(&target, "check-before-v6-restore")?;
        target.import_sql_string_for_sync(&sql)?;

        let conn = lock_conn!(target.conn);
        let version: i32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|error| AppError::Database(error.to_string()))?;
        let provider_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM providers WHERE id='remote-v6'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| AppError::Database(error.to_string()))?;
        let stream_check_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM stream_check_logs
                 WHERE provider_id='check-before-v6-restore'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| AppError::Database(error.to_string()))?;

        assert_eq!(version, SCHEMA_VERSION);
        assert_eq!(provider_count, 1);
        assert_eq!(stream_check_count, 1);
        Ok(())
    }

    #[test]
    fn rejects_untrusted_sql() -> Result<(), AppError> {
        let db = Database::memory()?;
        assert!(db.import_sql_string("DROP TABLE providers;").is_err());
        Ok(())
    }
}

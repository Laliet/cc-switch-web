use super::{lock_conn, Database, SCHEMA_VERSION};
use crate::error::AppError;
use rusqlite::Connection;

impl Database {
    pub(crate) fn create_tables(&self) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS providers (
                id TEXT NOT NULL,
                app_type TEXT NOT NULL,
                name TEXT NOT NULL,
                settings_config TEXT NOT NULL,
                website_url TEXT,
                category TEXT,
                created_at INTEGER,
                sort_index INTEGER,
                notes TEXT,
                meta TEXT NOT NULL DEFAULT '{}',
                is_current INTEGER NOT NULL DEFAULT 0,
                backup_current TEXT,
                PRIMARY KEY (id, app_type)
            );

            CREATE TABLE IF NOT EXISTS provider_endpoints (
                provider_id TEXT NOT NULL,
                app_type TEXT NOT NULL,
                url TEXT NOT NULL,
                added_at INTEGER NOT NULL,
                last_used INTEGER,
                PRIMARY KEY (provider_id, app_type, url),
                FOREIGN KEY (provider_id, app_type)
                    REFERENCES providers(id, app_type) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS mcp_servers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                server_config TEXT NOT NULL,
                description TEXT,
                homepage TEXT,
                docs TEXT,
                tags TEXT NOT NULL DEFAULT '[]',
                enabled_claude INTEGER NOT NULL DEFAULT 0,
                enabled_codex INTEGER NOT NULL DEFAULT 0,
                enabled_gemini INTEGER NOT NULL DEFAULT 0,
                enabled_opencode INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS prompts (
                id TEXT NOT NULL,
                app_type TEXT NOT NULL,
                name TEXT NOT NULL,
                content TEXT NOT NULL,
                description TEXT,
                enabled INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER,
                updated_at INTEGER,
                PRIMARY KEY (id, app_type)
            );

            CREATE TABLE IF NOT EXISTS skill_repos (
                owner TEXT NOT NULL,
                name TEXT NOT NULL,
                branch TEXT NOT NULL DEFAULT 'main',
                enabled INTEGER NOT NULL DEFAULT 1,
                skills_path TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (owner, name, branch, skills_path)
            );

            CREATE TABLE IF NOT EXISTS skill_states (
                state_key TEXT PRIMARY KEY,
                installed INTEGER NOT NULL DEFAULT 0,
                installed_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS skill_repo_cache (
                cache_key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT
            );

            CREATE TABLE IF NOT EXISTS proxy_config (
                app_type TEXT PRIMARY KEY,
                enabled INTEGER NOT NULL DEFAULT 0,
                auto_failover_enabled INTEGER NOT NULL DEFAULT 0,
                max_retries INTEGER NOT NULL DEFAULT 0,
                default_cost_multiplier TEXT NOT NULL DEFAULT '1',
                pricing_model_source TEXT NOT NULL DEFAULT 'response',
                host TEXT NOT NULL DEFAULT '127.0.0.1',
                port INTEGER NOT NULL DEFAULT 3456,
                upstream_proxy TEXT,
                bind_app TEXT NOT NULL DEFAULT 'claude',
                auto_start INTEGER NOT NULL DEFAULT 0,
                enable_logging INTEGER NOT NULL DEFAULT 0,
                live_takeover_active INTEGER NOT NULL DEFAULT 0,
                streaming_first_byte_timeout INTEGER NOT NULL DEFAULT 90,
                streaming_idle_timeout INTEGER NOT NULL DEFAULT 120,
                non_streaming_timeout INTEGER NOT NULL DEFAULT 180,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS provider_health (
                provider_id TEXT NOT NULL,
                app_type TEXT NOT NULL,
                is_healthy INTEGER NOT NULL DEFAULT 1,
                consecutive_failures INTEGER NOT NULL DEFAULT 0,
                last_success_at TEXT,
                last_failure_at TEXT,
                last_error TEXT,
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (provider_id, app_type)
            );

            CREATE TABLE IF NOT EXISTS proxy_request_logs (
                request_id TEXT PRIMARY KEY,
                provider_id TEXT NOT NULL,
                app_type TEXT NOT NULL,
                model TEXT NOT NULL,
                request_model TEXT,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                cache_read_tokens INTEGER NOT NULL DEFAULT 0,
                cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
                input_cost_usd TEXT NOT NULL DEFAULT '0',
                output_cost_usd TEXT NOT NULL DEFAULT '0',
                cache_read_cost_usd TEXT NOT NULL DEFAULT '0',
                cache_creation_cost_usd TEXT NOT NULL DEFAULT '0',
                total_cost_usd TEXT NOT NULL DEFAULT '0',
                latency_ms INTEGER NOT NULL DEFAULT 0,
                first_token_ms INTEGER,
                duration_ms INTEGER,
                status_code INTEGER NOT NULL DEFAULT 0,
                error_message TEXT,
                session_id TEXT,
                provider_type TEXT,
                is_streaming INTEGER NOT NULL DEFAULT 0,
                cost_multiplier TEXT NOT NULL DEFAULT '1.0',
                created_at INTEGER NOT NULL,
                data_source TEXT NOT NULL DEFAULT 'proxy'
            );
            CREATE INDEX IF NOT EXISTS idx_proxy_request_logs_created_at
                ON proxy_request_logs(created_at);
            CREATE INDEX IF NOT EXISTS idx_proxy_request_logs_provider
                ON proxy_request_logs(app_type, provider_id);
            CREATE INDEX IF NOT EXISTS idx_proxy_request_logs_model
                ON proxy_request_logs(model);
            CREATE INDEX IF NOT EXISTS idx_proxy_request_logs_session
                ON proxy_request_logs(session_id);
            CREATE INDEX IF NOT EXISTS idx_proxy_request_logs_status
                ON proxy_request_logs(status_code);

            CREATE TABLE IF NOT EXISTS usage_daily_rollups (
                date TEXT NOT NULL,
                app_type TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                model TEXT NOT NULL,
                request_count INTEGER NOT NULL DEFAULT 0,
                success_count INTEGER NOT NULL DEFAULT 0,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                cache_read_tokens INTEGER NOT NULL DEFAULT 0,
                cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
                total_cost_usd TEXT NOT NULL DEFAULT '0',
                avg_latency_ms INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (date, app_type, provider_id, model)
            );

            CREATE TABLE IF NOT EXISTS model_pricing (
                model_id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                input_cost_per_million TEXT NOT NULL,
                output_cost_per_million TEXT NOT NULL,
                cache_read_cost_per_million TEXT NOT NULL DEFAULT '0',
                cache_creation_cost_per_million TEXT NOT NULL DEFAULT '0'
            );

            CREATE TABLE IF NOT EXISTS failover_queue (
                app_type TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                position INTEGER NOT NULL,
                PRIMARY KEY (app_type, provider_id)
            );

            CREATE TABLE IF NOT EXISTS universal_providers (
                id TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(())
    }

    pub(crate) fn apply_schema_migrations(&self) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        let version: i32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|e| AppError::Database(e.to_string()))?;
        if version > SCHEMA_VERSION {
            return Err(AppError::Database(format!(
                "Database schema version {version} is newer than supported {SCHEMA_VERSION}"
            )));
        }
        if version < 2 {
            Self::migrate_v1_to_v2(&conn)?;
        }
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    fn migrate_v1_to_v2(conn: &Connection) -> Result<(), AppError> {
        for (column, definition) in [
            ("request_model", "TEXT"),
            ("cache_read_tokens", "INTEGER NOT NULL DEFAULT 0"),
            ("cache_creation_tokens", "INTEGER NOT NULL DEFAULT 0"),
            ("input_cost_usd", "TEXT NOT NULL DEFAULT '0'"),
            ("output_cost_usd", "TEXT NOT NULL DEFAULT '0'"),
            ("cache_read_cost_usd", "TEXT NOT NULL DEFAULT '0'"),
            ("cache_creation_cost_usd", "TEXT NOT NULL DEFAULT '0'"),
            ("first_token_ms", "INTEGER"),
            ("duration_ms", "INTEGER"),
            ("provider_type", "TEXT"),
            ("is_streaming", "INTEGER NOT NULL DEFAULT 0"),
            ("cost_multiplier", "TEXT NOT NULL DEFAULT '1.0'"),
        ] {
            Self::add_column_if_missing(conn, "proxy_request_logs", column, definition)?;
        }
        Self::add_column_if_missing(
            conn,
            "proxy_config",
            "default_cost_multiplier",
            "TEXT NOT NULL DEFAULT '1'",
        )?;
        Self::add_column_if_missing(
            conn,
            "proxy_config",
            "pricing_model_source",
            "TEXT NOT NULL DEFAULT 'response'",
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS usage_daily_rollups (
                date TEXT NOT NULL,
                app_type TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                model TEXT NOT NULL,
                request_count INTEGER NOT NULL DEFAULT 0,
                success_count INTEGER NOT NULL DEFAULT 0,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                cache_read_tokens INTEGER NOT NULL DEFAULT 0,
                cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
                total_cost_usd TEXT NOT NULL DEFAULT '0',
                avg_latency_ms INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (date, app_type, provider_id, model)
            )",
            [],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS model_pricing (
                model_id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                input_cost_per_million TEXT NOT NULL,
                output_cost_per_million TEXT NOT NULL,
                cache_read_cost_per_million TEXT NOT NULL DEFAULT '0',
                cache_creation_cost_per_million TEXT NOT NULL DEFAULT '0'
            )",
            [],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_proxy_request_logs_model
             ON proxy_request_logs(model)",
            [],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_proxy_request_logs_session
             ON proxy_request_logs(session_id)",
            [],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_proxy_request_logs_status
             ON proxy_request_logs(status_code)",
            [],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub(crate) fn seed_model_pricing(&self) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        Self::seed_model_pricing_on_conn(&conn)
    }

    fn seed_model_pricing_on_conn(conn: &Connection) -> Result<(), AppError> {
        let pricing_data = [
            (
                "claude-sonnet-4-20250514",
                "Claude Sonnet 4",
                "3",
                "15",
                "0.30",
                "3.75",
            ),
            (
                "claude-opus-4-20250514",
                "Claude Opus 4",
                "15",
                "75",
                "1.50",
                "18.75",
            ),
            (
                "claude-3-5-sonnet-20241022",
                "Claude 3.5 Sonnet",
                "3",
                "15",
                "0.30",
                "3.75",
            ),
            ("gpt-5", "GPT-5", "1.25", "10", "0.125", "0"),
            ("gpt-5-low", "GPT-5", "1.25", "10", "0.125", "0"),
            ("gpt-5-medium", "GPT-5", "1.25", "10", "0.125", "0"),
            ("gpt-5-high", "GPT-5", "1.25", "10", "0.125", "0"),
            ("gpt-5-codex", "GPT-5 Codex", "1.25", "10", "0.125", "0"),
            ("gpt-4.1", "GPT-4.1", "2", "8", "0.50", "0"),
            ("gpt-4.1-mini", "GPT-4.1 Mini", "0.40", "1.60", "0.10", "0"),
            ("o3", "OpenAI o3", "2", "8", "0.50", "0"),
            ("o4-mini", "OpenAI o4-mini", "1.10", "4.40", "0.275", "0"),
            ("codex-mini", "Codex Mini", "0.75", "3", "0.025", "0"),
            (
                "gemini-2.5-pro",
                "Gemini 2.5 Pro",
                "1.25",
                "10",
                "0.125",
                "0",
            ),
            (
                "gemini-2.5-flash",
                "Gemini 2.5 Flash",
                "0.3",
                "2.5",
                "0.03",
                "0",
            ),
            (
                "gemini-2.0-flash",
                "Gemini 2.0 Flash",
                "0.10",
                "0.40",
                "0.025",
                "0",
            ),
        ];
        let mut stmt = conn
            .prepare(
                "INSERT OR IGNORE INTO model_pricing (
                    model_id, display_name, input_cost_per_million, output_cost_per_million,
                    cache_read_cost_per_million, cache_creation_cost_per_million
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .map_err(|e| AppError::Database(format!("prepare model pricing seed failed: {e}")))?;
        for (model_id, display_name, input, output, cache_read, cache_creation) in pricing_data {
            stmt.execute(rusqlite::params![
                model_id,
                display_name,
                input,
                output,
                cache_read,
                cache_creation
            ])
            .map_err(|e| AppError::Database(format!("seed model pricing failed: {e}")))?;
        }
        Ok(())
    }

    fn add_column_if_missing(
        conn: &Connection,
        table: &str,
        column: &str,
        definition: &str,
    ) -> Result<(), AppError> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(|e| AppError::Database(e.to_string()))?;
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| AppError::Database(e.to_string()))?;
        for existing in columns {
            if existing.map_err(|e| AppError::Database(e.to_string()))? == column {
                return Ok(());
            }
        }
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }
}

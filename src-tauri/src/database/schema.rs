use super::{lock_conn, Database, SCHEMA_VERSION};
use crate::error::AppError;
use rusqlite::Connection;

impl Database {
    pub(crate) fn create_tables(&self) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);

        Self::create_tables_on_conn(&conn)
    }

    pub(crate) fn create_tables_on_conn(conn: &Connection) -> Result<(), AppError> {
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
                installed_at TEXT NOT NULL,
                repo_owner TEXT,
                repo_name TEXT,
                repo_branch TEXT,
                skills_path TEXT
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
                non_streaming_timeout INTEGER NOT NULL DEFAULT 600,
                circuit_failure_threshold INTEGER NOT NULL DEFAULT 3,
                circuit_recovery_threshold INTEGER NOT NULL DEFAULT 2,
                circuit_recovery_wait_seconds INTEGER NOT NULL DEFAULT 60,
                circuit_error_rate_threshold REAL NOT NULL DEFAULT 80,
                circuit_min_requests INTEGER NOT NULL DEFAULT 10,
                rectify_thinking_signature INTEGER NOT NULL DEFAULT 1,
                rectify_thinking_budget INTEGER NOT NULL DEFAULT 1,
                optimizer_enabled INTEGER NOT NULL DEFAULT 0,
                optimizer_thinking INTEGER NOT NULL DEFAULT 1,
                optimizer_cache_injection INTEGER NOT NULL DEFAULT 1,
                optimizer_cache_ttl TEXT NOT NULL DEFAULT '1h',
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

            CREATE TABLE IF NOT EXISTS session_log_sync (
                file_path TEXT PRIMARY KEY,
                last_modified INTEGER NOT NULL DEFAULT 0,
                last_line_offset INTEGER NOT NULL DEFAULT 0,
                last_synced_at INTEGER NOT NULL DEFAULT 0
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

            CREATE TABLE IF NOT EXISTS managed_auth_accounts (
                id TEXT NOT NULL,
                provider TEXT NOT NULL,
                label TEXT NOT NULL,
                username TEXT,
                avatar_url TEXT,
                plan TEXT,
                is_default INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                last_used_at TEXT,
                expires_at TEXT,
                scopes TEXT,
                token_type TEXT,
                access_token TEXT NOT NULL,
                refresh_token TEXT,
                status TEXT,
                PRIMARY KEY (provider, id)
            );
            CREATE INDEX IF NOT EXISTS idx_managed_auth_accounts_provider_default
                ON managed_auth_accounts(provider, is_default);
            ",
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(())
    }

    pub(crate) fn apply_schema_migrations(&self) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        Self::apply_schema_migrations_on_conn(&conn)
    }

    pub(crate) fn apply_schema_migrations_on_conn(conn: &Connection) -> Result<(), AppError> {
        let version: i32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|e| AppError::Database(e.to_string()))?;
        if version > SCHEMA_VERSION {
            return Err(AppError::Database(format!(
                "Database schema version {version} is newer than supported {SCHEMA_VERSION}"
            )));
        }
        if version < 2 {
            Self::migrate_v1_to_v2(conn)?;
        }
        if version < 3 {
            Self::migrate_v2_to_v3(conn)?;
        }
        if version < 4 {
            Self::migrate_v3_to_v4(conn)?;
        }
        if version < 5 {
            Self::migrate_v4_to_v5(conn)?;
        }
        if version < 6 {
            Self::migrate_v5_to_v6(conn)?;
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
            ("data_source", "TEXT NOT NULL DEFAULT 'proxy'"),
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
        for (column, definition) in [
            ("circuit_failure_threshold", "INTEGER NOT NULL DEFAULT 3"),
            ("circuit_recovery_threshold", "INTEGER NOT NULL DEFAULT 2"),
            (
                "circuit_recovery_wait_seconds",
                "INTEGER NOT NULL DEFAULT 60",
            ),
            ("circuit_error_rate_threshold", "REAL NOT NULL DEFAULT 80"),
            ("rectify_thinking_signature", "INTEGER NOT NULL DEFAULT 1"),
            ("rectify_thinking_budget", "INTEGER NOT NULL DEFAULT 1"),
            ("optimizer_enabled", "INTEGER NOT NULL DEFAULT 0"),
            ("optimizer_thinking", "INTEGER NOT NULL DEFAULT 1"),
            ("optimizer_cache_injection", "INTEGER NOT NULL DEFAULT 1"),
            ("optimizer_cache_ttl", "TEXT NOT NULL DEFAULT '1h'"),
        ] {
            Self::add_column_if_missing(conn, "proxy_config", column, definition)?;
        }

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

    fn migrate_v2_to_v3(conn: &Connection) -> Result<(), AppError> {
        Self::add_column_if_missing(
            conn,
            "proxy_request_logs",
            "data_source",
            "TEXT NOT NULL DEFAULT 'proxy'",
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS session_log_sync (
                file_path TEXT PRIMARY KEY,
                last_modified INTEGER NOT NULL DEFAULT 0,
                last_line_offset INTEGER NOT NULL DEFAULT 0,
                last_synced_at INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    fn migrate_v3_to_v4(conn: &Connection) -> Result<(), AppError> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS managed_auth_accounts (
                id TEXT NOT NULL,
                provider TEXT NOT NULL,
                label TEXT NOT NULL,
                username TEXT,
                avatar_url TEXT,
                plan TEXT,
                is_default INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                last_used_at TEXT,
                expires_at TEXT,
                scopes TEXT,
                token_type TEXT,
                access_token TEXT NOT NULL,
                refresh_token TEXT,
                status TEXT,
                PRIMARY KEY (provider, id)
            )",
            [],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_managed_auth_accounts_provider_default
             ON managed_auth_accounts(provider, is_default)",
            [],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    fn migrate_v4_to_v5(conn: &Connection) -> Result<(), AppError> {
        for (column, definition) in [
            ("repo_owner", "TEXT"),
            ("repo_name", "TEXT"),
            ("repo_branch", "TEXT"),
            ("skills_path", "TEXT"),
        ] {
            Self::add_column_if_missing(conn, "skill_states", column, definition)?;
        }
        Ok(())
    }

    fn migrate_v5_to_v6(conn: &Connection) -> Result<(), AppError> {
        Self::add_column_if_missing(
            conn,
            "proxy_config",
            "circuit_min_requests",
            "INTEGER NOT NULL DEFAULT 10",
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO proxy_config (app_type)
             VALUES ('claude'), ('codex'), ('gemini'), ('opencode')",
            [],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        conn.execute(
            "UPDATE proxy_config
             SET streaming_first_byte_timeout = (SELECT streaming_first_byte_timeout FROM proxy_config WHERE app_type = 'global'),
                 streaming_idle_timeout = (SELECT streaming_idle_timeout FROM proxy_config WHERE app_type = 'global'),
                 non_streaming_timeout = (SELECT non_streaming_timeout FROM proxy_config WHERE app_type = 'global'),
                 circuit_failure_threshold = (SELECT circuit_failure_threshold FROM proxy_config WHERE app_type = 'global'),
                 circuit_recovery_threshold = (SELECT circuit_recovery_threshold FROM proxy_config WHERE app_type = 'global'),
                 circuit_recovery_wait_seconds = (SELECT circuit_recovery_wait_seconds FROM proxy_config WHERE app_type = 'global'),
                 circuit_error_rate_threshold = (SELECT circuit_error_rate_threshold FROM proxy_config WHERE app_type = 'global'),
                 circuit_min_requests = 10
             WHERE app_type IN ('claude', 'codex', 'gemini', 'opencode')
               AND EXISTS (SELECT 1 FROM proxy_config WHERE app_type = 'global')",
            [],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub(crate) fn seed_model_pricing(&self) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        Self::seed_model_pricing_on_conn(&conn)
    }

    pub(crate) fn seed_model_pricing_on_conn(conn: &Connection) -> Result<(), AppError> {
        let pricing_data = [
            (
                "claude-opus-4-7",
                "Claude Opus 4.7",
                "5",
                "25",
                "0.50",
                "6.25",
            ),
            (
                "claude-opus-4-6-20260206",
                "Claude Opus 4.6",
                "5",
                "25",
                "0.50",
                "6.25",
            ),
            (
                "claude-sonnet-4-6-20260217",
                "Claude Sonnet 4.6",
                "3",
                "15",
                "0.30",
                "3.75",
            ),
            (
                "claude-opus-4-5-20251101",
                "Claude Opus 4.5",
                "5",
                "25",
                "0.50",
                "6.25",
            ),
            (
                "claude-sonnet-4-5-20250929",
                "Claude Sonnet 4.5",
                "3",
                "15",
                "0.30",
                "3.75",
            ),
            (
                "claude-haiku-4-5-20251001",
                "Claude Haiku 4.5",
                "1",
                "5",
                "0.10",
                "1.25",
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
                "claude-opus-4-1-20250805",
                "Claude Opus 4.1",
                "15",
                "75",
                "1.50",
                "18.75",
            ),
            (
                "claude-sonnet-4-20250514",
                "Claude Sonnet 4",
                "3",
                "15",
                "0.30",
                "3.75",
            ),
            (
                "claude-3-5-haiku-20241022",
                "Claude 3.5 Haiku",
                "0.80",
                "4",
                "0.08",
                "1",
            ),
            (
                "claude-3-5-sonnet-20241022",
                "Claude 3.5 Sonnet",
                "3",
                "15",
                "0.30",
                "3.75",
            ),
            ("gpt-5.5", "GPT-5.5", "5", "30", "0.50", "0"),
            ("gpt-5.5-low", "GPT-5.5", "5", "30", "0.50", "0"),
            ("gpt-5.5-medium", "GPT-5.5", "5", "30", "0.50", "0"),
            ("gpt-5.5-high", "GPT-5.5", "5", "30", "0.50", "0"),
            ("gpt-5.5-xhigh", "GPT-5.5", "5", "30", "0.50", "0"),
            ("gpt-5.5-minimal", "GPT-5.5", "5", "30", "0.50", "0"),
            ("gpt-5.4", "GPT-5.4", "2.50", "15", "0.25", "0"),
            ("gpt-5.4-mini", "GPT-5.4 Mini", "0.75", "4.50", "0.075", "0"),
            ("gpt-5.4-nano", "GPT-5.4 Nano", "0.20", "1.25", "0.02", "0"),
            ("gpt-5.2", "GPT-5.2", "1.75", "14", "0.175", "0"),
            ("gpt-5.2-low", "GPT-5.2", "1.75", "14", "0.175", "0"),
            ("gpt-5.2-medium", "GPT-5.2", "1.75", "14", "0.175", "0"),
            ("gpt-5.2-high", "GPT-5.2", "1.75", "14", "0.175", "0"),
            ("gpt-5.2-xhigh", "GPT-5.2", "1.75", "14", "0.175", "0"),
            ("gpt-5.2-codex", "GPT-5.2 Codex", "1.75", "14", "0.175", "0"),
            (
                "gpt-5.2-codex-low",
                "GPT-5.2 Codex",
                "1.75",
                "14",
                "0.175",
                "0",
            ),
            (
                "gpt-5.2-codex-medium",
                "GPT-5.2 Codex",
                "1.75",
                "14",
                "0.175",
                "0",
            ),
            (
                "gpt-5.2-codex-high",
                "GPT-5.2 Codex",
                "1.75",
                "14",
                "0.175",
                "0",
            ),
            (
                "gpt-5.2-codex-xhigh",
                "GPT-5.2 Codex",
                "1.75",
                "14",
                "0.175",
                "0",
            ),
            ("gpt-5.3-codex", "GPT-5.3 Codex", "1.75", "14", "0.175", "0"),
            (
                "gpt-5.3-codex-low",
                "GPT-5.3 Codex",
                "1.75",
                "14",
                "0.175",
                "0",
            ),
            (
                "gpt-5.3-codex-medium",
                "GPT-5.3 Codex",
                "1.75",
                "14",
                "0.175",
                "0",
            ),
            (
                "gpt-5.3-codex-high",
                "GPT-5.3 Codex",
                "1.75",
                "14",
                "0.175",
                "0",
            ),
            (
                "gpt-5.3-codex-xhigh",
                "GPT-5.3 Codex",
                "1.75",
                "14",
                "0.175",
                "0",
            ),
            ("gpt-5.1", "GPT-5.1", "1.25", "10", "0.125", "0"),
            ("gpt-5.1-low", "GPT-5.1", "1.25", "10", "0.125", "0"),
            ("gpt-5.1-medium", "GPT-5.1", "1.25", "10", "0.125", "0"),
            ("gpt-5.1-high", "GPT-5.1", "1.25", "10", "0.125", "0"),
            ("gpt-5.1-minimal", "GPT-5.1", "1.25", "10", "0.125", "0"),
            ("gpt-5.1-codex", "GPT-5.1 Codex", "1.25", "10", "0.125", "0"),
            (
                "gpt-5.1-codex-mini",
                "GPT-5.1 Codex",
                "1.25",
                "10",
                "0.125",
                "0",
            ),
            (
                "gpt-5.1-codex-max",
                "GPT-5.1 Codex",
                "1.25",
                "10",
                "0.125",
                "0",
            ),
            (
                "gpt-5.1-codex-max-high",
                "GPT-5.1 Codex",
                "1.25",
                "10",
                "0.125",
                "0",
            ),
            (
                "gpt-5.1-codex-max-xhigh",
                "GPT-5.1 Codex",
                "1.25",
                "10",
                "0.125",
                "0",
            ),
            ("gpt-5", "GPT-5", "1.25", "10", "0.125", "0"),
            ("gpt-5-low", "GPT-5", "1.25", "10", "0.125", "0"),
            ("gpt-5-medium", "GPT-5", "1.25", "10", "0.125", "0"),
            ("gpt-5-high", "GPT-5", "1.25", "10", "0.125", "0"),
            ("gpt-5-minimal", "GPT-5", "1.25", "10", "0.125", "0"),
            ("gpt-5-codex", "GPT-5 Codex", "1.25", "10", "0.125", "0"),
            ("gpt-5-codex-low", "GPT-5 Codex", "1.25", "10", "0.125", "0"),
            (
                "gpt-5-codex-medium",
                "GPT-5 Codex",
                "1.25",
                "10",
                "0.125",
                "0",
            ),
            (
                "gpt-5-codex-high",
                "GPT-5 Codex",
                "1.25",
                "10",
                "0.125",
                "0",
            ),
            (
                "gpt-5-codex-mini",
                "GPT-5 Codex",
                "1.25",
                "10",
                "0.125",
                "0",
            ),
            (
                "gpt-5-codex-mini-medium",
                "GPT-5 Codex",
                "1.25",
                "10",
                "0.125",
                "0",
            ),
            (
                "gpt-5-codex-mini-high",
                "GPT-5 Codex",
                "1.25",
                "10",
                "0.125",
                "0",
            ),
            ("o3", "OpenAI o3", "2", "8", "0.50", "0"),
            ("o4-mini", "OpenAI o4-mini", "1.10", "4.40", "0.275", "0"),
            ("gpt-4.1", "GPT-4.1", "2", "8", "0.50", "0"),
            ("gpt-4.1-mini", "GPT-4.1 Mini", "0.40", "1.60", "0.10", "0"),
            ("gpt-4.1-nano", "GPT-4.1 Nano", "0.10", "0.40", "0.025", "0"),
            (
                "gemini-3.1-pro-preview",
                "Gemini 3.1 Pro Preview",
                "2",
                "12",
                "0.20",
                "0",
            ),
            (
                "gemini-3.1-flash-lite-preview",
                "Gemini 3.1 Flash Lite Preview",
                "0.25",
                "1.50",
                "0.025",
                "0",
            ),
            (
                "gemini-3-pro-preview",
                "Gemini 3 Pro Preview",
                "2",
                "12",
                "0.2",
                "0",
            ),
            (
                "gemini-3-flash-preview",
                "Gemini 3 Flash Preview",
                "0.5",
                "3",
                "0.05",
                "0",
            ),
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
                "gemini-2.5-flash-lite",
                "Gemini 2.5 Flash Lite",
                "0.10",
                "0.40",
                "0.01",
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
            (
                "step-3.5-flash",
                "Step 3.5 Flash",
                "0.10",
                "0.30",
                "0.02",
                "0",
            ),
            (
                "doubao-seed-code",
                "Doubao Seed Code",
                "0.17",
                "1.11",
                "0.02",
                "0",
            ),
            (
                "doubao-seed-2-0-pro",
                "Doubao Seed 2.0 Pro",
                "0.47",
                "2.37",
                "0",
                "0",
            ),
            (
                "doubao-seed-2-0-code",
                "Doubao Seed 2.0 Code",
                "0.47",
                "2.37",
                "0",
                "0",
            ),
            (
                "doubao-seed-2-0-lite",
                "Doubao Seed 2.0 Lite",
                "0.25",
                "2",
                "0",
                "0",
            ),
            (
                "doubao-seed-2-0-mini",
                "Doubao Seed 2.0 Mini",
                "0.03",
                "0.31",
                "0",
                "0",
            ),
            (
                "deepseek-v3.2",
                "DeepSeek V3.2",
                "0.28",
                "0.42",
                "0.028",
                "0",
            ),
            (
                "deepseek-v3.1",
                "DeepSeek V3.1",
                "0.55",
                "1.67",
                "0.055",
                "0",
            ),
            ("deepseek-v3", "DeepSeek V3", "0.28", "1.11", "0.028", "0"),
            (
                "deepseek-chat",
                "DeepSeek Chat",
                "0.27",
                "1.10",
                "0.07",
                "0",
            ),
            (
                "deepseek-reasoner",
                "DeepSeek Reasoner",
                "0.55",
                "2.19",
                "0.14",
                "0",
            ),
            (
                "deepseek-v4-flash",
                "DeepSeek V4 Flash",
                "0.14",
                "0.28",
                "0.028",
                "0",
            ),
            (
                "deepseek-v4-pro",
                "DeepSeek V4 Pro",
                "1.68",
                "3.36",
                "0.14",
                "0",
            ),
            (
                "kimi-k2-thinking",
                "Kimi K2 Thinking",
                "0.55",
                "2.20",
                "0.10",
                "0",
            ),
            ("kimi-k2-0905", "Kimi K2", "0.55", "2.20", "0.10", "0"),
            (
                "kimi-k2-turbo",
                "Kimi K2 Turbo",
                "1.11",
                "8.06",
                "0.14",
                "0",
            ),
            ("kimi-k2.5", "Kimi K2.5", "0.60", "2.50", "0.10", "0"),
            ("kimi-k2.6", "Kimi K2.6", "0.95", "4.00", "0.16", "0"),
            ("minimax-m2.1", "MiniMax M2.1", "0.27", "0.95", "0.03", "0"),
            (
                "minimax-m2.1-lightning",
                "MiniMax M2.1 Lightning",
                "0.27",
                "2.33",
                "0.03",
                "0",
            ),
            ("minimax-m2", "MiniMax M2", "0.27", "0.95", "0.03", "0"),
            ("minimax-m2.5", "MiniMax M2.5", "0.12", "0.95", "0.03", "0"),
            (
                "minimax-m2.5-lightning",
                "MiniMax M2.5 Lightning",
                "0.30",
                "2.40",
                "0.03",
                "0",
            ),
            (
                "minimax-m2.7",
                "MiniMax M2.7",
                "0.30",
                "1.20",
                "0.06",
                "0.375",
            ),
            (
                "minimax-m2.7-highspeed",
                "MiniMax M2.7 Highspeed",
                "0.60",
                "2.40",
                "0.06",
                "0.375",
            ),
            ("glm-4.7", "GLM-4.7", "0.39", "1.75", "0.04", "0"),
            ("glm-4.6", "GLM-4.6", "0.28", "1.11", "0.03", "0"),
            ("glm-5", "GLM-5", "0.72", "2.30", "0", "0"),
            ("glm-5.1", "GLM-5.1", "0.95", "3.15", "0", "0"),
            (
                "mimo-v2-flash",
                "MiMo V2 Flash",
                "0.09",
                "0.29",
                "0.009",
                "0",
            ),
            ("mimo-v2-pro", "MiMo V2 Pro", "1", "3", "0", "0"),
            ("qwen3.6-plus", "Qwen3.6 Plus", "0.325", "1.95", "0", "0"),
            ("qwen3.5-plus", "Qwen3.5 Plus", "0.26", "1.56", "0", "0"),
            ("qwen3-max", "Qwen3 Max", "0.78", "3.90", "0", "0"),
            (
                "qwen3-235b-a22b",
                "Qwen3 235B-A22B",
                "0.70",
                "8.40",
                "0",
                "0",
            ),
            (
                "qwen3-coder-plus",
                "Qwen3 Coder Plus",
                "0.65",
                "3.25",
                "0",
                "0",
            ),
            (
                "qwen3-coder-flash",
                "Qwen3 Coder Flash",
                "0.195",
                "0.975",
                "0",
                "0",
            ),
            (
                "qwen3-coder-next",
                "Qwen3 Coder Next",
                "0.12",
                "0.75",
                "0",
                "0",
            ),
            ("qwq-plus", "QwQ Plus", "0.80", "2.40", "0", "0"),
            ("qwq-32b", "QwQ 32B", "0.20", "0.60", "0", "0"),
            ("qwen3-32b", "Qwen3 32B", "0.16", "0.64", "0", "0"),
            (
                "grok-4.20-0309-reasoning",
                "Grok 4.20 Reasoning",
                "2",
                "6",
                "0.20",
                "0",
            ),
            (
                "grok-4.20-0309-non-reasoning",
                "Grok 4.20",
                "2",
                "6",
                "0.20",
                "0",
            ),
            (
                "grok-4-1-fast-reasoning",
                "Grok 4.1 Fast Reasoning",
                "0.20",
                "0.50",
                "0.05",
                "0",
            ),
            (
                "grok-4-1-fast-non-reasoning",
                "Grok 4.1 Fast",
                "0.20",
                "0.50",
                "0.05",
                "0",
            ),
            ("grok-4", "Grok 4", "3", "15", "0.75", "0"),
            (
                "grok-code-fast-1",
                "Grok Code Fast",
                "0.20",
                "1.50",
                "0.02",
                "0",
            ),
            ("grok-3", "Grok 3", "3", "15", "0.75", "0"),
            ("grok-3-mini", "Grok 3 Mini", "0.25", "0.50", "0.075", "0"),
            ("codestral-2508", "Codestral", "0.30", "0.90", "0.03", "0"),
            (
                "devstral-small-1.1",
                "Devstral Small 1.1",
                "0.07",
                "0.28",
                "0.01",
                "0",
            ),
            ("devstral-2-2512", "Devstral 2", "0.40", "0.90", "0.04", "0"),
            (
                "devstral-medium",
                "Devstral Medium",
                "0.40",
                "2",
                "0.04",
                "0",
            ),
            (
                "mistral-large-3-2512",
                "Mistral Large 3",
                "0.50",
                "1.50",
                "0.05",
                "0",
            ),
            (
                "mistral-medium-3.1",
                "Mistral Medium 3.1",
                "0.40",
                "2",
                "0.04",
                "0",
            ),
            (
                "mistral-small-3.2-24b",
                "Mistral Small 3.2",
                "0.075",
                "0.20",
                "0.01",
                "0",
            ),
            ("magistral-medium", "Magistral Medium", "2", "5", "0", "0"),
            ("command-a", "Cohere Command A", "2.50", "10", "0", "0"),
            (
                "command-r-plus",
                "Cohere Command R+",
                "2.50",
                "10",
                "0",
                "0",
            ),
            ("command-r", "Cohere Command R", "0.15", "0.60", "0", "0"),
            ("o3-pro", "OpenAI o3-pro", "20", "80", "0", "0"),
            ("o3-mini", "OpenAI o3-mini", "0.55", "2.20", "0.55", "0"),
            ("o1", "OpenAI o1", "15", "60", "7.50", "0"),
            ("o1-mini", "OpenAI o1-mini", "0.55", "2.20", "0.55", "0"),
            ("codex-mini", "Codex Mini", "0.75", "3", "0.025", "0"),
            ("gpt-5-mini", "GPT-5 Mini", "0.25", "2", "0.025", "0"),
            ("gpt-5-nano", "GPT-5 Nano", "0.05", "0.40", "0.005", "0"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_v4_skill_states_with_source_columns() {
        let conn = Connection::open_in_memory().expect("memory database");
        conn.execute_batch(
            "CREATE TABLE skill_states (
                state_key TEXT PRIMARY KEY,
                installed INTEGER NOT NULL DEFAULT 0,
                installed_at TEXT NOT NULL
             );
             CREATE TABLE proxy_config (
                app_type TEXT PRIMARY KEY,
                streaming_first_byte_timeout INTEGER NOT NULL DEFAULT 90,
                streaming_idle_timeout INTEGER NOT NULL DEFAULT 120,
                non_streaming_timeout INTEGER NOT NULL DEFAULT 600,
                circuit_failure_threshold INTEGER NOT NULL DEFAULT 3,
                circuit_recovery_threshold INTEGER NOT NULL DEFAULT 2,
                circuit_recovery_wait_seconds INTEGER NOT NULL DEFAULT 60,
                circuit_error_rate_threshold REAL NOT NULL DEFAULT 80
             );
             INSERT INTO proxy_config (
                app_type, streaming_first_byte_timeout, streaming_idle_timeout,
                non_streaming_timeout, circuit_failure_threshold,
                circuit_recovery_threshold, circuit_recovery_wait_seconds,
                circuit_error_rate_threshold
             ) VALUES ('global', 17, 23, 456, 7, 4, 88, 55);
             PRAGMA user_version = 4;",
        )
        .expect("create v4 schema");

        Database::apply_schema_migrations_on_conn(&conn).expect("migrate schema");
        let columns = conn
            .prepare("PRAGMA table_info(skill_states)")
            .expect("prepare table info")
            .query_map([], |row| row.get::<_, String>(1))
            .expect("read columns")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect columns");

        for expected in ["repo_owner", "repo_name", "repo_branch", "skills_path"] {
            assert!(columns.iter().any(|column| column == expected));
        }
        let version: i32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read schema version");
        assert_eq!(version, SCHEMA_VERSION);
        let migrated: (i64, i64, i64, i64, i64, i64, f64, i64) = conn
            .query_row(
                "SELECT streaming_first_byte_timeout, streaming_idle_timeout,
                        non_streaming_timeout, circuit_failure_threshold,
                        circuit_recovery_threshold, circuit_recovery_wait_seconds,
                        circuit_error_rate_threshold, circuit_min_requests
                 FROM proxy_config WHERE app_type = 'codex'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .expect("read migrated proxy config");
        assert_eq!(migrated, (17, 23, 456, 7, 4, 88, 55.0, 10));
    }
}

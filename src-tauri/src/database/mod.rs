//! SQLite persistence layer.
//!
//! The database is the authoritative runtime store.  `config.json` is treated
//! as a legacy import/export format so web/headless deployments can build on the
//! same storage direction as upstream cc-switch without losing existing data.

mod backup;
mod dao;
mod migration;
mod schema;

pub use dao::{
    FailoverQueueItem, ModelPricing, ModelPricingRecord, ProviderHealthRecord,
    ProxyRequestLogRecord, ProxyRequestUsageUpdate, PRICING_SOURCE_REQUEST,
    PRICING_SOURCE_RESPONSE,
};

use crate::{config::get_app_config_dir, error::AppError};
use rusqlite::Connection;
use serde::{de::DeserializeOwned, Serialize};
use std::{path::PathBuf, sync::Mutex};

pub(crate) const SCHEMA_VERSION: i32 = 2;
pub(crate) const SETTINGS_CONFIG_VERSION: &str = "config_version";
pub(crate) const SETTINGS_COMMON_SNIPPETS: &str = "common_config_snippets";
pub(crate) const SETTINGS_DB_MIGRATED_FROM_JSON: &str = "migrated_from_config_json";

macro_rules! lock_conn {
    ($mutex:expr) => {
        $mutex
            .lock()
            .map_err(|e| AppError::Database(format!("Mutex lock failed: {e}")))?
    };
}

pub(crate) use lock_conn;

pub(crate) fn to_json_string<T: Serialize>(value: &T) -> Result<String, AppError> {
    serde_json::to_string(value)
        .map_err(|e| AppError::Config(format!("JSON serialization failed: {e}")))
}

pub(crate) fn from_json_string<T: DeserializeOwned>(
    raw: &str,
    context: &str,
) -> Result<T, AppError> {
    serde_json::from_str(raw)
        .map_err(|e| AppError::Config(format!("Failed to parse {context}: {e}")))
}

pub struct Database {
    pub(crate) conn: Mutex<Connection>,
}

impl Database {
    pub fn init() -> Result<Self, AppError> {
        let db_path = database_path()?;
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AppError::io(parent, e))?;
        }

        let conn = Connection::open(&db_path).map_err(|e| AppError::Database(e.to_string()))?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        let db = Self {
            conn: Mutex::new(conn),
        };
        db.create_tables()?;
        db.apply_schema_migrations()?;
        db.seed_model_pricing()?;
        db.migrate_legacy_json_if_needed()?;
        Ok(db)
    }

    pub fn memory() -> Result<Self, AppError> {
        let conn = Connection::open_in_memory().map_err(|e| AppError::Database(e.to_string()))?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|e| AppError::Database(e.to_string()))?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.create_tables()?;
        db.apply_schema_migrations()?;
        db.seed_model_pricing()?;
        Ok(db)
    }
}

pub fn database_path() -> Result<PathBuf, AppError> {
    Ok(get_app_config_dir()?.join("cc-switch.db"))
}

#[cfg(test)]
mod tests {
    use super::Database;
    use crate::{
        app_config::{CommonConfigSnippets, McpApps, McpServer, MultiAppConfig},
        database::ProxyRequestLogRecord,
        prompt::Prompt,
        provider::{Provider, ProviderManager, ProviderMeta},
        services::skill::{Skill, SkillRepo, SkillRepoCache, SkillState, SkillStore},
        settings::{CustomEndpoint, ProxySettings},
    };
    use chrono::Utc;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn proxy_config_roundtrips_through_sqlite() {
        let db = Database::memory().expect("memory db");
        let mut config = ProxySettings::default();
        config.enabled = true;
        config.host = "0.0.0.0".to_string();
        config.port = 4567;
        config.upstream_proxy = Some("http://127.0.0.1:7890".to_string());
        config.bind_app = "codex".to_string();
        config.auto_start = true;
        config.enable_logging = true;
        config.live_takeover_active = true;
        config.streaming_first_byte_timeout = 11;
        config.streaming_idle_timeout = 22;
        config.non_streaming_timeout = 33;
        config.apps.claude.enabled = true;
        config.apps.claude.auto_failover_enabled = true;
        config.apps.claude.max_retries = 2;
        config.apps.claude.default_cost_multiplier = "1.5".to_string();
        config.apps.claude.pricing_model_source = "request".to_string();
        config.apps.codex.enabled = true;
        config.apps.codex.max_retries = 3;

        db.save_proxy_config(&config).expect("save proxy config");
        let loaded = db.get_proxy_config().expect("load proxy config");

        assert!(loaded.enabled);
        assert_eq!(loaded.host, "0.0.0.0");
        assert_eq!(loaded.port, 4567);
        assert_eq!(
            loaded.upstream_proxy.as_deref(),
            Some("http://127.0.0.1:7890")
        );
        assert_eq!(loaded.bind_app, "codex");
        assert!(loaded.auto_start);
        assert!(loaded.enable_logging);
        assert!(loaded.live_takeover_active);
        assert_eq!(loaded.streaming_first_byte_timeout, 11);
        assert_eq!(loaded.streaming_idle_timeout, 22);
        assert_eq!(loaded.non_streaming_timeout, 33);
        assert!(loaded.apps.claude.enabled);
        assert!(loaded.apps.claude.auto_failover_enabled);
        assert_eq!(loaded.apps.claude.max_retries, 2);
        assert_eq!(loaded.apps.claude.default_cost_multiplier, "1.5");
        assert_eq!(loaded.apps.claude.pricing_model_source, "request");
        assert!(loaded.apps.codex.enabled);
        assert_eq!(loaded.apps.codex.max_retries, 3);
        assert!(!loaded.apps.gemini.enabled);
    }

    #[test]
    fn model_pricing_roundtrips_and_matches_model_candidates() {
        let db = Database::memory().expect("memory db");
        let rows = db.list_model_pricing().expect("list seeded pricing");
        assert!(
            rows.iter()
                .any(|row| row.model_id == "claude-sonnet-4-20250514"),
            "seeded pricing should include Claude Sonnet 4"
        );

        db.upsert_model_pricing(&crate::database::ModelPricingRecord {
            model_id: "custom-model".to_string(),
            display_name: "Custom Model".to_string(),
            input_cost_per_million: "2.5".to_string(),
            output_cost_per_million: "7.5".to_string(),
            cache_read_cost_per_million: "0.25".to_string(),
            cache_creation_cost_per_million: "1.25".to_string(),
        })
        .expect("upsert pricing");
        assert!(db
            .get_model_pricing("provider/custom-model:extra")
            .expect("get pricing")
            .is_some());
        assert!(db
            .upsert_model_pricing(&crate::database::ModelPricingRecord {
                model_id: "bad".to_string(),
                display_name: "Bad".to_string(),
                input_cost_per_million: "not-a-number".to_string(),
                output_cost_per_million: "7.5".to_string(),
                cache_read_cost_per_million: "0.25".to_string(),
                cache_creation_cost_per_million: "1.25".to_string(),
            })
            .is_err());
        assert!(db
            .delete_model_pricing("custom-model")
            .expect("delete pricing"));
    }

    #[test]
    fn providers_roundtrip_through_sqlite() {
        let db = Database::memory().expect("memory db");
        let mut config = MultiAppConfig::default();
        let mut endpoints = HashMap::new();
        endpoints.insert(
            "https://endpoint.example/v1".to_string(),
            CustomEndpoint {
                url: "https://endpoint.example/v1".to_string(),
                added_at: 123,
                last_used: Some(456),
            },
        );
        let mut manager = ProviderManager {
            current: "primary".to_string(),
            backup_current: Some("backup".to_string()),
            ..ProviderManager::default()
        };
        manager.providers.insert(
            "primary".to_string(),
            Provider {
                id: "primary".to_string(),
                name: "Primary".to_string(),
                settings_config: json!({"apiKey": "secret", "baseUrl": "https://api.example"}),
                website_url: Some("https://example.com".to_string()),
                category: Some("custom".to_string()),
                created_at: Some(10),
                sort_index: Some(2),
                notes: Some("note".to_string()),
                meta: Some(ProviderMeta {
                    custom_endpoints: endpoints,
                    is_partner: Some(true),
                    partner_promotion_key: Some("partner".to_string()),
                    ..ProviderMeta::default()
                }),
            },
        );
        manager.providers.insert(
            "backup".to_string(),
            Provider {
                id: "backup".to_string(),
                name: "Backup".to_string(),
                settings_config: json!({"baseUrl": "https://backup.example"}),
                website_url: None,
                category: None,
                created_at: Some(11),
                sort_index: Some(1),
                notes: None,
                meta: None,
            },
        );
        config.apps.insert("claude".to_string(), manager);

        db.replace_config(&config).expect("replace config");
        let loaded = db.load_config().expect("load config");
        let manager = loaded.apps.get("claude").expect("claude manager");

        assert_eq!(manager.current, "primary");
        assert_eq!(manager.backup_current.as_deref(), Some("backup"));
        let primary = manager.providers.get("primary").expect("primary provider");
        assert_eq!(primary.name, "Primary");
        assert_eq!(primary.website_url.as_deref(), Some("https://example.com"));
        assert_eq!(primary.category.as_deref(), Some("custom"));
        assert_eq!(primary.created_at, Some(10));
        assert_eq!(primary.sort_index, Some(2));
        assert_eq!(primary.notes.as_deref(), Some("note"));
        let meta = primary.meta.as_ref().expect("provider meta");
        assert_eq!(meta.is_partner, Some(true));
        assert_eq!(meta.partner_promotion_key.as_deref(), Some("partner"));
        let endpoint = meta
            .custom_endpoints
            .get("https://endpoint.example/v1")
            .expect("custom endpoint");
        assert_eq!(endpoint.added_at, 123);
        assert_eq!(endpoint.last_used, Some(456));
    }

    #[test]
    fn mcp_prompts_and_common_snippets_roundtrip_through_sqlite() {
        let db = Database::memory().expect("memory db");
        let mut config = MultiAppConfig::default();
        config.common_config_snippets = CommonConfigSnippets {
            claude: Some("claude snippet".to_string()),
            codex: Some("codex snippet".to_string()),
            gemini: None,
        };
        let mut servers = HashMap::new();
        servers.insert(
            "server-a".to_string(),
            McpServer {
                id: "server-a".to_string(),
                name: "Server A".to_string(),
                server: json!({"command": "node", "args": ["server.js"]}),
                apps: McpApps {
                    claude: true,
                    codex: false,
                    gemini: true,
                    opencode: true,
                },
                description: Some("desc".to_string()),
                homepage: Some("https://home.example".to_string()),
                docs: Some("https://docs.example".to_string()),
                tags: vec!["one".to_string(), "two".to_string()],
            },
        );
        config.mcp.servers = Some(servers);
        config.prompts.claude.prompts.insert(
            "prompt-a".to_string(),
            Prompt {
                id: "prompt-a".to_string(),
                name: "Prompt A".to_string(),
                content: "Hello".to_string(),
                description: Some("prompt desc".to_string()),
                enabled: true,
                created_at: Some(100),
                updated_at: Some(200),
            },
        );
        config.prompts.codex.prompts.insert(
            "prompt-b".to_string(),
            Prompt {
                id: "prompt-b".to_string(),
                name: "Prompt B".to_string(),
                content: "World".to_string(),
                description: None,
                enabled: false,
                created_at: None,
                updated_at: None,
            },
        );

        db.replace_config(&config).expect("replace config");
        let loaded = db.load_config().expect("load config");
        let servers = loaded.mcp.servers.expect("mcp servers");
        let server = servers.get("server-a").expect("server-a");

        assert!(server.apps.claude);
        assert!(!server.apps.codex);
        assert!(server.apps.gemini);
        assert!(server.apps.opencode);
        assert_eq!(server.homepage.as_deref(), Some("https://home.example"));
        assert_eq!(server.docs.as_deref(), Some("https://docs.example"));
        assert_eq!(server.tags, vec!["one".to_string(), "two".to_string()]);
        assert_eq!(
            loaded.prompts.claude.prompts["prompt-a"]
                .description
                .as_deref(),
            Some("prompt desc")
        );
        assert!(loaded.prompts.claude.prompts["prompt-a"].enabled);
        assert!(!loaded.prompts.codex.prompts["prompt-b"].enabled);
        assert_eq!(
            loaded.common_config_snippets.claude.as_deref(),
            Some("claude snippet")
        );
    }

    #[test]
    fn skills_roundtrip_through_sqlite() {
        let db = Database::memory().expect("memory db");
        let installed_at = Utc::now();
        let mut config = MultiAppConfig::default();
        config.skills = SkillStore {
            repos: vec![
                SkillRepo {
                    owner: "owner".to_string(),
                    name: "repo".to_string(),
                    branch: "main".to_string(),
                    enabled: true,
                    skills_path: None,
                },
                SkillRepo {
                    owner: "owner".to_string(),
                    name: "repo2".to_string(),
                    branch: "dev".to_string(),
                    enabled: false,
                    skills_path: Some("skills".to_string()),
                },
            ],
            skills: HashMap::from([(
                "owner/repo:skill-a".to_string(),
                SkillState {
                    installed: true,
                    installed_at,
                },
            )]),
            repo_cache: HashMap::from([(
                "owner/repo/main".to_string(),
                SkillRepoCache {
                    skills: vec![Skill {
                        key: "owner/repo:skill-a".to_string(),
                        name: "Skill A".to_string(),
                        description: "Desc".to_string(),
                        directory: "skill-a".to_string(),
                        parent_path: None,
                        depth: 0,
                        readme_url: Some("https://readme.example".to_string()),
                        installed: true,
                        installed_apps: vec!["claude".to_string()],
                        repo_owner: Some("owner".to_string()),
                        repo_name: Some("repo".to_string()),
                        repo_branch: Some("main".to_string()),
                        skills_path: None,
                        commands: Vec::new(),
                    }],
                    fetched_at: installed_at,
                    etag: Some("etag".to_string()),
                    last_modified: Some("today".to_string()),
                },
            )]),
        };

        db.replace_config(&config).expect("replace config");
        let loaded = db.load_config().expect("load config");

        assert_eq!(loaded.skills.repos.len(), 2);
        assert_eq!(loaded.skills.repos[0].skills_path, None);
        assert_eq!(
            loaded.skills.repos[1].skills_path.as_deref(),
            Some("skills")
        );
        assert!(loaded.skills.skills["owner/repo:skill-a"].installed);
        let cache = loaded
            .skills
            .repo_cache
            .get("owner/repo/main")
            .expect("repo cache");
        assert_eq!(cache.skills[0].name, "Skill A");
        assert_eq!(cache.etag.as_deref(), Some("etag"));
    }

    #[test]
    fn failover_health_logs_and_universal_provider_daos_roundtrip() {
        let db = Database::memory().expect("memory db");
        db.replace_failover_queue(
            "claude",
            &[
                "first".to_string(),
                "second".to_string(),
                "third".to_string(),
            ],
        )
        .expect("replace failover queue");
        db.remove_failover_provider("claude", "second")
            .expect("remove provider");
        db.add_failover_provider("claude", "fourth")
            .expect("add provider");

        let queue = db.list_failover_queue("claude").expect("list queue");
        assert_eq!(
            queue
                .iter()
                .map(|item| item.provider_id.as_str())
                .collect::<Vec<_>>(),
            vec!["first", "third", "fourth"]
        );

        db.record_provider_success("claude", "first")
            .expect("success health");
        db.record_provider_failure("claude", "first", Some("rate limited"), true)
            .expect("failure health");
        let health = db
            .get_provider_health("claude", "first")
            .expect("get health")
            .expect("health row");
        assert!(!health.is_healthy);
        assert_eq!(health.consecutive_failures, 1);
        assert_eq!(health.last_error.as_deref(), Some("rate limited"));

        let log = ProxyRequestLogRecord {
            request_id: "request-1".to_string(),
            provider_id: "first".to_string(),
            app_type: "claude".to_string(),
            model: "claude-sonnet".to_string(),
            request_model: Some("claude-sonnet".to_string()),
            input_tokens: 10,
            output_tokens: 20,
            cache_read_tokens: 3,
            cache_creation_tokens: 4,
            input_cost_usd: "0.002".to_string(),
            output_cost_usd: "0.003".to_string(),
            cache_read_cost_usd: "0.001".to_string(),
            cache_creation_cost_usd: "0.004".to_string(),
            total_cost_usd: "0.01".to_string(),
            latency_ms: 123,
            first_token_ms: Some(50),
            duration_ms: Some(150),
            status_code: 200,
            error_message: None,
            session_id: Some("session-1".to_string()),
            provider_type: Some("custom".to_string()),
            is_streaming: true,
            cost_multiplier: "1.0".to_string(),
            created_at: 1000,
            data_source: "proxy".to_string(),
        };
        db.insert_proxy_request_log(&log).expect("insert log");
        let logs = db.recent_proxy_request_logs(10).expect("recent logs");
        assert_eq!(logs, vec![log]);

        db.save_universal_provider_json("provider-a", &json!({"name": "Provider A"}))
            .expect("save universal provider");
        assert_eq!(
            db.list_universal_provider_ids()
                .expect("list universal providers"),
            vec!["provider-a".to_string()]
        );
        let value: serde_json::Value = db
            .get_universal_provider_json("provider-a")
            .expect("get universal provider")
            .expect("universal provider");
        assert_eq!(value["name"], "Provider A");
        db.delete_universal_provider("provider-a")
            .expect("delete universal provider");
        assert!(db
            .get_universal_provider_json::<serde_json::Value>("provider-a")
            .expect("get deleted universal provider")
            .is_none());
    }

    #[test]
    fn usage_rollup_aggregates_and_prunes_old_proxy_logs() {
        let db = Database::memory().expect("memory db");
        let now = Utc::now().timestamp_millis();
        let old_created_at = now - 40 * 86_400_000;
        let recent_created_at = now - 5 * 86_400_000;

        for index in 0..3 {
            db.insert_proxy_request_log(&ProxyRequestLogRecord {
                request_id: format!("old-{index}"),
                provider_id: "provider-a".to_string(),
                app_type: "claude".to_string(),
                model: "claude-sonnet".to_string(),
                request_model: None,
                input_tokens: 10,
                output_tokens: 20,
                cache_read_tokens: 1,
                cache_creation_tokens: 2,
                input_cost_usd: "0".to_string(),
                output_cost_usd: "0".to_string(),
                cache_read_cost_usd: "0".to_string(),
                cache_creation_cost_usd: "0".to_string(),
                total_cost_usd: "0.1".to_string(),
                latency_ms: 100,
                first_token_ms: None,
                duration_ms: None,
                status_code: 200,
                error_message: None,
                session_id: None,
                provider_type: None,
                is_streaming: false,
                cost_multiplier: "1.0".to_string(),
                created_at: old_created_at + index,
                data_source: "proxy".to_string(),
            })
            .expect("insert old log");
        }
        db.insert_proxy_request_log(&ProxyRequestLogRecord {
            request_id: "recent".to_string(),
            provider_id: "provider-a".to_string(),
            app_type: "claude".to_string(),
            model: "claude-sonnet".to_string(),
            request_model: None,
            input_tokens: 100,
            output_tokens: 200,
            cache_read_tokens: 10,
            cache_creation_tokens: 20,
            input_cost_usd: "0".to_string(),
            output_cost_usd: "0".to_string(),
            cache_read_cost_usd: "0".to_string(),
            cache_creation_cost_usd: "0".to_string(),
            total_cost_usd: "1.0".to_string(),
            latency_ms: 500,
            first_token_ms: None,
            duration_ms: None,
            status_code: 200,
            error_message: None,
            session_id: None,
            provider_type: None,
            is_streaming: false,
            cost_multiplier: "1.0".to_string(),
            created_at: recent_created_at,
            data_source: "proxy".to_string(),
        })
        .expect("insert recent log");

        let deleted = db
            .rollup_and_prune_proxy_request_logs(30)
            .expect("rollup and prune");
        assert_eq!(deleted, 3);

        let rollups = db.list_usage_daily_rollups("claude").expect("list rollups");
        assert_eq!(rollups.len(), 1);
        assert_eq!(rollups[0].request_count, 3);
        assert_eq!(rollups[0].success_count, 3);
        assert_eq!(rollups[0].input_tokens, 30);
        assert_eq!(rollups[0].output_tokens, 60);
        assert_eq!(rollups[0].cache_read_tokens, 3);
        assert_eq!(rollups[0].cache_creation_tokens, 6);

        let remaining = db.recent_proxy_request_logs(10).expect("recent logs");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].request_id, "recent");
    }
}

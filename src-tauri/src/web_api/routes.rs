#![cfg(feature = "web-server")]

use axum::{
    routing::{delete, get, post, put},
    Router,
};

use super::{
    handlers::{
        auth, capabilities, config, deeplink, health, mcp, model_fetch, openclaw, prompts,
        providers, proxy, sessions, settings, skills, stream_check, subscription, system, usage,
        webdav, workspace,
    },
    SharedState,
};

pub fn create_router(state: SharedState) -> Router {
    Router::new()
        .route("/capabilities", get(capabilities::get_capabilities))
        .route("/health/status", get(health::proxy_status))
        .nest("/auth", auth_routes())
        .nest("/providers", provider_routes())
        .nest("/mcp", mcp_routes())
        .nest("/prompts", prompt_routes())
        .nest("/skills", skill_routes())
        .nest("/sessions", session_routes())
        .nest("/settings", settings_routes())
        .nest("/deeplink", deeplink_routes())
        .nest("/proxy", proxy_routes())
        .nest("/usage", usage_routes())
        .nest("/webdav", webdav_routes())
        .nest("/workspace", workspace_routes())
        .nest("/openclaw", openclaw_routes())
        .nest("/config", config_routes())
        .route("/model-fetch", post(model_fetch::fetch_models_for_config))
        .route(
            "/model-fetch/codex-oauth",
            get(model_fetch::get_codex_oauth_models),
        )
        .route(
            "/model-fetch/github-copilot",
            get(model_fetch::get_github_copilot_models),
        )
        .nest("/stream-check", stream_check_routes())
        .route("/subscriptions/quota", get(subscription::query_quota))
        .route("/tray/update", post(system::update_tray))
        .route("/system/csrf-token", get(system::get_csrf_token))
        .route("/system/credentials", put(system::update_credentials))
        .route("/system/open-external", post(system::open_external))
        .route(
            "/unsupported/:operation",
            get(system::unsupported_operation)
                .post(system::unsupported_operation)
                .put(system::unsupported_operation)
                .delete(system::unsupported_operation),
        )
        .route("/fs/pick-directory", post(config::pick_directory))
        .route("/fs/save-file", post(config::save_file_dialog))
        .route("/fs/open-file", post(config::open_file_dialog))
        .with_state(state)
}

fn openclaw_routes() -> Router<SharedState> {
    Router::new()
        .route("/status", get(openclaw::get_status))
        .route(
            "/raw",
            get(openclaw::get_raw_config).put(openclaw::set_raw_config),
        )
        .route("/providers", get(openclaw::get_providers))
        .route("/providers/:provider_id", get(openclaw::get_provider))
        .route(
            "/reconciliation",
            get(openclaw::preview_reconciliation).post(openclaw::apply_reconciliation),
        )
        .route(
            "/reconciliation/import-new",
            post(openclaw::import_live_providers),
        )
        .route(
            "/default-model",
            get(openclaw::get_default_model)
                .put(openclaw::set_default_model)
                .delete(openclaw::clear_default_model),
        )
        .route(
            "/model-catalog",
            get(openclaw::get_model_catalog).put(openclaw::set_model_catalog),
        )
        .route(
            "/agents-defaults",
            get(openclaw::get_agents_defaults).put(openclaw::set_agents_defaults),
        )
        .route("/env", get(openclaw::get_env).put(openclaw::set_env))
        .route("/tools", get(openclaw::get_tools).put(openclaw::set_tools))
        .route("/health", get(openclaw::get_health))
}

fn workspace_routes() -> Router<SharedState> {
    Router::new()
        .route("/files", get(workspace::list_files))
        .route(
            "/files/:name",
            get(workspace::read_file).put(workspace::write_file),
        )
        .route("/files/:name/backups", get(workspace::list_backups))
        .route("/files/:name/restore", post(workspace::restore_backup))
        .route("/memory", get(workspace::list_daily_memory))
        .route("/memory/search", get(workspace::search_daily_memory))
        .route(
            "/memory/:date",
            get(workspace::read_daily_memory)
                .put(workspace::write_daily_memory)
                .delete(workspace::delete_daily_memory),
        )
}

fn session_routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/",
            get(sessions::list_sessions).delete(sessions::delete_session),
        )
        .route("/messages", post(sessions::get_messages))
        .route("/page", get(sessions::list_sessions_page))
        .route("/delete-batch", post(sessions::delete_sessions))
}

fn auth_routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/accounts",
            get(auth::list_accounts)
                .post(auth::import_account)
                .delete(auth::delete_account_by_query),
        )
        .route(
            "/accounts/:provider/:account_id/default",
            post(auth::set_default),
        )
        .route("/accounts/default", post(auth::set_default_by_query))
        .route(
            "/accounts/:provider/:account_id/logout",
            post(auth::logout_account),
        )
        .route("/accounts/logout", post(auth::logout_account_by_query))
        .route(
            "/accounts/:provider/:account_id",
            delete(auth::delete_account),
        )
        .route("/device/start", post(auth::start_device_login))
        .route("/device/poll", post(auth::poll_device_login))
        .route("/usage", get(auth::query_usage))
}

fn webdav_routes() -> Router<SharedState> {
    Router::new()
        .route("/snapshot/upload", post(webdav::upload_snapshot))
        .route(
            "/snapshot/preview",
            get(webdav::preview_snapshot).post(webdav::preview_snapshot),
        )
        .route("/snapshot/download", post(webdav::download_snapshot))
        .route("/snapshot/sync", post(webdav::sync_snapshot))
        .route(
            "/backups",
            get(webdav::list_backups).post(webdav::list_backups),
        )
        .route("/backups/restore", post(webdav::restore_backup))
}

fn stream_check_routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/config",
            get(stream_check::get_stream_check_config).put(stream_check::save_stream_check_config),
        )
        .route(
            "/providers/:app/:id",
            post(stream_check::stream_check_provider),
        )
        .route(
            "/providers/:id",
            post(stream_check::stream_check_provider_by_id),
        )
        .route("/all", post(stream_check::stream_check_all_providers))
        .route("/providers", post(stream_check::stream_check_all_providers))
        .route("/logs", get(stream_check::get_stream_check_logs))
        .route(
            "/logs/latest",
            get(stream_check::get_latest_stream_check_logs),
        )
}

fn provider_routes() -> Router<SharedState> {
    Router::new()
        .route("/universal", get(providers::list_universal_providers))
        .route(
            "/universal/preview",
            post(providers::preview_universal_provider),
        )
        .route(
            "/universal/:id",
            get(providers::get_universal_provider)
                .put(providers::upsert_universal_provider)
                .delete(providers::delete_universal_provider),
        )
        .route(
            "/universal/:id/sync",
            post(providers::sync_universal_provider),
        )
        .route(
            "/:app",
            get(providers::list_providers).post(providers::add_provider),
        )
        .route("/:app/current", get(providers::current_provider))
        .route(
            "/:app/live-settings",
            get(providers::read_live_provider_settings),
        )
        .route(
            "/opencode/live-provider-ids",
            get(providers::opencode_live_provider_ids),
        )
        .route(
            "/claude-desktop/default-routes",
            get(providers::claude_desktop_default_routes),
        )
        .route(
            "/claude-desktop/status",
            get(providers::claude_desktop_status),
        )
        .route(
            "/claude-desktop/import-from-claude",
            post(providers::import_claude_desktop_providers_from_claude),
        )
        .route(
            "/:app/:id",
            put(providers::update_provider).delete(providers::delete_provider),
        )
        .route("/:app/:id/switch", post(providers::switch_provider))
        .route("/:app/:id/usage", post(providers::query_provider_usage))
        .route("/:app/:id/usage/test", post(providers::test_usage_script))
        .route(
            "/:app/import-default",
            post(providers::import_default_config),
        )
        .route("/:app/sort-order", put(providers::update_sort_order))
        .route("/omo/plugin-status", get(providers::omo_plugin_status))
        .route(
            "/omo-slim/plugin-status",
            get(providers::omo_slim_plugin_status),
        )
        .route("/omo/disable-current", post(providers::disable_current_omo))
        .route(
            "/omo-slim/disable-current",
            post(providers::disable_current_omo_slim),
        )
        .route(
            "/:app/backup",
            get(providers::backup_provider).put(providers::set_backup_provider),
        )
        .route(
            "/sync-current",
            post(providers::sync_current_providers_live),
        )
}

fn mcp_routes() -> Router<SharedState> {
    Router::new()
        .route("/status", get(mcp::get_status))
        .route("/config/claude", get(mcp::read_config))
        .route(
            "/config/claude/servers/:id",
            put(mcp::upsert_claude_server).delete(mcp::delete_claude_server),
        )
        .route("/validate", post(mcp::validate_command))
        .route("/config/:app", get(mcp::get_config))
        .route(
            "/config/:app/servers/:id",
            put(mcp::upsert_server_in_config).delete(mcp::delete_server_in_config),
        )
        .route("/config/:app/servers/:id/enabled", post(mcp::set_enabled))
        .route("/servers", get(mcp::list_servers).post(mcp::upsert_server))
        .route("/servers/import-from-apps", post(mcp::import_from_apps))
        .route(
            "/servers/:id",
            put(mcp::update_server).delete(mcp::delete_server),
        )
        .route("/servers/:id/apps/:app", post(mcp::toggle_app))
}

fn prompt_routes() -> Router<SharedState> {
    Router::new()
        .route("/:app", get(prompts::list_prompts))
        .route(
            "/:app/:id",
            put(prompts::upsert_prompt).delete(prompts::delete_prompt),
        )
        .route("/:app/:id/enable", post(prompts::enable_prompt))
        .route("/:app/import-from-file", post(prompts::import_from_file))
        .route("/:app/current-file", get(prompts::current_file_content))
}

fn skill_routes() -> Router<SharedState> {
    Router::new()
        .route("/", get(skills::list_skills))
        .route("/install", post(skills::install_skill))
        .route("/uninstall", post(skills::uninstall_skill))
        .route("/discovery", get(skills::scan_unmanaged_skills))
        .route("/discovery/import", post(skills::import_skills_from_apps))
        .route("/import-zip", post(skills::install_from_zip))
        .route("/storage/migrate", post(skills::migrate_storage))
        .route("/updates", get(skills::check_updates))
        .route("/updates/apply", post(skills::update_skill))
        .route("/catalog/search", get(skills::search_skills_sh))
        .route("/catalog/install", post(skills::install_catalog_skill))
        .route("/backups", get(skills::list_backups))
        .route("/backups/restore", post(skills::restore_backup))
        .route("/backups/:backup_id", delete(skills::delete_backup))
        .route("/repos", get(skills::list_repos).post(skills::add_repo))
        .route("/repos/:owner/:name", delete(skills::remove_repo))
}

fn settings_routes() -> Router<SharedState> {
    Router::new().route(
        "/",
        get(settings::get_settings).put(settings::save_settings),
    )
}

fn deeplink_routes() -> Router<SharedState> {
    Router::new()
        .route("/parse", post(deeplink::parse_deeplink))
        .route("/merge-config", post(deeplink::merge_deeplink_config))
        .route("/import", post(deeplink::import_from_deeplink))
}

fn proxy_routes() -> Router<SharedState> {
    Router::new()
        .route("/status", get(proxy::get_status))
        .route("/config", get(proxy::get_config).put(proxy::save_config))
        .route("/settings", put(proxy::save_settings))
        .route("/start", post(proxy::start))
        .route("/stop", post(proxy::stop))
        .route("/test", post(proxy::test))
        .route("/logs/recent", get(proxy::recent_logs))
        .route("/pricing/models", get(proxy::list_model_pricing))
        .route(
            "/pricing/models/:model_id",
            put(proxy::upsert_model_pricing).delete(proxy::delete_model_pricing),
        )
        .route(
            "/failover/:app",
            get(proxy::get_failover_queue)
                .put(proxy::replace_failover_queue)
                .delete(proxy::clear_failover_queue),
        )
        .route(
            "/failover/:app/:id",
            post(proxy::add_failover_provider).delete(proxy::remove_failover_provider),
        )
        .route(
            "/health/:app/:id/reset",
            post(proxy::reset_provider_circuit),
        )
        .route("/takeover", get(proxy::get_takeover))
        .route("/takeover/:app", put(proxy::set_takeover))
        .route("/restore", post(proxy::restore))
        .route(
            "/recover-stale-takeover",
            post(proxy::recover_stale_takeover),
        )
}

fn usage_routes() -> Router<SharedState> {
    Router::new()
        .route("/summary", get(usage::summary))
        .route("/summary-by-app", get(usage::summary_by_app))
        .route("/trends", get(usage::trends))
        .route("/providers", get(usage::providers))
        .route("/models", get(usage::models))
        .route("/logs", post(usage::logs))
        .route("/logs/:request_id", get(usage::detail))
        .route("/pricing/models", get(usage::pricing))
        .route(
            "/pricing/models/:model_id",
            put(usage::upsert_pricing).delete(usage::delete_pricing),
        )
        .route("/limits/:app_type/:provider_id", get(usage::limits))
        .route("/sessions/sync", post(usage::sync_sessions))
        .route("/data-sources", get(usage::data_sources))
        .route("/data-extent", get(usage::data_extent))
}

fn config_routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/export",
            get(config::export_config_snapshot).post(config::export_config),
        )
        .route("/import", post(config::import_config))
        .route(
            "/backups",
            get(config::list_db_backups).post(config::create_db_backup),
        )
        .route("/backups/restore", post(config::restore_db_backup))
        .route("/backups/rename", post(config::rename_db_backup))
        .route("/backups/:filename", delete(config::delete_db_backup))
        .route("/:app/dir", get(config::get_config_dir))
        .route("/:app/dir-info", get(config::get_config_dir_info))
        .route("/:app/open", post(config::open_config_folder))
        .route(
            "/claude-code/path",
            get(config::get_claude_code_config_path),
        )
        .route("/app/path", get(config::get_app_config_path))
        .route("/app/open", post(config::open_app_config_folder))
        .route(
            "/app/override",
            get(config::get_app_config_dir_override).put(config::set_app_config_dir_override),
        )
        .route(
            "/claude/common-snippet",
            get(config::get_claude_common_config_snippet)
                .put(config::set_claude_common_config_snippet),
        )
        .route("/claude/plugin", post(config::apply_claude_plugin_config))
        .route(
            "/:app/common-snippet",
            get(config::get_common_config_snippet).put(config::set_common_config_snippet),
        )
}

#![cfg(feature = "web-server")]

use axum::{
    routing::{delete, get, post, put},
    Router,
};

use super::{
    handlers::{
        config, health, mcp, model_fetch, prompts, providers, proxy, settings, skills,
        stream_check, system,
    },
    SharedState,
};

pub fn create_router(state: SharedState) -> Router {
    Router::new()
        .route("/health/status", get(health::proxy_status))
        .nest("/providers", provider_routes())
        .nest("/mcp", mcp_routes())
        .nest("/prompts", prompt_routes())
        .nest("/skills", skill_routes())
        .nest("/settings", settings_routes())
        .nest("/proxy", proxy_routes())
        .nest("/config", config_routes())
        .route("/model-fetch", post(model_fetch::fetch_models_for_config))
        .nest("/stream-check", stream_check_routes())
        .route("/tray/update", post(system::update_tray))
        .route("/system/csrf-token", get(system::get_csrf_token))
        .route("/system/credentials", put(system::update_credentials))
        .route("/system/open-external", post(system::open_external))
        .route("/fs/pick-directory", post(config::pick_directory))
        .route("/fs/save-file", post(config::save_file_dialog))
        .route("/fs/open-file", post(config::open_file_dialog))
        .with_state(state)
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
        .route("/providers", post(stream_check::stream_check_all_providers))
}

fn provider_routes() -> Router<SharedState> {
    Router::new()
        .route("/universal", get(providers::list_universal_providers))
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
        .route("/repos", get(skills::list_repos).post(skills::add_repo))
        .route("/repos/:owner/:name", delete(skills::remove_repo))
}

fn settings_routes() -> Router<SharedState> {
    Router::new().route(
        "/",
        get(settings::get_settings).put(settings::save_settings),
    )
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
        .route("/takeover", get(proxy::get_takeover))
        .route("/takeover/:app", put(proxy::set_takeover))
        .route("/restore", post(proxy::restore))
        .route(
            "/recover-stale-takeover",
            post(proxy::recover_stale_takeover),
        )
}

fn config_routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/export",
            get(config::export_config_snapshot).post(config::export_config),
        )
        .route("/import", post(config::import_config))
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

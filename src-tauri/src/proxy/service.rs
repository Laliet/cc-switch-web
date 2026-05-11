use std::sync::Arc;

use crate::{
    app_config::AppType,
    error::AppError,
    services::provider::ProviderService,
    settings::{self, ProxySettings},
    store::AppState,
};

use super::{
    live, server,
    types::{ProxyStatus, ProxyTakeoverResult},
};

pub struct ProxyService;

impl ProxyService {
    pub fn config() -> ProxySettings {
        settings::get_settings().proxy
    }

    pub fn save_config(config: ProxySettings) -> Result<ProxySettings, AppError> {
        let mut app_settings = settings::get_settings();
        app_settings.proxy = normalize_config(config);
        server::validate_settings(&app_settings.proxy)?;
        settings::update_settings(app_settings)?;
        Ok(settings::get_settings().proxy)
    }

    pub async fn start(
        state: Arc<AppState>,
        mut config: ProxySettings,
    ) -> Result<ProxyStatus, AppError> {
        config.enabled = true;
        config.live_takeover_active = has_takeover_apps(&config);
        config = normalize_config(config);
        ensure_takeover_config_supported(&state, &config)?;
        server::start_proxy(state.clone(), config.clone()).await?;
        if let Err(err) = Self::save_config(config) {
            let _ = server::stop_proxy().await;
            let _ = live::restore_all();
            return Err(err);
        }
        Ok(server::status_for_state(&state).await)
    }

    pub async fn stop(state: Arc<AppState>) -> Result<ProxyStatus, AppError> {
        let _ = server::stop_proxy().await?;
        live::restore_all()?;
        server::clear_recent_logs().await;
        let mut app_settings = settings::get_settings();
        app_settings.proxy.enabled = false;
        app_settings.proxy.live_takeover_active = false;
        settings::update_settings(app_settings)?;
        Ok(server::status_for_state(&state).await)
    }

    pub async fn set_takeover(
        state: Arc<AppState>,
        app: AppType,
        enabled: bool,
    ) -> Result<ProxyTakeoverResult, AppError> {
        if matches!(app, AppType::Omo) {
            return Err(AppError::localized(
                "proxy.omo.unsupported",
                "代理暂不支持 OMO。",
                "Proxy does not support OMO yet.",
            ));
        }
        if enabled && matches!(app, AppType::Gemini) {
            let provider = server::current_provider(&state, &app)?;
            ensure_gemini_takeover_supported(&provider)?;
        }

        let original_settings = settings::get_settings();
        let mut app_settings = original_settings.clone();
        set_app_enabled(&mut app_settings.proxy, &app, enabled);
        app_settings.proxy.live_takeover_active = has_takeover_apps(&app_settings.proxy);

        let status = server::status_for_state(&state).await;
        if status.running {
            if enabled {
                let provider = server::current_provider(&state, &app)?;
                let listen_url = status.listen_url.clone().ok_or_else(|| {
                    AppError::Config("Proxy is running without a listen URL".into())
                })?;
                live::apply_takeover(&app, &provider, &listen_url)?;
            } else {
                live::restore_takeover(&app)?;
            }
        }

        if let Err(err) = settings::update_settings(app_settings.clone()) {
            if status.running {
                if enabled {
                    let _ = live::restore_takeover(&app);
                } else if is_app_enabled(&original_settings.proxy, &app) {
                    if let (Ok(provider), Some(listen_url)) = (
                        server::current_provider(&state, &app),
                        status.listen_url.as_deref(),
                    ) {
                        let _ = live::apply_takeover(&app, &provider, listen_url);
                    }
                }
            }
            return Err(err);
        }
        server::update_runtime_settings(app_settings.proxy.clone()).await;

        Ok(ProxyTakeoverResult {
            app: app.as_str().to_string(),
            enabled,
            status: server::status_for_state(&state).await,
        })
    }

    pub async fn restore(state: Arc<AppState>) -> Result<ProxyStatus, AppError> {
        live::restore_all()?;
        server::update_runtime_settings(settings::get_settings().proxy).await;
        server::clear_recent_logs().await;
        Ok(server::status_for_state(&state).await)
    }

    pub async fn recover_stale_takeover(state: Arc<AppState>) -> Result<ProxyStatus, AppError> {
        live::restore_all()?;
        server::update_runtime_settings(settings::get_settings().proxy).await;
        server::clear_recent_logs().await;
        Ok(server::status_for_state(&state).await)
    }
}

fn normalize_config(mut config: ProxySettings) -> ProxySettings {
    config.host = config.host.trim().to_string();
    config.bind_app = config.bind_app.trim().to_lowercase();
    config.upstream_proxy = config
        .upstream_proxy
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    config.live_takeover_active = has_takeover_apps(&config);
    config
}

fn has_takeover_apps(config: &ProxySettings) -> bool {
    config.apps.claude.enabled
        || config.apps.codex.enabled
        || config.apps.gemini.enabled
        || config.apps.opencode.enabled
}

fn set_app_enabled(config: &mut ProxySettings, app: &AppType, enabled: bool) {
    match app {
        AppType::Claude => config.apps.claude.enabled = enabled,
        AppType::Codex => config.apps.codex.enabled = enabled,
        AppType::Gemini => config.apps.gemini.enabled = enabled,
        AppType::Opencode => config.apps.opencode.enabled = enabled,
        AppType::Omo => {}
    }
}

fn is_app_enabled(config: &ProxySettings, app: &AppType) -> bool {
    match app {
        AppType::Claude => config.apps.claude.enabled,
        AppType::Codex => config.apps.codex.enabled,
        AppType::Gemini => config.apps.gemini.enabled,
        AppType::Opencode => config.apps.opencode.enabled,
        AppType::Omo => false,
    }
}

fn ensure_takeover_config_supported(
    state: &Arc<AppState>,
    config: &ProxySettings,
) -> Result<(), AppError> {
    if config.apps.gemini.enabled {
        let provider = server::current_provider(state, &AppType::Gemini)?;
        ensure_gemini_takeover_supported(&provider)?;
    }
    Ok(())
}

pub(crate) fn ensure_gemini_takeover_supported(
    provider: &crate::provider::Provider,
) -> Result<(), AppError> {
    if ProviderService::is_google_official_gemini_provider(provider) {
        return Err(AppError::localized(
            "proxy.gemini.oauth.unsupported",
            "Gemini OAuth Provider 暂不支持代理接管，请使用 API Key Provider。",
            "Gemini OAuth providers are not supported for proxy takeover yet. Use an API key provider.",
        ));
    }
    Ok(())
}

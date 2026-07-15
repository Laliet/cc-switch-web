use serde::Serialize;
use std::collections::BTreeMap;

use crate::app_config::AppType;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCapabilities {
    pub runtime: &'static str,
    pub host: &'static str,
    pub apps: Vec<&'static str>,
    pub features: FeatureCapabilities,
    pub app_features: BTreeMap<&'static str, AppCapabilities>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureCapabilities {
    pub directory_picker: bool,
    pub open_external: bool,
    pub endpoint_test: bool,
    pub workspace: bool,
    pub subscription_quota: bool,
    pub tray: bool,
    pub terminal_launch: bool,
    pub config_dir_override: bool,
    pub file_dialogs: bool,
    pub session_manager: bool,
    pub usage_dashboard: bool,
    pub environment_management: bool,
    pub app_update: bool,
    pub portable_mode: bool,
    pub claude_plugin_integration: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppCapabilities {
    pub providers: bool,
    pub prompts: bool,
    pub mcp: bool,
    pub skills: bool,
    pub usage: bool,
    pub sessions: bool,
    pub local_routing: bool,
    pub additive_provider_mode: bool,
    pub host_managed: bool,
}

impl AppCapabilities {
    pub fn supports(&self, feature: &str) -> Option<bool> {
        match feature {
            "providers" => Some(self.providers),
            "prompts" => Some(self.prompts),
            "mcp" => Some(self.mcp),
            "skills" => Some(self.skills),
            "usage" => Some(self.usage),
            "sessions" => Some(self.sessions),
            "local_routing" => Some(self.local_routing),
            "additive_provider_mode" => Some(self.additive_provider_mode),
            _ => None,
        }
    }
}

pub fn supports_app_feature(app: &AppType, feature: &str) -> Option<bool> {
    app_capabilities(true)
        .get(app.as_str())
        .and_then(|capabilities| capabilities.supports(feature))
}

impl RuntimeCapabilities {
    pub fn web() -> Self {
        Self {
            runtime: "web",
            host: "server",
            apps: managed_apps(),
            features: FeatureCapabilities {
                directory_picker: false,
                open_external: true,
                endpoint_test: false,
                workspace: true,
                subscription_quota: true,
                tray: false,
                terminal_launch: false,
                config_dir_override: false,
                file_dialogs: false,
                session_manager: true,
                usage_dashboard: true,
                environment_management: false,
                app_update: false,
                portable_mode: false,
                claude_plugin_integration: false,
            },
            app_features: app_capabilities(true),
        }
    }

    pub fn desktop() -> Self {
        Self {
            runtime: "desktop",
            host: "local",
            apps: managed_apps(),
            features: FeatureCapabilities {
                directory_picker: true,
                open_external: true,
                endpoint_test: true,
                workspace: true,
                subscription_quota: true,
                tray: true,
                terminal_launch: false,
                config_dir_override: true,
                file_dialogs: true,
                session_manager: true,
                usage_dashboard: true,
                environment_management: true,
                app_update: true,
                portable_mode: true,
                claude_plugin_integration: true,
            },
            app_features: app_capabilities(false),
        }
    }
}

fn managed_apps() -> Vec<&'static str> {
    vec![
        "claude",
        "claude-desktop",
        "codex",
        "gemini",
        "opencode",
        "openclaw",
        "omo",
        "omo-slim",
    ]
}

fn app_capabilities(host_managed: bool) -> BTreeMap<&'static str, AppCapabilities> {
    let mut apps = BTreeMap::new();
    apps.insert("claude", full_switch_app(host_managed, true, true));
    apps.insert(
        "claude-desktop",
        AppCapabilities {
            providers: true,
            prompts: false,
            mcp: false,
            skills: false,
            usage: false,
            sessions: false,
            local_routing: true,
            additive_provider_mode: false,
            host_managed,
        },
    );
    apps.insert("codex", full_switch_app(host_managed, true, true));
    apps.insert("gemini", full_switch_app(host_managed, true, true));
    apps.insert(
        "opencode",
        AppCapabilities {
            additive_provider_mode: true,
            ..full_switch_app(host_managed, true, true)
        },
    );
    apps.insert(
        "openclaw",
        AppCapabilities {
            providers: true,
            prompts: false,
            mcp: false,
            skills: false,
            usage: false,
            sessions: true,
            local_routing: false,
            additive_provider_mode: true,
            host_managed,
        },
    );
    for app in ["omo", "omo-slim"] {
        apps.insert(
            app,
            AppCapabilities {
                providers: true,
                prompts: false,
                mcp: true,
                skills: true,
                usage: false,
                sessions: false,
                local_routing: false,
                additive_provider_mode: false,
                host_managed,
            },
        );
    }
    apps
}

fn full_switch_app(host_managed: bool, local_routing: bool, sessions: bool) -> AppCapabilities {
    AppCapabilities {
        providers: true,
        prompts: true,
        mcp: true,
        skills: true,
        usage: true,
        sessions,
        local_routing,
        additive_provider_mode: false,
        host_managed,
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeCapabilities;

    #[test]
    fn web_capabilities_expose_server_boundaries() {
        let capabilities = RuntimeCapabilities::web();
        assert_eq!(capabilities.runtime, "web");
        assert_eq!(capabilities.host, "server");
        assert!(!capabilities.features.directory_picker);
        assert!(!capabilities.features.tray);
        assert!(!capabilities.features.environment_management);
        assert!(!capabilities.features.app_update);
        assert!(capabilities.features.session_manager);
        assert!(capabilities.app_features["opencode"].additive_provider_mode);
        for app in ["omo", "omo-slim"] {
            let app = &capabilities.app_features[app];
            assert!(app.providers);
            assert!(app.mcp);
            assert!(app.skills);
            assert!(!app.prompts);
            assert!(!app.usage);
            assert!(!app.sessions);
            assert!(!app.local_routing);
        }
    }

    #[test]
    fn desktop_capabilities_keep_native_operations_explicit() {
        let capabilities = RuntimeCapabilities::desktop();
        assert_eq!(capabilities.runtime, "desktop");
        assert_eq!(capabilities.host, "local");
        assert!(capabilities.features.directory_picker);
        assert!(capabilities.features.endpoint_test);
        assert!(capabilities.features.tray);
        assert!(capabilities.features.environment_management);
        assert!(capabilities.features.app_update);
    }
}

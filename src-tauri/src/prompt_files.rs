use std::path::PathBuf;

use crate::app_config::AppType;
use crate::codex_config::get_codex_auth_path;
use crate::config::{get_claude_settings_path, get_home_dir};
use crate::error::AppError;
use crate::gemini_config::get_gemini_dir;

/// 返回指定应用所使用的提示词文件路径。
pub fn prompt_file_path(app: &AppType) -> Result<PathBuf, AppError> {
    let base_dir: PathBuf = match app {
        AppType::Claude => get_base_dir_with_fallback(get_claude_settings_path()?, ".claude")?,
        AppType::Codex => get_base_dir_with_fallback(get_codex_auth_path()?, ".codex")?,
        AppType::Gemini => get_gemini_dir()?,
        AppType::Opencode | AppType::Omo => {
            return Err(AppError::localized(
                "app_not_supported_yet",
                format!("应用 '{}' 暂未支持，敬请期待。", app.as_str()),
                format!("App '{}' is not supported yet.", app.as_str()),
            ))
        }
    };

    let filename = match app {
        AppType::Claude => "CLAUDE.md",
        AppType::Codex => "AGENTS.md",
        AppType::Gemini => "GEMINI.md",
        AppType::Opencode | AppType::Omo => unreachable!("unsupported app should return above"),
    };

    Ok(base_dir.join(filename))
}

fn get_base_dir_with_fallback(
    primary_path: PathBuf,
    fallback_dir: &str,
) -> Result<PathBuf, AppError> {
    primary_path
        .parent()
        .map(|p| p.to_path_buf())
        .or_else(|| get_home_dir().map(|h| h.join(fallback_dir)))
        .ok_or_else(|| {
            AppError::localized(
                "home_dir_not_found",
                format!("无法确定 {fallback_dir} 配置目录：用户主目录不存在"),
                format!("Cannot determine {fallback_dir} config directory: user home not found"),
            )
        })
}

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::AppError;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// 获取用户主目录。
///
/// 在 Windows 上，`dirs::home_dir()` 使用 Windows API 而非环境变量，
/// 导致测试无法通过设置 HOME/USERPROFILE 来隔离。此函数在 Windows 上
/// 优先检查环境变量，以支持测试隔离。
pub fn get_home_dir() -> Option<PathBuf> {
    // 在 Windows 上优先检查环境变量（用于测试隔离）
    #[cfg(windows)]
    {
        if let Ok(home) = std::env::var("HOME") {
            if !home.is_empty() {
                return Some(PathBuf::from(home));
            }
        }
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            if !userprofile.is_empty() {
                return Some(PathBuf::from(userprofile));
            }
        }
    }

    dirs::home_dir()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigDirSource {
    Override,
    ServiceHomeDefault,
    AccountHomeFallback,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDirInfo {
    pub dir: String,
    pub source: ConfigDirSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_home: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_home: Option<String>,
    #[serde(default)]
    pub home_mismatch: bool,
}

#[derive(Debug, Clone)]
struct ResolvedConfigDir {
    path: PathBuf,
    info: ConfigDirInfo,
}

#[cfg(unix)]
fn parse_passwd_home_dir(passwd_content: &str, username: &str) -> Option<PathBuf> {
    passwd_content.lines().find_map(|line| {
        let mut parts = line.split(':');
        let name = parts.next()?.trim();
        if name != username {
            return None;
        }

        let _password = parts.next()?;
        let _uid = parts.next()?;
        let _gid = parts.next()?;
        let _gecos = parts.next()?;
        let home = parts.next()?.trim();
        if home.is_empty() {
            return None;
        }

        Some(PathBuf::from(home))
    })
}

#[cfg(unix)]
pub fn get_account_home_dir() -> Option<PathBuf> {
    let username = std::env::var("USER")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("LOGNAME")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })?;
    let passwd = fs::read_to_string("/etc/passwd").ok()?;
    parse_passwd_home_dir(&passwd, username.trim())
}

#[cfg(not(unix))]
pub fn get_account_home_dir() -> Option<PathBuf> {
    None
}

fn resolve_client_config_dir_with_homes(
    override_dir: Option<PathBuf>,
    folder_name: &str,
    service_home: Option<PathBuf>,
    account_home: Option<PathBuf>,
    prefer_account_home: bool,
) -> Result<ResolvedConfigDir, AppError> {
    let home_mismatch = matches!(
        (&service_home, &account_home),
        (Some(service), Some(account)) if service != account
    );

    if let Some(custom) = override_dir {
        let dir = custom.to_string_lossy().to_string();
        return Ok(ResolvedConfigDir {
            path: custom,
            info: ConfigDirInfo {
                dir: dir.clone(),
                source: ConfigDirSource::Override,
                override_dir: Some(dir),
                service_home: service_home.map(|path| path.to_string_lossy().to_string()),
                account_home: account_home.map(|path| path.to_string_lossy().to_string()),
                home_mismatch,
            },
        });
    }

    let (base_home, source) = match (&service_home, &account_home) {
        (Some(service), Some(account)) if prefer_account_home && service != account => {
            (account.clone(), ConfigDirSource::AccountHomeFallback)
        }
        (Some(service), _) => (service.clone(), ConfigDirSource::ServiceHomeDefault),
        (None, Some(account)) => (account.clone(), ConfigDirSource::AccountHomeFallback),
        (None, None) => {
            return Err(AppError::Config("无法获取用户主目录".into()));
        }
    };

    let resolved = base_home.join(folder_name);
    Ok(ResolvedConfigDir {
        path: resolved.clone(),
        info: ConfigDirInfo {
            dir: resolved.to_string_lossy().to_string(),
            source,
            override_dir: None,
            service_home: service_home.map(|path| path.to_string_lossy().to_string()),
            account_home: account_home.map(|path| path.to_string_lossy().to_string()),
            home_mismatch,
        },
    })
}

fn resolve_client_config_dir(
    override_dir: Option<PathBuf>,
    folder_name: &str,
) -> Result<ResolvedConfigDir, AppError> {
    resolve_client_config_dir_with_homes(
        override_dir,
        folder_name,
        get_home_dir(),
        get_account_home_dir(),
        cfg!(feature = "web-server"),
    )
}

pub fn get_client_config_dir_path(
    override_dir: Option<PathBuf>,
    folder_name: &str,
) -> Result<PathBuf, AppError> {
    Ok(resolve_client_config_dir(override_dir, folder_name)?.path)
}

pub fn get_client_config_dir_info(
    override_dir: Option<PathBuf>,
    folder_name: &str,
) -> Result<ConfigDirInfo, AppError> {
    Ok(resolve_client_config_dir(override_dir, folder_name)?.info)
}

/// 获取 Claude Code 配置目录路径
pub fn get_claude_config_dir() -> Result<PathBuf, AppError> {
    get_client_config_dir_path(crate::settings::get_claude_override_dir(), ".claude")
}

pub fn get_claude_config_dir_info() -> Result<ConfigDirInfo, AppError> {
    get_client_config_dir_info(crate::settings::get_claude_override_dir(), ".claude")
}

/// 默认 Claude MCP 配置文件路径 (~/.claude.json)
pub fn get_default_claude_mcp_path() -> Result<PathBuf, AppError> {
    let home = get_home_dir().ok_or_else(|| AppError::Config("无法获取用户主目录".into()))?;
    Ok(home.join(".claude.json"))
}

fn derive_mcp_path_from_override(dir: &Path) -> Option<PathBuf> {
    let file_name = dir
        .file_name()
        .map(|name| name.to_string_lossy().to_string())?
        .trim()
        .to_string();
    if file_name.is_empty() {
        return None;
    }
    let parent = dir.parent().unwrap_or_else(|| Path::new(""));
    Some(parent.join(format!("{file_name}.json")))
}

/// 获取 Claude MCP 配置文件路径，若设置了目录覆盖则与覆盖目录同级
pub fn get_claude_mcp_path() -> Result<PathBuf, AppError> {
    if let Some(custom_dir) = crate::settings::get_claude_override_dir() {
        if let Some(path) = derive_mcp_path_from_override(&custom_dir) {
            return Ok(path);
        }
    }
    get_default_claude_mcp_path()
}

/// 获取 Claude Code 主配置文件路径
pub fn get_claude_settings_path() -> Result<PathBuf, AppError> {
    let dir = get_claude_config_dir()?;
    let settings = dir.join("settings.json");
    if settings.exists() {
        return Ok(settings);
    }
    // 兼容旧版命名：若存在旧文件则继续使用
    let legacy = dir.join("claude.json");
    if legacy.exists() {
        return Ok(legacy);
    }
    // 默认新建：回落到标准文件名 settings.json（不再生成 claude.json）
    Ok(settings)
}

/// 获取应用配置目录路径 (~/.cc-switch)
pub fn get_app_config_dir() -> Result<PathBuf, AppError> {
    #[cfg(feature = "desktop")]
    if let Some(custom) = crate::app_store::get_app_config_dir_override() {
        return Ok(custom);
    }

    let home = get_home_dir().ok_or_else(|| AppError::Config("无法获取用户主目录".into()))?;
    Ok(home.join(".cc-switch"))
}

/// 获取应用配置文件路径
pub fn get_app_config_path() -> Result<PathBuf, AppError> {
    Ok(get_app_config_dir()?.join("config.json"))
}

fn get_codex_config_dir_for_permissions() -> Option<PathBuf> {
    get_client_config_dir_path(crate::settings::get_codex_override_dir(), ".codex").ok()
}

fn get_gemini_config_dir_for_permissions() -> Option<PathBuf> {
    get_client_config_dir_path(crate::settings::get_gemini_override_dir(), ".gemini").ok()
}

fn get_opencode_config_dir_for_permissions() -> PathBuf {
    crate::opencode_config::get_opencode_dir()
}

fn should_enforce_private_permissions(path: &Path) -> bool {
    let mut dirs = Vec::new();
    if let Ok(dir) = get_app_config_dir() {
        dirs.push(dir);
    }
    if let Ok(dir) = get_claude_config_dir() {
        dirs.push(dir);
    }
    if let Some(dir) = get_codex_config_dir_for_permissions() {
        dirs.push(dir);
    }
    if let Some(dir) = get_gemini_config_dir_for_permissions() {
        dirs.push(dir);
    }
    dirs.push(get_opencode_config_dir_for_permissions());

    if dirs.iter().any(|dir| path.starts_with(dir)) {
        return true;
    }

    match get_claude_mcp_path() {
        Ok(claude_mcp_path) => path == claude_mcp_path,
        Err(_) => false,
    }
}

#[cfg(unix)]
fn enforce_private_permissions(path: &Path) -> std::io::Result<()> {
    fs::set_permissions(path, PermissionsExt::from_mode(0o600))
}

#[cfg(windows)]
fn enforce_private_permissions(path: &Path) -> std::io::Result<()> {
    use std::process::Command;

    let path_str = path.to_string_lossy();
    let output = Command::new("icacls")
        .args([&*path_str, "/inheritance:r", "/grant:r", "*S-1-3-4:F"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::warn!("Failed to set Windows file permissions: {}", stderr);
    }

    Ok(())
}

#[cfg(all(not(unix), not(windows)))]
fn enforce_private_permissions(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// 清理供应商名称，确保文件名安全
pub fn sanitize_provider_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '-',
            _ => c,
        })
        .collect::<String>()
        .to_lowercase()
}

/// 获取供应商配置文件路径
pub fn get_provider_config_path(
    provider_id: &str,
    provider_name: Option<&str>,
) -> Result<PathBuf, AppError> {
    let base_name = provider_name
        .map(sanitize_provider_name)
        .unwrap_or_else(|| sanitize_provider_name(provider_id));

    Ok(get_claude_config_dir()?.join(format!("settings-{base_name}.json")))
}

/// 读取 JSON 配置文件
pub fn read_json_file<T: for<'a> Deserialize<'a>>(path: &Path) -> Result<T, AppError> {
    if !path.exists() {
        return Err(AppError::Config(format!("文件不存在: {}", path.display())));
    }

    let content = fs::read_to_string(path).map_err(|e| AppError::io(path, e))?;

    serde_json::from_str(&content).map_err(|e| AppError::json(path, e))
}

/// 写入 JSON 配置文件
pub fn write_json_file<T: Serialize>(path: &Path, data: &T) -> Result<(), AppError> {
    // 确保目录存在
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::io(parent, e))?;
    }

    let json =
        serde_json::to_string_pretty(data).map_err(|e| AppError::JsonSerialize { source: e })?;

    atomic_write(path, json.as_bytes())
}

/// 原子写入文本文件（用于 TOML/纯文本）
pub fn write_text_file(path: &Path, data: &str) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::io(parent, e))?;
    }
    atomic_write(path, data.as_bytes())
}

/// 原子写入：写入临时文件后 rename 替换，避免半写状态
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::io(parent, e))?;
    }

    let parent = path
        .parent()
        .ok_or_else(|| AppError::Config("无效的路径".to_string()))?;
    let mut tmp = parent.to_path_buf();
    let file_name = path
        .file_name()
        .ok_or_else(|| AppError::Config("无效的文件名".to_string()))?
        .to_string_lossy()
        .to_string();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    tmp.push(format!("{file_name}.tmp.{ts}"));

    {
        let mut f = fs::File::create(&tmp).map_err(|e| AppError::io(&tmp, e))?;
        f.write_all(data).map_err(|e| AppError::io(&tmp, e))?;
        f.flush().map_err(|e| AppError::io(&tmp, e))?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(path) {
            let perm = meta.permissions().mode();
            let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(perm));
        }
    }

    #[cfg(windows)]
    {
        // Windows 原子替换：优先使用 std::fs::rename；目标存在时删除后重试
        if let Err(first_err) = fs::rename(&tmp, path) {
            if let Err(remove_err) = fs::remove_file(path) {
                if remove_err.kind() != std::io::ErrorKind::NotFound {
                    return Err(AppError::IoContext {
                        context: format!(
                            "原子替换失败，无法删除旧文件 {}: {}",
                            path.display(),
                            remove_err
                        ),
                        source: remove_err,
                    });
                }
            }

            fs::rename(&tmp, path).map_err(|e| AppError::IoContext {
                context: format!(
                    "原子替换失败: {} -> {}（初始错误: {}）",
                    tmp.display(),
                    path.display(),
                    first_err
                ),
                source: e,
            })?;
        }
    }

    #[cfg(not(windows))]
    {
        fs::rename(&tmp, path).map_err(|e| AppError::IoContext {
            context: format!("原子替换失败: {} -> {}", tmp.display(), path.display()),
            source: e,
        })?;
    }

    if should_enforce_private_permissions(path) {
        if let Err(err) = enforce_private_permissions(path) {
            log::warn!(
                "Failed to enforce private permissions on {}: {}",
                path.display(),
                err
            );
        }
    }
    Ok(())
}

/// 复制文件
pub fn copy_file(from: &Path, to: &Path) -> Result<(), AppError> {
    fs::copy(from, to).map_err(|e| AppError::IoContext {
        context: format!("复制文件失败 ({} -> {})", from.display(), to.display()),
        source: e,
    })?;

    if should_enforce_private_permissions(to) {
        if let Err(err) = enforce_private_permissions(to) {
            log::warn!(
                "Failed to enforce private permissions on {}: {}",
                to.display(),
                err
            );
        }
    }
    Ok(())
}

/// 删除文件
pub fn delete_file(path: &Path) -> Result<(), AppError> {
    if path.exists() {
        fs::remove_file(path).map_err(|e| AppError::io(path, e))?;
    }
    Ok(())
}

/// 检查 Claude Code 配置状态
#[derive(Serialize, Deserialize)]
pub struct ConfigStatus {
    pub exists: bool,
    pub path: String,
}

/// 获取 Claude Code 配置状态
pub fn get_claude_config_status() -> Result<ConfigStatus, AppError> {
    let path = get_claude_settings_path()?;
    Ok(ConfigStatus {
        exists: path.exists(),
        path: path.to_string_lossy().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;
    use tempfile::tempdir;

    // These tests mutate HOME/USERPROFILE, so run them serially to avoid
    // cross-test env races (especially on Windows).

    struct EnvGuard {
        key: &'static str,
        original: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let original = env::var(key).ok();
            env::set_var(key, value);
            Self { key, original }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(ref original) = self.original {
                env::set_var(self.key, original);
            } else {
                env::remove_var(self.key);
            }
        }
    }

    #[test]
    #[serial]
    fn test_get_home_dir() {
        let temp_dir = tempdir().expect("temp dir should be created");
        let home_str = temp_dir.path().to_string_lossy().to_string();
        let _home_guard = EnvGuard::set("HOME", &home_str);
        #[cfg(windows)]
        let _user_guard = EnvGuard::set("USERPROFILE", &home_str);

        let home = get_home_dir().expect("home dir should resolve");
        assert_eq!(home, temp_dir.path());
    }

    #[test]
    #[serial]
    fn test_get_app_config_path() {
        let temp_dir = tempdir().expect("temp dir should be created");
        let home_str = temp_dir.path().to_string_lossy().to_string();
        let _home_guard = EnvGuard::set("HOME", &home_str);
        #[cfg(windows)]
        let _user_guard = EnvGuard::set("USERPROFILE", &home_str);

        let app_config_path = get_app_config_path().expect("app config path should resolve");
        assert_eq!(
            app_config_path,
            temp_dir.path().join(".cc-switch").join("config.json")
        );
    }

    #[test]
    fn derive_mcp_path_from_override_preserves_folder_name() {
        let override_dir = PathBuf::from("/tmp/profile/.claude");
        let derived = derive_mcp_path_from_override(&override_dir)
            .expect("should derive path for nested dir");
        assert_eq!(derived, PathBuf::from("/tmp/profile/.claude.json"));
    }

    #[test]
    fn derive_mcp_path_from_override_handles_non_hidden_folder() {
        let override_dir = PathBuf::from("/data/claude-config");
        let derived = derive_mcp_path_from_override(&override_dir)
            .expect("should derive path for standard dir");
        assert_eq!(derived, PathBuf::from("/data/claude-config.json"));
    }

    #[test]
    fn derive_mcp_path_from_override_supports_relative_rootless_dir() {
        let override_dir = PathBuf::from("claude");
        let derived = derive_mcp_path_from_override(&override_dir)
            .expect("should derive path for single segment");
        assert_eq!(derived, PathBuf::from("claude.json"));
    }

    #[test]
    fn derive_mcp_path_from_root_like_dir_returns_none() {
        let override_dir = PathBuf::from("/");
        assert!(derive_mcp_path_from_override(&override_dir).is_none());
    }
}

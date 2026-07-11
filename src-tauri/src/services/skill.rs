use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use reqwest::{header, Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::{ErrorKind, Read};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use tokio::time::timeout;
use url::Url;

use crate::app_config::AppType;
use crate::config::{get_app_config_dir, get_home_dir, write_json_file};
use crate::error::format_skill_error;
use crate::settings;

const MAX_SKILL_SCAN_DEPTH: usize = 32;
const DEFAULT_SKILL_CACHE_TTL_SECS: u64 = 0;
const DEFAULT_MAX_ZIP_BYTES: u64 = 50 * 1024 * 1024;
const DEFAULT_MAX_ZIP_ENTRIES: usize = 20_000;
const DEFAULT_MAX_TOTAL_UNCOMPRESSED_BYTES: u64 = 500 * 1024 * 1024;
const DEFAULT_MAX_SINGLE_FILE_BYTES: u64 = 50 * 1024 * 1024;
const DEFAULT_MAX_COMPRESSION_RATIO: u64 = 200;
const DEFAULT_MAX_PATH_COMPONENTS: usize = 64;
const DEFAULT_MAX_PATH_LENGTH: usize = 240;
const SKILL_BACKUP_RETAIN_COUNT: usize = 20;

/// Skill 同步方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SyncMethod {
    /// 自动选择：优先 symlink，失败时回退到 copy
    #[default]
    Auto,
    /// 符号链接（推荐，节省磁盘空间）
    Symlink,
    /// 文件复制（兼容模式）
    Copy,
}

/// Skill 存储位置（SSOT 目录选择）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SkillStorageLocation {
    /// CC Switch Web 管理目录 (~/.cc-switch/skills/)
    #[default]
    CcSwitch,
    /// Agent Skills 统一标准目录 (~/.agents/skills/)
    Unified,
}

/// 技能对象
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// 唯一标识: "owner/name:directory" 或 "local:directory"
    pub key: String,
    /// 显示名称 (从 SKILL.md 解析)
    pub name: String,
    /// 技能描述
    pub description: String,
    /// 目录名称 (安装路径的相对路径，可能包含子目录)
    pub directory: String,
    /// 父目录路径 (相对技能根目录，包含嵌套信息)
    #[serde(rename = "parentPath", skip_serializing_if = "Option::is_none")]
    pub parent_path: Option<String>,
    /// 嵌套深度 (0 表示直接位于技能根目录)
    #[serde(default)]
    pub depth: usize,
    /// GitHub README URL
    #[serde(rename = "readmeUrl")]
    pub readme_url: Option<String>,
    /// 是否已安装
    pub installed: bool,
    /// 已安装到哪些客户端
    #[serde(
        rename = "installedApps",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub installed_apps: Vec<String>,
    /// 仓库所有者
    #[serde(rename = "repoOwner")]
    pub repo_owner: Option<String>,
    /// 仓库名称
    #[serde(rename = "repoName")]
    pub repo_name: Option<String>,
    /// 分支名称
    #[serde(rename = "repoBranch")]
    pub repo_branch: Option<String>,
    /// 技能所在的子目录路径 (可选, 如 "skills")
    #[serde(rename = "skillsPath")]
    pub skills_path: Option<String>,
    /// workflows 中的命令
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<SkillCommand>,
}

/// 技能 workflows 命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCommand {
    /// 命令名称
    pub name: String,
    /// 命令描述
    pub description: String,
    /// workflow 文件路径 (相对技能目录)
    #[serde(rename = "filePath")]
    pub file_path: String,
}

/// 仓库配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRepo {
    /// GitHub 用户/组织名
    pub owner: String,
    /// 仓库名称
    pub name: String,
    /// 分支 (默认 "main")
    pub branch: String,
    /// 是否启用
    pub enabled: bool,
    /// 技能所在的子目录路径 (可选, 如 "skills", "my-skills/subdir")
    #[serde(rename = "skillsPath")]
    pub skills_path: Option<String>,
}

/// 技能安装状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillState {
    /// 是否已安装
    pub installed: bool,
    /// 安装时间
    #[serde(rename = "installedAt")]
    pub installed_at: DateTime<Utc>,
    #[serde(rename = "repoOwner", default, skip_serializing_if = "Option::is_none")]
    pub repo_owner: Option<String>,
    #[serde(rename = "repoName", default, skip_serializing_if = "Option::is_none")]
    pub repo_name: Option<String>,
    #[serde(
        rename = "repoBranch",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub repo_branch: Option<String>,
    #[serde(
        rename = "skillsPath",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub skills_path: Option<String>,
}

/// 仓库技能缓存
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRepoCache {
    /// 缓存的技能列表
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<Skill>,
    /// 缓存时间
    #[serde(rename = "fetchedAt", alias = "cachedAt")]
    pub fetched_at: DateTime<Utc>,
    /// ETag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// Last-Modified
    #[serde(rename = "lastModified", skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
}

/// 缓存存储结构
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillCacheStore {
    #[serde(default)]
    pub repos: HashMap<String, SkillRepoCache>,
}

/// 持久化存储结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillStore {
    /// directory -> 安装状态
    pub skills: HashMap<String, SkillState>,
    /// 仓库列表
    pub repos: Vec<SkillRepo>,
    /// 仓库缓存
    #[serde(
        default,
        rename = "repoCache",
        skip_serializing_if = "HashMap::is_empty"
    )]
    pub repo_cache: HashMap<String, SkillRepoCache>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillBackupEntry {
    pub backup_id: String,
    pub backup_path: String,
    pub created_at: DateTime<Utc>,
    pub app: String,
    pub directory: String,
    pub name: String,
    pub description: String,
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationResult {
    pub migrated_count: usize,
    pub skipped_count: usize,
    pub errors: Vec<String>,
}

/// Skill 更新检测结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillUpdateInfo {
    pub id: String,
    pub name: String,
    pub directory: String,
    pub current_hash: Option<String>,
    pub remote_hash: String,
    pub installed_apps: Vec<String>,
}

/// skills.sh 公共目录搜索结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsShSearchResult {
    pub skills: Vec<SkillsShDiscoverableSkill>,
    pub total_count: usize,
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsShDiscoverableSkill {
    pub key: String,
    pub name: String,
    pub directory: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub repo_branch: String,
    pub installs: u64,
    pub readme_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SkillsShApiResponse {
    pub query: String,
    pub skills: Vec<SkillsShApiSkill>,
    pub count: usize,
}

#[derive(Debug, Deserialize)]
struct SkillsShApiSkill {
    pub id: String,
    #[serde(rename = "skillId")]
    pub skill_id: String,
    pub name: String,
    pub installs: u64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillBackupMetadata {
    created_at: DateTime<Utc>,
    app: String,
    directory: String,
    name: String,
    description: String,
    source_path: String,
}

#[derive(Debug, Clone)]
pub struct SkillListResult {
    pub skills: Vec<Skill>,
    pub warnings: Vec<String>,
    pub cache_hit: bool,
    pub refreshing: bool,
}

impl Default for SkillStore {
    fn default() -> Self {
        SkillStore {
            skills: HashMap::new(),
            repos: vec![
                SkillRepo {
                    owner: "ComposioHQ".to_string(),
                    name: "awesome-claude-skills".to_string(),
                    branch: "master".to_string(),
                    enabled: true,
                    skills_path: None, // 扫描根目录
                },
                SkillRepo {
                    owner: "anthropics".to_string(),
                    name: "skills".to_string(),
                    branch: "main".to_string(),
                    enabled: true,
                    skills_path: Some("skills".to_string()), // 扫描 skills 子目录，避免安装到 skills/skills/*
                },
                SkillRepo {
                    owner: "cexll".to_string(),
                    name: "myclaude".to_string(),
                    branch: "master".to_string(),
                    enabled: true,
                    skills_path: Some("skills".to_string()), // 扫描 skills 子目录
                },
            ],
            repo_cache: HashMap::new(),
        }
    }
}

/// 技能元数据 (从 SKILL.md 解析)
#[derive(Debug, Clone, Deserialize)]
pub struct SkillMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct WorkflowMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
}

pub struct SkillService {
    http_client: Client,
    install_dir: PathBuf,
    app: AppType,
    github_mirror_base_url: Option<String>,
}

#[derive(Debug, Clone)]
struct RepoCacheHeaders {
    etag: Option<String>,
    last_modified: Option<String>,
}

#[derive(Clone, Copy, Debug)]
struct ZipLimits {
    max_zip_bytes: u64,
    max_zip_entries: usize,
    max_total_uncompressed_bytes: u64,
    max_single_file_bytes: u64,
    max_compression_ratio: u64,
    max_path_components: usize,
    max_path_length: usize,
}

struct DownloadedRepo {
    temp_dir: tempfile::TempDir,
    etag: Option<String>,
    last_modified: Option<String>,
}

enum DownloadOutcome {
    Downloaded {
        etag: Option<String>,
        last_modified: Option<String>,
    },
    NotModified,
}

enum RepoDownloadResult {
    Downloaded(DownloadedRepo),
    NotModified,
}

enum RepoFetchOutcome {
    Updated {
        skills: Vec<Skill>,
        etag: Option<String>,
        last_modified: Option<String>,
    },
    NotModified,
}

impl SkillService {
    pub fn new() -> Result<Self> {
        Self::new_for_app(&AppType::Claude)
    }

    pub fn new_for_app(app: &AppType) -> Result<Self> {
        let install_dir = Self::get_ssot_dir()?;

        // 确保目录存在
        fs::create_dir_all(&install_dir)?;

        let http_client = Client::builder()
            .user_agent("cc-switch")
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(120))
            .build()?;

        Ok(Self {
            http_client,
            install_dir,
            app: app.clone(),
            github_mirror_base_url: Self::configured_github_mirror_base_url(),
        })
    }

    fn configured_github_mirror_base_url() -> Option<String> {
        let raw = settings::get_settings().network.github_mirror_base_url;
        Self::normalize_github_mirror_base_url(&raw)
    }

    fn normalize_github_mirror_base_url(raw: &str) -> Option<String> {
        let trimmed = raw.trim().trim_end_matches('/');
        if trimmed.is_empty() {
            return None;
        }

        let Ok(url) = Url::parse(trimmed) else {
            log::warn!("GitHub 镜像地址无效，已忽略: {trimmed}");
            return None;
        };
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            log::warn!("GitHub 镜像地址必须是 http(s) URL，已忽略: {trimmed}");
            return None;
        }

        Some(trimmed.to_string())
    }

    fn github_archive_url(owner: &str, name: &str, branch: &str) -> String {
        format!("https://github.com/{owner}/{name}/archive/refs/heads/{branch}.zip")
    }

    fn mirrored_github_archive_url(&self, owner: &str, name: &str, branch: &str) -> String {
        let url = Self::github_archive_url(owner, name, branch);
        match self.github_mirror_base_url.as_deref() {
            Some(mirror) => format!("{mirror}/{url}"),
            None => url,
        }
    }

    fn github_download_hint(&self) -> &'static str {
        if self.github_mirror_base_url.is_some() {
            "当前已配置 GitHub 镜像，请确认镜像地址可访问或临时切回 GitHub 原站。"
        } else {
            "中国大陆网络如访问 GitHub 不稳定，可在设置 > 高级 > 网络中配置 GitHub 镜像。"
        }
    }

    fn get_install_dir_for_app(app: &AppType) -> Result<PathBuf> {
        let home = get_home_dir().context(format_skill_error(
            "GET_HOME_DIR_FAILED",
            &[],
            Some("checkPermission"),
        ))?;
        let dir = match app {
            AppType::Claude => ".claude",
            AppType::Codex => ".codex",
            AppType::Gemini => ".gemini",
            AppType::Opencode => {
                return Ok(home.join(".config").join("opencode").join("skills"));
            }
            AppType::ClaudeDesktop | AppType::Omo | AppType::OmoSlim => {
                return Err(anyhow!(format_skill_error(
                    "APP_NOT_SUPPORTED",
                    &[("app", app.as_str())],
                    None,
                )))
            }
        };
        Ok(home.join(dir).join("skills"))
    }

    fn storage_dir_for_location(location: SkillStorageLocation) -> Result<PathBuf> {
        match location {
            SkillStorageLocation::CcSwitch => Ok(get_app_config_dir()?.join("skills")),
            SkillStorageLocation::Unified => {
                let home = get_home_dir().context(format_skill_error(
                    "GET_HOME_DIR_FAILED",
                    &[],
                    Some("checkPermission"),
                ))?;
                Ok(home.join(".agents").join("skills"))
            }
        }
    }

    /// 获取 SSOT 目录（根据设置返回 ~/.cc-switch/skills/ 或 ~/.agents/skills/）
    pub fn get_ssot_dir() -> Result<PathBuf> {
        let dir = Self::storage_dir_for_location(settings::get_skill_storage_location())?;
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub fn state_key(app: &AppType, directory: &str) -> String {
        format!("{}:{directory}", app.as_str())
    }

    fn state_matches_source(
        states: &HashMap<String, SkillState>,
        directory: &str,
        repo: &SkillRepo,
    ) -> bool {
        states.iter().any(|(state_key, state)| {
            if !state.installed {
                return false;
            }
            let state_directory = state_key
                .split_once(':')
                .map(|(_, directory)| directory)
                .unwrap_or(state_key);
            state_directory.eq_ignore_ascii_case(directory)
                && match (&state.repo_owner, &state.repo_name) {
                    (Some(owner), Some(name)) => {
                        owner.eq_ignore_ascii_case(&repo.owner)
                            && name.eq_ignore_ascii_case(&repo.name)
                    }
                    _ => true,
                }
        })
    }

    fn get_backup_dir() -> Result<PathBuf> {
        let dir = get_app_config_dir()?.join("skill-backups");
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    fn sanitize_backup_segment(segment: &str) -> String {
        let sanitized = segment
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                    ch
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches('-')
            .to_string();
        if sanitized.is_empty() {
            "skill".to_string()
        } else {
            sanitized
        }
    }

    fn backup_path_for_id(backup_id: &str) -> Result<PathBuf> {
        let value = backup_id.trim();
        if value.is_empty()
            || value.contains("..")
            || value.contains('/')
            || value.contains('\\')
            || !value
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        {
            return Err(anyhow!(format_skill_error(
                "SKILL_BACKUP_ID_INVALID",
                &[("backupId", backup_id)],
                Some("checkDirectory"),
            )));
        }
        Ok(Self::get_backup_dir()?.join(value))
    }

    fn read_backup_metadata(backup_path: &Path) -> Result<SkillBackupMetadata> {
        let metadata_path = backup_path.join("meta.json");
        let content = fs::read_to_string(&metadata_path)
            .with_context(|| format!("failed to read {}", metadata_path.display()))?;
        serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", metadata_path.display()))
    }

    fn backup_entry_from_path(path: &Path) -> Result<SkillBackupEntry> {
        let metadata = Self::read_backup_metadata(path)?;
        Ok(SkillBackupEntry {
            backup_id: path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default(),
            backup_path: path.to_string_lossy().to_string(),
            created_at: metadata.created_at,
            app: metadata.app,
            directory: metadata.directory,
            name: metadata.name,
            description: metadata.description,
            source_path: metadata.source_path,
        })
    }

    fn sanitize_install_name(raw: &str) -> Option<String> {
        let sanitized = raw
            .trim()
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                    ch
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches(['-', '.'])
            .to_string();
        if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
            None
        } else {
            Some(sanitized)
        }
    }

    fn cleanup_old_skill_backups(dir: &Path) -> Result<()> {
        let mut entries = fs::read_dir(dir)?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let path = entry.path();
                let metadata = fs::metadata(&path).ok()?;
                if metadata.is_dir() {
                    Some((path, metadata.modified().ok()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        if entries.len() <= SKILL_BACKUP_RETAIN_COUNT {
            return Ok(());
        }

        entries.sort_by_key(|(_, modified)| *modified);
        let remove_count = entries.len().saturating_sub(SKILL_BACKUP_RETAIN_COUNT);
        for (path, _) in entries.into_iter().take(remove_count) {
            fs::remove_dir_all(path)?;
        }
        Ok(())
    }

    fn installed_apps_for_directory(directory: &str) -> Vec<String> {
        [
            AppType::Claude,
            AppType::Codex,
            AppType::Gemini,
            AppType::Opencode,
        ]
        .into_iter()
        .filter_map(|app| {
            let install_dir = Self::get_install_dir_for_app(&app).ok()?;
            let skill_md = install_dir.join(directory).join("SKILL.md");
            if skill_md.is_file() {
                Some(app.as_str().to_string())
            } else {
                None
            }
        })
        .collect()
    }

    #[cfg(unix)]
    fn create_symlink(src: &Path, dest: &Path) -> Result<()> {
        std::os::unix::fs::symlink(src, dest)
            .with_context(|| format!("创建符号链接失败: {} -> {}", src.display(), dest.display()))
    }

    #[cfg(windows)]
    fn create_symlink(src: &Path, dest: &Path) -> Result<()> {
        std::os::windows::fs::symlink_dir(src, dest)
            .with_context(|| format!("创建符号链接失败: {} -> {}", src.display(), dest.display()))
    }

    fn is_symlink(path: &Path) -> bool {
        path.symlink_metadata()
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
    }

    fn remove_path(path: &Path) -> Result<()> {
        if Self::is_symlink(path) || path.is_file() {
            fs::remove_file(path)?;
        } else if path.is_dir() {
            fs::remove_dir_all(path)?;
        }
        Ok(())
    }

    fn app_skill_dir_for_current_app(&self, directory: &str) -> Result<PathBuf> {
        Self::validate_skill_directory(directory)?;
        Ok(Self::get_install_dir_for_app(&self.app)?.join(directory))
    }

    /// 同步 Skill 到当前应用目录（使用 symlink 或 copy）。
    pub fn sync_to_current_app(&self, directory: &str) -> Result<()> {
        Self::validate_skill_directory(directory)?;
        let source = self.install_dir.join(directory);
        if !source.join("SKILL.md").is_file() {
            return Err(anyhow!(format_skill_error(
                "SKILL_DIR_NOT_FOUND",
                &[("path", &source.display().to_string())],
                Some("checkDirectory"),
            )));
        }

        let app_dir = Self::get_install_dir_for_app(&self.app)?;
        fs::create_dir_all(&app_dir)?;
        let dest = app_dir.join(directory);
        if source == dest {
            return Ok(());
        }
        if dest.exists() || Self::is_symlink(&dest) {
            Self::remove_path(&dest)?;
        }

        match settings::get_skill_sync_method() {
            SyncMethod::Auto => match Self::create_symlink(&source, &dest) {
                Ok(()) => {
                    log::debug!(
                        "Skill {directory} 已通过 symlink 同步到 {}",
                        self.app.as_str()
                    );
                    Ok(())
                }
                Err(err) => {
                    log::warn!(
                        "Symlink 创建失败，将回退到文件复制: {} -> {}. 错误: {err:#}",
                        source.display(),
                        dest.display()
                    );
                    Self::copy_dir_recursive(&source, &dest)
                }
            },
            SyncMethod::Symlink => Self::create_symlink(&source, &dest),
            SyncMethod::Copy => Self::copy_dir_recursive(&source, &dest),
        }
    }
}

// 核心方法实现
impl SkillService {
    fn normalize_skills_path(skills_path: &str) -> Result<Option<String>> {
        let trimmed = skills_path.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        let trimmed = trimmed.trim_matches(|c| c == '/' || c == '\\');
        if trimmed.is_empty() {
            return Ok(None);
        }

        let normalized = trimmed.replace('\\', "/");
        let normalized_path = Path::new(&normalized);
        let has_traversal = normalized_path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        });

        if has_traversal {
            return Err(anyhow!(format_skill_error(
                "SKILL_PATH_INVALID",
                &[("path", skills_path)],
                Some("checkRepoUrl"),
            )));
        }

        Ok(Some(normalized))
    }

    pub(crate) fn validate_skill_directory(directory: &str) -> Result<()> {
        let trimmed = directory.trim();
        if trimmed.is_empty() {
            return Err(anyhow!(format_skill_error(
                "SKILL_DIR_INVALID",
                &[("directory", directory)],
                Some("checkDirectory"),
            )));
        }

        let path = Path::new(trimmed);
        let mut has_component = false;
        let mut has_invalid_component = false;

        for component in path.components() {
            match component {
                Component::Normal(_) => has_component = true,
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    has_invalid_component = true;
                }
            }
        }

        let has_traversal = trimmed.split(['/', '\\']).any(|segment| segment == "..");

        if !has_component || has_invalid_component || path.is_absolute() || has_traversal {
            return Err(anyhow!(format_skill_error(
                "SKILL_DIR_INVALID",
                &[("directory", directory)],
                Some("checkDirectory"),
            )));
        }

        Ok(())
    }

    pub(crate) fn resolve_install_target<'a>(
        skills: &'a [Skill],
        directory: &str,
    ) -> Result<&'a Skill, String> {
        let matches: Vec<&Skill> = skills
            .iter()
            .filter(|skill| skill.directory.eq_ignore_ascii_case(directory))
            .collect();
        if matches.len() > 1 {
            let mut sources: Vec<String> = matches
                .iter()
                .map(|skill| {
                    if let (Some(owner), Some(name)) = (&skill.repo_owner, &skill.repo_name) {
                        let branch = skill.repo_branch.as_deref().unwrap_or("main");
                        format!("{owner}/{name}@{branch}")
                    } else {
                        "local".to_string()
                    }
                })
                .collect();
            sources.sort();
            sources.dedup();
            let sources_joined = sources.join(", ");
            return Err(format_skill_error(
                "SKILL_INSTALL_PATH_CONFLICT",
                &[("directory", directory), ("sources", &sources_joined)],
                None,
            ));
        }

        matches.first().copied().ok_or_else(|| {
            format_skill_error(
                "SKILL_NOT_FOUND",
                &[("directory", directory)],
                Some("checkRepoUrl"),
            )
        })
    }

    fn relative_path_components(root: &Path, current_dir: &Path) -> Option<Vec<String>> {
        let relative = current_dir.strip_prefix(root).ok()?;
        let components: Vec<String> = relative
            .components()
            .filter_map(|component| match component {
                Component::Normal(os) => Some(os.to_string_lossy().to_string()),
                _ => None,
            })
            .collect();
        if components.is_empty() {
            None
        } else {
            Some(components)
        }
    }

    fn build_path_info(components: &[String]) -> (String, Option<String>, usize, String) {
        let directory = components.join("/");
        let depth = components.len().saturating_sub(1);
        let parent_path = if depth > 0 {
            Some(components[..depth].join("/"))
        } else {
            None
        };
        let leaf_name = components.last().cloned().unwrap_or_default();
        (directory, parent_path, depth, leaf_name)
    }

    fn cache_key(repo: &SkillRepo) -> String {
        let raw_path = repo.skills_path.as_deref().unwrap_or("");
        let normalized_path = raw_path
            .trim()
            .trim_matches(|c| c == '/' || c == '\\')
            .replace('\\', "/");
        if normalized_path.is_empty() {
            format!("{}/{}/{}", repo.owner, repo.name, repo.branch)
        } else {
            format!(
                "{}/{}/{}:{}",
                repo.owner, repo.name, repo.branch, normalized_path
            )
        }
    }

    fn cache_ttl() -> Duration {
        let default_ttl = Duration::from_secs(DEFAULT_SKILL_CACHE_TTL_SECS);
        let raw = match env::var("CC_SWITCH_SKILLS_CACHE_TTL_SECS") {
            Ok(value) => value,
            Err(_) => return default_ttl,
        };

        match raw.trim().parse::<u64>() {
            Ok(value) => Duration::from_secs(value),
            Err(_) => {
                log::warn!(
                    "环境变量 CC_SWITCH_SKILLS_CACHE_TTL_SECS 无法解析: {}，使用默认值 {} 秒",
                    raw,
                    DEFAULT_SKILL_CACHE_TTL_SECS
                );
                default_ttl
            }
        }
    }

    fn parse_env_usize(name: &str, default: usize) -> usize {
        let raw = match env::var(name) {
            Ok(value) => value,
            Err(_) => return default,
        };

        match raw.trim().parse::<usize>() {
            Ok(value) => value,
            Err(_) => {
                log::warn!(
                    "环境变量 {} 无法解析: {}，使用默认值 {}",
                    name,
                    raw,
                    default
                );
                default
            }
        }
    }

    fn parse_env_u64(name: &str, default: u64) -> u64 {
        let raw = match env::var(name) {
            Ok(value) => value,
            Err(_) => return default,
        };

        match raw.trim().parse::<u64>() {
            Ok(value) => value,
            Err(_) => {
                log::warn!(
                    "环境变量 {} 无法解析: {}，使用默认值 {}",
                    name,
                    raw,
                    default
                );
                default
            }
        }
    }

    fn zip_limits() -> ZipLimits {
        ZipLimits {
            max_zip_bytes: Self::parse_env_u64(
                "CC_SWITCH_SKILLS_MAX_ZIP_BYTES",
                DEFAULT_MAX_ZIP_BYTES,
            ),
            max_zip_entries: Self::parse_env_usize(
                "CC_SWITCH_SKILLS_MAX_ZIP_ENTRIES",
                DEFAULT_MAX_ZIP_ENTRIES,
            ),
            max_total_uncompressed_bytes: Self::parse_env_u64(
                "CC_SWITCH_SKILLS_MAX_TOTAL_UNCOMPRESSED_BYTES",
                DEFAULT_MAX_TOTAL_UNCOMPRESSED_BYTES,
            ),
            max_single_file_bytes: Self::parse_env_u64(
                "CC_SWITCH_SKILLS_MAX_SINGLE_FILE_BYTES",
                DEFAULT_MAX_SINGLE_FILE_BYTES,
            ),
            max_compression_ratio: Self::parse_env_u64(
                "CC_SWITCH_SKILLS_MAX_COMPRESSION_RATIO",
                DEFAULT_MAX_COMPRESSION_RATIO,
            ),
            max_path_components: Self::parse_env_usize(
                "CC_SWITCH_SKILLS_MAX_PATH_COMPONENTS",
                DEFAULT_MAX_PATH_COMPONENTS,
            ),
            max_path_length: Self::parse_env_usize(
                "CC_SWITCH_SKILLS_MAX_PATH_LENGTH",
                DEFAULT_MAX_PATH_LENGTH,
            ),
        }
    }

    fn is_cache_fresh(fetched_at: DateTime<Utc>) -> bool {
        let ttl_secs = Self::cache_ttl().as_secs() as i64;
        if ttl_secs == 0 {
            return false;
        }
        let elapsed = Utc::now().signed_duration_since(fetched_at);
        elapsed <= chrono::Duration::seconds(ttl_secs)
    }

    fn load_repo_cache(&self) -> SkillCacheStore {
        let cache_path = match get_app_config_dir() {
            Ok(dir) => dir.join("skills-cache.json"),
            Err(e) => {
                log::warn!("获取技能缓存目录失败: {}", e);
                return SkillCacheStore::default();
            }
        };

        let content = match fs::read_to_string(&cache_path) {
            Ok(content) => content,
            Err(e) => {
                if e.kind() != ErrorKind::NotFound {
                    log::warn!("读取技能缓存文件 {} 失败: {}", cache_path.display(), e);
                }
                return SkillCacheStore::default();
            }
        };

        match serde_json::from_str::<SkillCacheStore>(&content) {
            Ok(store) => store,
            Err(e) => {
                log::warn!("解析技能缓存文件 {} 失败: {}", cache_path.display(), e);
                SkillCacheStore::default()
            }
        }
    }

    fn save_repo_cache(&self, cache_store: &SkillCacheStore) {
        let cache_path = match get_app_config_dir() {
            Ok(dir) => dir.join("skills-cache.json"),
            Err(e) => {
                log::warn!("获取技能缓存目录失败: {}", e);
                return;
            }
        };

        if let Err(e) = write_json_file(&cache_path, cache_store) {
            log::warn!("写入技能缓存文件 {} 失败: {}", cache_path.display(), e);
        }
    }

    /// 列出所有技能
    pub async fn list_skills(
        &self,
        repos: Vec<SkillRepo>,
        repo_cache: &mut HashMap<String, SkillRepoCache>,
    ) -> Result<SkillListResult> {
        let mut skills = Vec::new();
        let mut warnings = Vec::new();
        let mut cache_store = self.load_repo_cache();
        let mut cache_updated = false;

        if !repo_cache.is_empty() {
            for (key, entry) in repo_cache.iter() {
                let should_replace = match cache_store.repos.get(key) {
                    None => true,
                    Some(existing) => entry.fetched_at > existing.fetched_at,
                };
                if should_replace {
                    cache_store.repos.insert(key.clone(), entry.clone());
                    cache_updated = true;
                }
            }
        }

        // 仅使用启用的仓库，并行获取技能列表，避免单个无效仓库拖慢整体刷新
        let enabled_repos: Vec<SkillRepo> = repos.into_iter().filter(|repo| repo.enabled).collect();
        let mut fetch_tasks = Vec::new();

        for repo in enabled_repos.iter().cloned() {
            let cache_key = Self::cache_key(&repo);
            let cached_entry = cache_store.repos.get(&cache_key).cloned();

            if let Some(entry) = cached_entry.as_ref() {
                if Self::is_cache_fresh(entry.fetched_at) {
                    skills.extend(entry.skills.clone());
                    continue;
                }
            }

            fetch_tasks.push(async move {
                let result = self
                    .fetch_repo_skills_with_cache(&repo, cached_entry.as_ref())
                    .await;
                (repo, cache_key, cached_entry, result)
            });
        }

        let refreshing = !fetch_tasks.is_empty();
        let cache_hit = !refreshing;

        let results: Vec<(
            SkillRepo,
            String,
            Option<SkillRepoCache>,
            Result<RepoFetchOutcome>,
        )> = futures::future::join_all(fetch_tasks).await;

        for (repo, cache_key, cached_entry, result) in results {
            match result {
                Ok(outcome) => match outcome {
                    RepoFetchOutcome::Updated {
                        skills: repo_skills,
                        etag,
                        last_modified,
                    } => {
                        let fetched_at = Utc::now();
                        skills.extend(repo_skills.clone());
                        cache_store.repos.insert(
                            cache_key,
                            SkillRepoCache {
                                fetched_at,
                                skills: repo_skills,
                                etag,
                                last_modified,
                            },
                        );
                        cache_updated = true;
                    }
                    RepoFetchOutcome::NotModified => {
                        if let Some(mut entry) = cached_entry {
                            entry.fetched_at = Utc::now();
                            skills.extend(entry.skills.clone());
                            cache_store.repos.insert(cache_key, entry);
                            cache_updated = true;
                        } else {
                            let warning = format!(
                                "仓库 {}/{} 返回 304，但本地没有缓存",
                                repo.owner, repo.name
                            );
                            log::warn!("{warning}");
                            warnings.push(warning);
                        }
                    }
                },
                Err(e) => {
                    if let Some(entry) = cached_entry {
                        let warning = format!(
                            "获取仓库 {}/{} 失败: {}，使用缓存",
                            repo.owner, repo.name, e
                        );
                        log::warn!("{warning}");
                        warnings.push(warning);
                        skills.extend(entry.skills);
                    } else {
                        let warning = format!("获取仓库 {}/{} 失败: {}", repo.owner, repo.name, e);
                        log::warn!("{warning}");
                        warnings.push(warning);
                    }
                }
            }
        }

        if cache_updated {
            self.save_repo_cache(&cache_store);
        }

        repo_cache.clear();
        repo_cache.extend(cache_store.repos.clone());

        // 合并本地技能
        self.merge_local_skills(&mut skills)?;

        // 去重并排序
        Self::deduplicate_skills(&mut skills);
        for skill in skills.iter_mut() {
            let installed_apps = Self::installed_apps_for_directory(&skill.directory);
            skill.installed = installed_apps
                .iter()
                .any(|app_id| app_id == self.app.as_str());
            skill.installed_apps = installed_apps;
        }
        skills.sort_by_key(|skill| skill.name.to_lowercase());

        Ok(SkillListResult {
            skills,
            warnings,
            cache_hit,
            refreshing,
        })
    }

    /// 从仓库获取技能列表
    async fn fetch_repo_skills_with_cache(
        &self,
        repo: &SkillRepo,
        cache_entry: Option<&SkillRepoCache>,
    ) -> Result<RepoFetchOutcome> {
        let cache_headers = cache_entry.map(|entry| RepoCacheHeaders {
            etag: entry.etag.clone(),
            last_modified: entry.last_modified.clone(),
        });

        // 为单个仓库加载增加整体超时，避免无效链接长时间阻塞
        let download_result = timeout(
            Duration::from_secs(180),
            self.download_repo(repo, cache_headers.as_ref()),
        )
        .await
        .map_err(|_| {
            anyhow!(format_skill_error(
                "DOWNLOAD_TIMEOUT",
                &[
                    ("owner", &repo.owner),
                    ("name", &repo.name),
                    ("timeout", "180")
                ],
                Some("checkNetwork"),
            ))
        })??;

        let download = match download_result {
            RepoDownloadResult::NotModified => {
                return Ok(RepoFetchOutcome::NotModified);
            }
            RepoDownloadResult::Downloaded(download) => download,
        };

        let temp_path = download.temp_dir.path().to_path_buf();
        let mut skills = Vec::new();

        let normalized_skills_path = match repo.skills_path.as_ref() {
            Some(skills_path) => match Self::normalize_skills_path(skills_path) {
                Ok(path) => path,
                Err(err) => {
                    return Err(err);
                }
            },
            None => None,
        };

        // 确定要扫描的目录路径
        let scan_dir = if let Some(ref normalized_skills_path) = normalized_skills_path {
            // 如果指定了 skillsPath，则扫描该子目录
            let subdir = temp_path.join(normalized_skills_path);
            if !subdir.exists() {
                log::warn!(
                    "仓库 {}/{} 中指定的技能路径 '{}' 不存在",
                    repo.owner,
                    repo.name,
                    repo.skills_path.as_deref().unwrap_or_default()
                );
                return Ok(RepoFetchOutcome::Updated {
                    skills,
                    etag: download.etag,
                    last_modified: download.last_modified,
                });
            }
            subdir
        } else {
            // 否则扫描仓库根目录
            temp_path.clone()
        };

        self.scan_skills_recursive(
            &scan_dir,
            &scan_dir,
            repo,
            normalized_skills_path.as_deref(),
            &mut skills,
        )?;

        Ok(RepoFetchOutcome::Updated {
            skills,
            etag: download.etag,
            last_modified: download.last_modified,
        })
    }

    /// 递归扫描目录树，查找所有 SKILL.md
    fn scan_skills_recursive(
        &self,
        scan_root: &Path,
        current_dir: &Path,
        repo: &SkillRepo,
        normalized_skills_path: Option<&str>,
        skills: &mut Vec<Skill>,
    ) -> Result<()> {
        let root_metadata = match fs::symlink_metadata(current_dir) {
            Ok(metadata) => metadata,
            Err(e) => {
                log::warn!("读取扫描目录 {} 元数据失败: {}", current_dir.display(), e);
                return Ok(());
            }
        };

        if root_metadata.file_type().is_symlink() {
            log::warn!("跳过符号链接目录 {}，避免路径穿越", current_dir.display());
            return Ok(());
        }

        if !root_metadata.is_dir() {
            return Ok(());
        }

        self.scan_skills_recursive_inner(
            scan_root,
            current_dir,
            repo,
            normalized_skills_path,
            skills,
            0,
        )
    }

    fn scan_skills_recursive_inner(
        &self,
        scan_root: &Path,
        current_dir: &Path,
        repo: &SkillRepo,
        normalized_skills_path: Option<&str>,
        skills: &mut Vec<Skill>,
        depth: usize,
    ) -> Result<()> {
        let (components, root_skill) = if current_dir == scan_root {
            if let Some(skills_path) = normalized_skills_path {
                let leaf = skills_path.rsplit('/').next().unwrap_or("").trim();
                if !leaf.is_empty() && leaf != "." {
                    (Some(vec![leaf.to_string()]), true)
                } else {
                    (None, false)
                }
            } else {
                (None, false)
            }
        } else {
            (
                Self::relative_path_components(scan_root, current_dir),
                false,
            )
        };

        if let Some(components) = components {
            let skill_md = current_dir.join("SKILL.md");
            match fs::symlink_metadata(&skill_md) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() {
                        log::warn!("跳过符号链接文件 {}，避免路径穿越", skill_md.display());
                    } else if metadata.is_file() {
                        match self.parse_skill_metadata(&skill_md) {
                            Ok(meta) => {
                                let (directory, parent_path, depth, leaf_name) =
                                    Self::build_path_info(&components);
                                if !directory.is_empty() {
                                    let readme_path =
                                        if let Some(skills_path) = normalized_skills_path {
                                            if root_skill {
                                                skills_path.to_string()
                                            } else {
                                                format!("{}/{}", skills_path, directory)
                                            }
                                        } else {
                                            directory.clone()
                                        };
                                    let commands = match self.scan_workflow_commands(current_dir) {
                                        Ok(commands) => commands,
                                        Err(e) => {
                                            log::warn!(
                                                "扫描 {} workflows 失败: {}",
                                                current_dir.display(),
                                                e
                                            );
                                            Vec::new()
                                        }
                                    };

                                    skills.push(Skill {
                                        key: format!("{}/{}:{}", repo.owner, repo.name, directory),
                                        name: meta.name.unwrap_or_else(|| leaf_name.clone()),
                                        description: meta.description.unwrap_or_default(),
                                        directory,
                                        parent_path,
                                        depth,
                                        readme_url: Some(format!(
                                            "https://github.com/{}/{}/tree/{}/{}",
                                            repo.owner, repo.name, repo.branch, readme_path
                                        )),
                                        installed: false,
                                        installed_apps: Vec::new(),
                                        repo_owner: Some(repo.owner.clone()),
                                        repo_name: Some(repo.name.clone()),
                                        repo_branch: Some(repo.branch.clone()),
                                        skills_path: repo.skills_path.clone(),
                                        commands,
                                    });
                                }
                            }
                            Err(e) => log::warn!("解析 {} 元数据失败: {}", skill_md.display(), e),
                        }
                    }
                }
                Err(e) => {
                    if e.kind() != ErrorKind::NotFound {
                        log::warn!("读取 {} 元数据失败: {}", skill_md.display(), e);
                    }
                }
            }
        }

        if depth >= MAX_SKILL_SCAN_DEPTH {
            log::warn!(
                "扫描目录 {} 已达到最大深度 {}, 停止向下递归",
                current_dir.display(),
                MAX_SKILL_SCAN_DEPTH
            );
            return Ok(());
        }

        let entries = match fs::read_dir(current_dir) {
            Ok(entries) => entries,
            Err(e) => {
                log::warn!("读取目录 {} 失败: {}", current_dir.display(), e);
                return Ok(());
            }
        };

        for entry_result in entries {
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(e) => {
                    log::warn!("读取目录项 {} 失败: {}", current_dir.display(), e);
                    continue;
                }
            };
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(e) => {
                    log::warn!("读取 {} 类型失败: {}", entry.path().display(), e);
                    continue;
                }
            };
            if !file_type.is_dir() || file_type.is_symlink() {
                continue;
            }
            self.scan_skills_recursive_inner(
                scan_root,
                &entry.path(),
                repo,
                normalized_skills_path,
                skills,
                depth + 1,
            )?;
        }

        Ok(())
    }

    /// 解析技能元数据
    fn parse_skill_metadata(&self, path: &Path) -> Result<SkillMetadata> {
        let content = fs::read_to_string(path)?;

        // 移除 BOM
        let content = content.trim_start_matches('\u{feff}');

        // 提取 YAML front matter
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() < 3 {
            return Ok(SkillMetadata {
                name: None,
                description: None,
            });
        }

        let front_matter = parts[1].trim();
        let meta: SkillMetadata = serde_yaml::from_str(front_matter).unwrap_or(SkillMetadata {
            name: None,
            description: None,
        });

        Ok(meta)
    }

    fn scan_workflow_commands(&self, skill_dir: &Path) -> Result<Vec<SkillCommand>> {
        let workflows_dir = skill_dir.join("workflows");
        if !workflows_dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut commands = Vec::new();
        for entry in fs::read_dir(&workflows_dir)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_file() {
                continue;
            }

            let path = entry.path();
            let ext = path.extension().and_then(|ext| ext.to_str());
            if !ext
                .map(|ext| ext.eq_ignore_ascii_case("md"))
                .unwrap_or(false)
            {
                continue;
            }

            let relative_path = path
                .strip_prefix(skill_dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");

            match self.parse_workflow_command(&path, relative_path) {
                Ok(command) => commands.push(command),
                Err(e) => log::warn!("解析 {} workflow 命令失败: {}", path.display(), e),
            }
        }

        commands.sort_by(|a, b| {
            let name_cmp = a.name.to_lowercase().cmp(&b.name.to_lowercase());
            if name_cmp == std::cmp::Ordering::Equal {
                a.file_path.cmp(&b.file_path)
            } else {
                name_cmp
            }
        });

        Ok(commands)
    }

    fn parse_workflow_command(&self, path: &Path, file_path: String) -> Result<SkillCommand> {
        let content = fs::read_to_string(path)?;
        let content = content.trim_start_matches('\u{feff}');

        let (front_matter, body) = Self::split_front_matter(content);
        let mut name = None;
        let mut description = None;

        if let Some(front_matter) = front_matter {
            let meta: WorkflowMetadata =
                serde_yaml::from_str(front_matter).unwrap_or(WorkflowMetadata {
                    name: None,
                    description: None,
                });
            name = meta.name;
            description = meta.description;
        }

        let body = body.trim_start_matches(['\n', '\r']);
        if name.is_none() || description.is_none() {
            let (heading, summary) = Self::extract_markdown_heading_and_summary(body);
            if name.is_none() {
                name = heading;
            }
            if description.is_none() {
                description = summary;
            }
        }

        let fallback_name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("command");
        let name = name
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| fallback_name.to_string());
        let description = description.unwrap_or_default();

        Ok(SkillCommand {
            name,
            description,
            file_path,
        })
    }

    fn split_front_matter(content: &str) -> (Option<&str>, &str) {
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() < 3 {
            (None, content)
        } else {
            (Some(parts[1].trim()), parts[2])
        }
    }

    fn extract_markdown_heading_and_summary(body: &str) -> (Option<String>, Option<String>) {
        let mut heading = None;
        let mut summary = None;

        for line in body.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if heading.is_none() {
                if let Some(stripped) = trimmed.strip_prefix('#') {
                    let title = stripped.trim_start_matches('#').trim();
                    if !title.is_empty() {
                        heading = Some(title.to_string());
                        continue;
                    }
                }
            }

            if heading.is_some() && summary.is_none() && !trimmed.starts_with('#') {
                summary = Some(trimmed.to_string());
                break;
            }

            if heading.is_none() && summary.is_none() && !trimmed.starts_with('#') {
                summary = Some(trimmed.to_string());
                break;
            }
        }

        (heading, summary)
    }

    /// 合并本地技能
    fn merge_local_skills(&self, skills: &mut Vec<Skill>) -> Result<()> {
        if !self.install_dir.exists() {
            return Ok(());
        }

        for skill in skills.iter_mut() {
            let skill_path = self.install_dir.join(&skill.directory);
            if skill_path.join("SKILL.md").is_file() {
                skill.installed = true;
            }
        }

        self.merge_local_skills_recursive(&self.install_dir, &self.install_dir, skills)?;

        Ok(())
    }

    fn merge_local_skills_recursive(
        &self,
        scan_root: &Path,
        current_dir: &Path,
        skills: &mut Vec<Skill>,
    ) -> Result<()> {
        self.merge_local_skills_recursive_inner(scan_root, current_dir, skills, 0)
    }

    fn merge_local_skills_recursive_inner(
        &self,
        scan_root: &Path,
        current_dir: &Path,
        skills: &mut Vec<Skill>,
        depth: usize,
    ) -> Result<()> {
        if let Some(components) = Self::relative_path_components(scan_root, current_dir) {
            let skill_md = current_dir.join("SKILL.md");
            match fs::symlink_metadata(&skill_md) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() {
                        log::warn!("跳过符号链接文件 {}，避免路径穿越", skill_md.display());
                    } else if metadata.is_file() {
                        let (directory, parent_path, depth, leaf_name) =
                            Self::build_path_info(&components);
                        let exists = skills
                            .iter()
                            .any(|skill| skill.directory.eq_ignore_ascii_case(&directory));
                        if !exists {
                            if let Ok(meta) = self.parse_skill_metadata(&skill_md) {
                                let commands = match self.scan_workflow_commands(current_dir) {
                                    Ok(commands) => commands,
                                    Err(e) => {
                                        log::warn!(
                                            "扫描 {} workflows 失败: {}",
                                            current_dir.display(),
                                            e
                                        );
                                        Vec::new()
                                    }
                                };
                                skills.push(Skill {
                                    key: format!("local:{directory}"),
                                    name: meta.name.unwrap_or_else(|| leaf_name.clone()),
                                    description: meta.description.unwrap_or_default(),
                                    directory,
                                    parent_path,
                                    depth,
                                    readme_url: None,
                                    installed: true,
                                    installed_apps: vec![self.app.as_str().to_string()],
                                    repo_owner: None,
                                    repo_name: None,
                                    repo_branch: None,
                                    skills_path: None,
                                    commands,
                                });
                            }
                        }
                    }
                }
                Err(e) => {
                    if e.kind() != ErrorKind::NotFound {
                        log::warn!("读取 {} 元数据失败: {}", skill_md.display(), e);
                    }
                }
            }
        }

        if depth >= MAX_SKILL_SCAN_DEPTH {
            log::warn!(
                "扫描目录 {} 已达到最大深度 {}, 停止向下递归",
                current_dir.display(),
                MAX_SKILL_SCAN_DEPTH
            );
            return Ok(());
        }

        let entries = match fs::read_dir(current_dir) {
            Ok(entries) => entries,
            Err(e) => {
                log::warn!("读取目录 {} 失败: {}", current_dir.display(), e);
                return Ok(());
            }
        };

        for entry_result in entries {
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(e) => {
                    log::warn!("读取目录项 {} 失败: {}", current_dir.display(), e);
                    continue;
                }
            };
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(e) => {
                    log::warn!("读取 {} 类型失败: {}", entry.path().display(), e);
                    continue;
                }
            };
            if !file_type.is_dir() || file_type.is_symlink() {
                continue;
            }
            self.merge_local_skills_recursive_inner(scan_root, &entry.path(), skills, depth + 1)?;
        }

        Ok(())
    }

    fn scan_skill_dirs_in_dir(root: &Path) -> Result<Vec<PathBuf>> {
        let mut dirs = Vec::new();
        Self::scan_skill_dirs_in_dir_inner(root, root, &mut dirs, 0)?;
        dirs.sort();
        dirs.dedup();
        Ok(dirs)
    }

    fn scan_skill_dirs_in_dir_inner(
        root: &Path,
        current_dir: &Path,
        dirs: &mut Vec<PathBuf>,
        depth: usize,
    ) -> Result<()> {
        let metadata = match fs::symlink_metadata(current_dir) {
            Ok(metadata) => metadata,
            Err(err) => {
                log::warn!("读取 ZIP 解压目录 {} 失败: {err}", current_dir.display());
                return Ok(());
            }
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Ok(());
        }

        let skill_md = current_dir.join("SKILL.md");
        if skill_md.is_file() {
            dirs.push(current_dir.to_path_buf());
        }

        if depth >= MAX_SKILL_SCAN_DEPTH {
            return Ok(());
        }

        let entries = match fs::read_dir(current_dir) {
            Ok(entries) => entries,
            Err(err) => {
                log::warn!("读取 ZIP 解压子目录 {} 失败: {err}", current_dir.display());
                return Ok(());
            }
        };
        for entry in entries {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_dir() && !file_type.is_symlink() {
                let path = entry.path();
                if path.starts_with(root) {
                    Self::scan_skill_dirs_in_dir_inner(root, &path, dirs, depth + 1)?;
                }
            }
        }
        Ok(())
    }

    /// 去重技能列表
    fn deduplicate_skills(skills: &mut Vec<Skill>) {
        let mut seen = HashSet::new();
        skills.retain(|skill| {
            // key 已包含 owner/name:directory 或 local:directory，使用它避免不同仓库同名目录被误去重
            let key = skill.key.to_lowercase();
            seen.insert(key)
        });
    }

    /// 下载仓库
    async fn download_repo(
        &self,
        repo: &SkillRepo,
        cache_headers: Option<&RepoCacheHeaders>,
    ) -> Result<RepoDownloadResult> {
        // 尝试多个分支
        let branches = if repo.branch.is_empty() {
            vec!["main", "master"]
        } else {
            vec![repo.branch.as_str(), "main", "master"]
        };

        let mut last_error = None;
        for branch in branches {
            let temp_dir = tempfile::tempdir()?;
            let url = self.mirrored_github_archive_url(&repo.owner, &repo.name, branch);

            match self
                .download_and_extract(&url, temp_dir.path(), cache_headers)
                .await
            {
                Ok(DownloadOutcome::Downloaded {
                    etag,
                    last_modified,
                }) => {
                    return Ok(RepoDownloadResult::Downloaded(DownloadedRepo {
                        temp_dir,
                        etag,
                        last_modified,
                    }));
                }
                Ok(DownloadOutcome::NotModified) => {
                    return Ok(RepoDownloadResult::NotModified);
                }
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            };
        }

        let hint = self.github_download_hint();
        match last_error {
            Some(error) => Err(anyhow::anyhow!("{error}\n{hint}")),
            None => Err(anyhow::anyhow!("所有分支下载失败\n{hint}")),
        }
    }

    /// 下载并解压 ZIP
    async fn download_and_extract(
        &self,
        url: &str,
        dest: &Path,
        cache_headers: Option<&RepoCacheHeaders>,
    ) -> Result<DownloadOutcome> {
        // 下载 ZIP
        let mut request = self.http_client.get(url);
        if let Some(headers) = cache_headers {
            if let Some(etag) = headers.etag.as_deref() {
                request = request.header(header::IF_NONE_MATCH, etag);
            }
            if let Some(last_modified) = headers.last_modified.as_deref() {
                request = request.header(header::IF_MODIFIED_SINCE, last_modified);
            }
        }

        let response = request.send().await?;
        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(DownloadOutcome::NotModified);
        }
        if !response.status().is_success() {
            let status = response.status().as_u16().to_string();
            return Err(anyhow::anyhow!(format_skill_error(
                "DOWNLOAD_FAILED",
                &[("status", &status)],
                match status.as_str() {
                    "403" => Some("http403"),
                    "404" => Some("http404"),
                    "429" => Some("http429"),
                    _ => Some("checkNetwork"),
                },
            )));
        }

        let limits = Self::zip_limits();
        if let Some(content_length) = response.content_length() {
            if content_length > limits.max_zip_bytes {
                return Err(anyhow::anyhow!(format_skill_error(
                    "ZIP_TOO_LARGE",
                    &[
                        ("contentLength", &content_length.to_string()),
                        ("maxBytes", &limits.max_zip_bytes.to_string())
                    ],
                    Some("checkRepoUrl"),
                )));
            }
        }

        let etag = response
            .headers()
            .get(header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string());
        let last_modified = response
            .headers()
            .get(header::LAST_MODIFIED)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string());

        let mut bytes = Vec::new();
        let mut total_bytes: u64 = 0;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            total_bytes = total_bytes.saturating_add(chunk.len() as u64);
            if total_bytes > limits.max_zip_bytes {
                return Err(anyhow::anyhow!(format_skill_error(
                    "ZIP_TOO_LARGE",
                    &[
                        ("receivedBytes", &total_bytes.to_string()),
                        ("maxBytes", &limits.max_zip_bytes.to_string())
                    ],
                    Some("checkRepoUrl"),
                )));
            }
            bytes.extend_from_slice(&chunk);
        }
        let dest = dest.to_path_buf();
        tokio::task::spawn_blocking(move || Self::extract_zip_to_dir(bytes, dest, limits))
            .await??;

        Ok(DownloadOutcome::Downloaded {
            etag,
            last_modified,
        })
    }

    fn extract_zip_to_dir(bytes: Vec<u8>, dest: PathBuf, limits: ZipLimits) -> Result<()> {
        // 解压
        let cursor = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor)?;

        // 获取根目录名称 (GitHub 的 zip 会有一个根目录)
        let entry_count = archive.len();
        if entry_count > limits.max_zip_entries {
            return Err(anyhow::anyhow!(format_skill_error(
                "ZIP_TOO_MANY_ENTRIES",
                &[
                    ("entries", &entry_count.to_string()),
                    ("maxEntries", &limits.max_zip_entries.to_string())
                ],
                Some("checkRepoUrl"),
            )));
        }

        if entry_count == 0 {
            return Err(anyhow::anyhow!(format_skill_error(
                "EMPTY_ARCHIVE",
                &[],
                Some("checkRepoUrl"),
            )));
        }

        let mut common_root: Option<String> = None;
        for i in 0..entry_count {
            let file = archive.by_index(i)?;
            let name = file.name();
            if !name.contains('/') {
                common_root = None;
                break;
            }
            let first_component = name.split('/').next().unwrap_or("");
            if first_component.is_empty() {
                common_root = None;
                break;
            }
            match &common_root {
                None => common_root = Some(first_component.to_string()),
                Some(root) => {
                    if root != first_component {
                        common_root = None;
                        break;
                    }
                }
            }
        }

        let mut total_uncompressed_bytes: u64 = 0;
        let mut extracted_count: usize = 0;

        // 解压所有文件
        for i in 0..entry_count {
            let file = archive.by_index(i)?;
            let file_path = file.name();

            let relative_path = if let Some(root) = common_root.as_deref() {
                if let Some(stripped) = file_path.strip_prefix(&format!("{root}/")) {
                    stripped
                } else if file_path == root {
                    ""
                } else {
                    file_path
                }
            } else {
                file_path
            };

            if relative_path.is_empty() {
                continue;
            }

            let relative_path = relative_path.to_string();
            let relative_path_obj = Path::new(&relative_path);
            let has_traversal = relative_path_obj.components().any(|c| {
                matches!(
                    c,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            }) || relative_path
                .split(['/', '\\'])
                .any(|segment| segment == "..");

            if relative_path_obj.is_absolute() || has_traversal {
                return Err(anyhow!(format_skill_error(
                    "INVALID_ARCHIVE_PATH",
                    &[("path", file_path)],
                    Some("checkRepoUrl"),
                )));
            }

            let component_count = relative_path_obj
                .components()
                .filter(|component| matches!(component, Component::Normal(_)))
                .count();
            if component_count > limits.max_path_components {
                return Err(anyhow!(format_skill_error(
                    "ZIP_PATH_TOO_DEEP",
                    &[
                        ("path", &relative_path),
                        ("components", &component_count.to_string()),
                        ("maxComponents", &limits.max_path_components.to_string())
                    ],
                    Some("checkRepoUrl"),
                )));
            }

            if relative_path.len() > limits.max_path_length {
                return Err(anyhow!(format_skill_error(
                    "ZIP_PATH_TOO_LONG",
                    &[
                        ("path", &relative_path),
                        ("length", &relative_path.len().to_string()),
                        ("maxLength", &limits.max_path_length.to_string())
                    ],
                    Some("checkRepoUrl"),
                )));
            }

            let outpath = dest.join(relative_path_obj);

            if file.is_dir() {
                fs::create_dir_all(&outpath)?;
                extracted_count = extracted_count.saturating_add(1);
            } else {
                let file_size = file.size();
                if file_size > limits.max_single_file_bytes {
                    return Err(anyhow!(format_skill_error(
                        "ZIP_FILE_TOO_LARGE",
                        &[
                            ("path", &relative_path),
                            ("size", &file_size.to_string()),
                            ("maxBytes", &limits.max_single_file_bytes.to_string())
                        ],
                        Some("checkRepoUrl"),
                    )));
                }

                let compressed_size = file.compressed_size();
                if compressed_size == 0 && file_size > 0 {
                    return Err(anyhow!(format_skill_error(
                        "ZIP_INVALID_COMPRESSION",
                        &[
                            ("path", &relative_path),
                            ("size", &file_size.to_string()),
                            ("compressedSize", "0")
                        ],
                        Some("checkRepoUrl"),
                    )));
                }
                if compressed_size > 0 {
                    if let Some(max_allowed) =
                        compressed_size.checked_mul(limits.max_compression_ratio)
                    {
                        if file_size > max_allowed {
                            return Err(anyhow!(format_skill_error(
                                "ZIP_COMPRESSION_RATIO_TOO_HIGH",
                                &[
                                    ("path", &relative_path),
                                    ("size", &file_size.to_string()),
                                    ("compressedSize", &compressed_size.to_string()),
                                    ("maxRatio", &limits.max_compression_ratio.to_string())
                                ],
                                Some("checkRepoUrl"),
                            )));
                        }
                    }
                }

                total_uncompressed_bytes = total_uncompressed_bytes.saturating_add(file_size);
                if total_uncompressed_bytes > limits.max_total_uncompressed_bytes {
                    return Err(anyhow!(format_skill_error(
                        "ZIP_TOTAL_TOO_LARGE",
                        &[
                            ("totalBytes", &total_uncompressed_bytes.to_string()),
                            ("maxBytes", &limits.max_total_uncompressed_bytes.to_string())
                        ],
                        Some("checkRepoUrl"),
                    )));
                }

                if let Some(parent) = outpath.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut outfile = fs::File::create(&outpath)?;
                let mut limited_reader = file.take(limits.max_single_file_bytes.saturating_add(1));
                let written = std::io::copy(&mut limited_reader, &mut outfile)?;
                if written > limits.max_single_file_bytes {
                    return Err(anyhow!(format_skill_error(
                        "ZIP_FILE_TOO_LARGE",
                        &[
                            ("path", &relative_path),
                            ("size", &written.to_string()),
                            ("maxBytes", &limits.max_single_file_bytes.to_string())
                        ],
                        Some("checkRepoUrl"),
                    )));
                }
                extracted_count = extracted_count.saturating_add(1);
            }
        }

        if extracted_count == 0 {
            return Err(anyhow!(format_skill_error(
                "ZIP_NO_ENTRIES_EXTRACTED",
                &[],
                Some("checkRepoUrl"),
            )));
        }

        Ok(())
    }

    /// 安装技能（仅负责下载和文件操作，状态更新由上层负责）
    pub async fn install_skill(
        &self,
        directory: String,
        repo: SkillRepo,
        force: bool,
    ) -> Result<()> {
        Self::validate_skill_directory(&directory)?;
        let dest = self.install_dir.join(&directory);

        // SSOT 已有该 Skill 时仍需同步到当前目标应用，避免 Web/headless 下
        // “库里有但当前 app 未启用”的状态。
        if !dest.exists() || force {
            // 下载仓库时增加总超时，防止无效链接导致长时间卡住安装过程
            let temp_dir = timeout(
                std::time::Duration::from_secs(180),
                self.download_repo(&repo, None),
            )
            .await
            .map_err(|_| {
                anyhow!(format_skill_error(
                    "DOWNLOAD_TIMEOUT",
                    &[
                        ("owner", &repo.owner),
                        ("name", &repo.name),
                        ("timeout", "180")
                    ],
                    Some("checkNetwork"),
                ))
            })??;
            let temp_dir = match temp_dir {
                RepoDownloadResult::Downloaded(download) => download.temp_dir,
                RepoDownloadResult::NotModified => {
                    return Err(anyhow::anyhow!(format_skill_error(
                        "DOWNLOAD_FAILED",
                        &[("status", "304")],
                        Some("checkNetwork"),
                    )));
                }
            };
            let temp_path = temp_dir.path().to_path_buf();

            // 根据 skills_path 确定源目录路径
            let source = Self::resolve_install_source_path(
                &temp_path,
                &directory,
                repo.skills_path.as_deref(),
            )?;

            if !source.exists() {
                return Err(anyhow::anyhow!(format_skill_error(
                    "SKILL_DIR_NOT_FOUND",
                    &[("path", &source.display().to_string())],
                    Some("checkRepoUrl"),
                )));
            }

            Self::install_from_source(&source, &dest, force)?;
        }

        self.sync_to_current_app(&directory)?;

        Ok(())
    }

    fn resolve_install_source_path(
        temp_path: &Path,
        directory: &str,
        skills_path: Option<&str>,
    ) -> Result<PathBuf> {
        let normalized_skills_path = match skills_path {
            Some(skills_path) => Self::normalize_skills_path(skills_path)?,
            None => None,
        };

        let source = match normalized_skills_path {
            Some(path) => {
                let skills_leaf = path.rsplit('/').next().unwrap_or("");
                let directory_leaf = directory
                    .rsplit(|c| ['/', '\\'].contains(&c))
                    .next()
                    .unwrap_or("");
                if !skills_leaf.is_empty()
                    && !directory_leaf.is_empty()
                    && skills_leaf.eq_ignore_ascii_case(directory_leaf)
                {
                    temp_path.join(path)
                } else {
                    temp_path.join(path).join(directory)
                }
            }
            None => temp_path.join(directory),
        };

        if source.join("SKILL.md").is_file() {
            return Ok(source);
        }

        fn find_by_name(current: &Path, target: &str, depth: usize) -> Option<PathBuf> {
            if depth > MAX_SKILL_SCAN_DEPTH {
                return None;
            }
            for entry in fs::read_dir(current).ok()?.flatten() {
                let file_type = entry.file_type().ok()?;
                if !file_type.is_dir() || file_type.is_symlink() {
                    continue;
                }
                let path = entry.path();
                if entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(target)
                    && path.join("SKILL.md").is_file()
                {
                    return Some(path);
                }
                if let Some(found) = find_by_name(&path, target, depth + 1) {
                    return Some(found);
                }
            }
            None
        }

        let search_root = match skills_path {
            Some(path) => Self::normalize_skills_path(path)?
                .map(|path| temp_path.join(path))
                .unwrap_or_else(|| temp_path.to_path_buf()),
            None => temp_path.to_path_buf(),
        };
        let target = Path::new(directory)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(directory);
        if let Some(found) = find_by_name(&search_root, target, 0) {
            return Ok(found);
        }
        if search_root.join("SKILL.md").is_file() {
            return Ok(search_root);
        }

        Ok(source)
    }

    fn install_from_source(source: &Path, dest: &Path, force: bool) -> Result<bool> {
        if dest.exists() {
            if !force {
                return Ok(false);
            }
            fs::remove_dir_all(dest)?;
        }

        Self::copy_dir_recursive(source, dest)?;

        Ok(true)
    }

    /// 递归复制目录
    fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
        fs::create_dir_all(dest)?;

        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let path = entry.path();
            let dest_path = dest.join(entry.file_name());
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                log::warn!("跳过技能目录中的符号链接: {}", path.display());
                continue;
            }

            if file_type.is_dir() {
                Self::copy_dir_recursive(&path, &dest_path)?;
            } else if file_type.is_file() {
                fs::copy(&path, &dest_path)?;
            }
        }

        Ok(())
    }

    /// 计算 Skill 目录的稳定 SHA-256。隐藏文件和符号链接不参与计算，
    /// 与上游的更新检测规则保持一致。
    pub fn compute_dir_hash(dir: &Path) -> Result<String> {
        use sha2::{Digest, Sha256};

        fn collect(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
            for entry in fs::read_dir(current)? {
                let entry = entry?;
                let name = entry.file_name();
                if name.to_string_lossy().starts_with('.') {
                    continue;
                }
                let file_type = entry.file_type()?;
                if file_type.is_symlink() {
                    continue;
                }
                let path = entry.path();
                if file_type.is_dir() {
                    collect(root, &path, files)?;
                } else if file_type.is_file() && path.starts_with(root) {
                    files.push(path);
                }
            }
            Ok(())
        }

        let mut files = Vec::new();
        collect(dir, dir, &mut files)?;
        files.sort();

        let mut hasher = Sha256::new();
        for path in files {
            let relative = path.strip_prefix(dir).unwrap_or(&path);
            hasher.update(relative.to_string_lossy().replace('\\', "/").as_bytes());
            hasher.update(b"\0");
            hasher.update(fs::read(&path)?);
            hasher.update(b"\0");
        }
        Ok(format!("{:x}", hasher.finalize()))
    }

    /// 检查所有由仓库安装且当前至少在一个 App 中启用的 Skill。
    /// 每个仓库只下载一次，避免逐 Skill 重复拉取。
    pub async fn check_updates(
        &self,
        repos: &[SkillRepo],
        states: &HashMap<String, SkillState>,
    ) -> Result<Vec<SkillUpdateInfo>> {
        let mut updates = Vec::new();

        for repo in repos.iter().filter(|repo| repo.enabled) {
            let downloaded =
                match timeout(Duration::from_secs(180), self.download_repo(repo, None)).await {
                    Ok(Ok(RepoDownloadResult::Downloaded(download))) => download,
                    Ok(Ok(RepoDownloadResult::NotModified)) => continue,
                    Ok(Err(err)) => {
                        log::warn!("检查 {}/{} 更新失败: {err:#}", repo.owner, repo.name);
                        continue;
                    }
                    Err(_) => {
                        log::warn!("检查 {}/{} 更新超时", repo.owner, repo.name);
                        continue;
                    }
                };

            let root = downloaded.temp_dir.path();
            let normalized_path = repo
                .skills_path
                .as_deref()
                .map(Self::normalize_skills_path)
                .transpose()?
                .flatten();
            let scan_root = normalized_path
                .as_deref()
                .map(|path| root.join(path))
                .unwrap_or_else(|| root.to_path_buf());
            if !scan_root.is_dir() {
                continue;
            }

            let mut remote_skills = Vec::new();
            self.scan_skills_recursive(
                &scan_root,
                &scan_root,
                repo,
                normalized_path.as_deref(),
                &mut remote_skills,
            )?;

            for remote in remote_skills {
                let source_is_installed =
                    Self::state_matches_source(states, &remote.directory, repo);
                if !source_is_installed {
                    continue;
                }
                let installed_apps = Self::installed_apps_for_directory(&remote.directory);
                if installed_apps.is_empty() {
                    continue;
                }
                let local_dir = self.install_dir.join(&remote.directory);
                if !local_dir.join("SKILL.md").is_file() {
                    continue;
                }
                let source = Self::resolve_install_source_path(
                    root,
                    &remote.directory,
                    repo.skills_path.as_deref(),
                )?;
                if !source.join("SKILL.md").is_file() {
                    continue;
                }
                let current_hash = Self::compute_dir_hash(&local_dir).ok();
                let remote_hash = Self::compute_dir_hash(&source)?;
                if current_hash.as_deref() != Some(remote_hash.as_str()) {
                    updates.push(SkillUpdateInfo {
                        id: remote.key,
                        name: remote.name,
                        directory: remote.directory,
                        current_hash,
                        remote_hash,
                        installed_apps,
                    });
                }
            }
        }

        updates.sort_by_key(|update| update.name.to_lowercase());
        updates.dedup_by(|left, right| left.id.eq_ignore_ascii_case(&right.id));
        Ok(updates)
    }

    /// 更新一个 Skill。先复制到同文件系统暂存目录，再用 rename 替换 SSOT；
    /// 任一步失败都会恢复旧目录，并重新同步已经启用的 App。
    pub async fn update_skill(
        &self,
        repos: &[SkillRepo],
        states: &HashMap<String, SkillState>,
        id: &str,
    ) -> Result<SkillUpdateInfo> {
        let repo = repos
            .iter()
            .filter(|repo| repo.enabled)
            .find(|repo| id.starts_with(&format!("{}/{}:", repo.owner, repo.name)))
            .ok_or_else(|| anyhow!("Skill source not found: {id}"))?;
        let directory = id
            .strip_prefix(&format!("{}/{}:", repo.owner, repo.name))
            .ok_or_else(|| anyhow!("Invalid Skill id: {id}"))?;
        Self::validate_skill_directory(directory)?;

        let source_is_installed = Self::state_matches_source(states, directory, repo);
        if !source_is_installed {
            return Err(anyhow!(
                "Skill source does not match installed record: {id}"
            ));
        }

        let installed_apps = Self::installed_apps_for_directory(directory);
        if installed_apps.is_empty() {
            return Err(anyhow!("Skill is not installed: {directory}"));
        }

        let downloaded = timeout(Duration::from_secs(180), self.download_repo(repo, None))
            .await
            .map_err(|_| anyhow!("Skill update download timed out: {id}"))??;
        let downloaded = match downloaded {
            RepoDownloadResult::Downloaded(downloaded) => downloaded,
            RepoDownloadResult::NotModified => {
                return Err(anyhow!("Skill update returned an empty 304 response: {id}"));
            }
        };
        let source = Self::resolve_install_source_path(
            downloaded.temp_dir.path(),
            directory,
            repo.skills_path.as_deref(),
        )?;
        if !source.join("SKILL.md").is_file() {
            return Err(anyhow!(format_skill_error(
                "SKILL_DIR_NOT_FOUND",
                &[("path", &source.display().to_string())],
                Some("checkRepoUrl"),
            )));
        }

        let destination = self.install_dir.join(directory);
        let current_hash = Self::compute_dir_hash(&destination).ok();
        let remote_hash = Self::compute_dir_hash(&source)?;
        let metadata = self.parse_skill_metadata(&source.join("SKILL.md"))?;
        if current_hash.as_deref() == Some(remote_hash.as_str()) {
            return Ok(SkillUpdateInfo {
                id: id.to_string(),
                name: metadata.name.unwrap_or_else(|| directory.to_string()),
                directory: directory.to_string(),
                current_hash,
                remote_hash,
                installed_apps,
            });
        }

        let _ = self.backup_skill_before_uninstall(directory)?;
        let parent = destination
            .parent()
            .ok_or_else(|| anyhow!("Invalid Skill destination: {}", destination.display()))?;
        fs::create_dir_all(parent)?;
        let nonce = Utc::now().timestamp_nanos_opt().unwrap_or_default();
        let leaf = destination
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("skill");
        let staging = parent.join(format!(".{leaf}.update-{nonce}"));
        let rollback = parent.join(format!(".{leaf}.rollback-{nonce}"));
        Self::copy_dir_recursive(&source, &staging)?;

        let replace_result = (|| -> Result<()> {
            fs::rename(&destination, &rollback)?;
            if let Err(err) = fs::rename(&staging, &destination) {
                let _ = fs::rename(&rollback, &destination);
                return Err(err.into());
            }
            for app_id in &installed_apps {
                let app = AppType::parse_skills_app(app_id)?;
                Self::new_for_app(&app)?.sync_to_current_app(directory)?;
            }
            Ok(())
        })();

        if let Err(err) = replace_result {
            let _ = Self::remove_path(&staging);
            if rollback.exists() {
                let _ = Self::remove_path(&destination);
                let _ = fs::rename(&rollback, &destination);
                for app_id in &installed_apps {
                    if let Ok(app) = AppType::parse_skills_app(app_id) {
                        let _ = Self::new_for_app(&app)
                            .and_then(|service| service.sync_to_current_app(directory));
                    }
                }
            }
            return Err(err);
        }
        let _ = Self::remove_path(&rollback);

        Ok(SkillUpdateInfo {
            id: id.to_string(),
            name: metadata.name.unwrap_or_else(|| directory.to_string()),
            directory: directory.to_string(),
            current_hash,
            remote_hash,
            installed_apps,
        })
    }

    /// 搜索 skills.sh 公共目录。只接收可映射到 GitHub owner/repo 的结果，
    /// 与上游保持相同的来源过滤规则。
    pub async fn search_skills_sh(
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<SkillsShSearchResult> {
        let query = query.trim();
        if query.len() < 2 {
            return Ok(SkillsShSearchResult {
                skills: Vec::new(),
                total_count: 0,
                query: query.to_string(),
            });
        }
        let limit = limit.clamp(1, 100);
        let url = Url::parse_with_params(
            "https://skills.sh/api/search",
            &[
                ("q", query.to_string()),
                ("limit", limit.to_string()),
                ("offset", offset.to_string()),
            ],
        )?;
        let response = Client::builder()
            .user_agent("cc-switch-web")
            .timeout(Duration::from_secs(15))
            .build()?
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json::<SkillsShApiResponse>()
            .await?;
        let skills = response
            .skills
            .into_iter()
            .filter_map(|skill| {
                let (owner, repo) = skill.source.split_once('/')?;
                if owner.contains('.') || repo.contains('.') || owner.is_empty() || repo.is_empty()
                {
                    return None;
                }
                Some(SkillsShDiscoverableSkill {
                    key: skill.id,
                    name: skill.name,
                    directory: skill.skill_id,
                    repo_owner: owner.to_string(),
                    repo_name: repo.to_string(),
                    repo_branch: "main".to_string(),
                    installs: skill.installs,
                    readme_url: Some(format!("https://github.com/{owner}/{repo}")),
                })
            })
            .collect();
        Ok(SkillsShSearchResult {
            skills,
            total_count: response.count,
            query: response.query,
        })
    }

    pub fn list_backups() -> Result<Vec<SkillBackupEntry>> {
        let backup_dir = Self::get_backup_dir()?;
        let mut entries = Vec::new();

        for entry in fs::read_dir(&backup_dir)? {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    log::warn!("读取 Skill 备份目录项失败: {err}");
                    continue;
                }
            };
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            match Self::backup_entry_from_path(&path) {
                Ok(entry) => entries.push(entry),
                Err(err) => log::warn!("解析 Skill 备份失败 {}: {err:#}", path.display()),
            }
        }

        entries.sort_by_key(|entry| std::cmp::Reverse(entry.created_at));
        Ok(entries)
    }

    pub fn delete_backup(backup_id: &str) -> Result<()> {
        let backup_path = Self::backup_path_for_id(backup_id)?;
        if !backup_path.is_dir() {
            return Err(anyhow!(format_skill_error(
                "SKILL_BACKUP_NOT_FOUND",
                &[("backupId", backup_id)],
                Some("checkDirectory"),
            )));
        }
        fs::remove_dir_all(&backup_path)?;
        Ok(())
    }

    pub fn backup_skill_before_uninstall(
        &self,
        directory: &str,
    ) -> Result<Option<SkillBackupEntry>> {
        Self::validate_skill_directory(directory)?;
        let app_source = self.app_skill_dir_for_current_app(directory)?;
        let ssot_source = self.install_dir.join(directory);
        let source = if Self::is_symlink(&app_source) {
            &ssot_source
        } else if app_source.is_dir() {
            &app_source
        } else {
            &ssot_source
        };
        if !source.is_dir() {
            return Ok(None);
        }

        let metadata = self
            .parse_skill_metadata(&source.join("SKILL.md"))
            .unwrap_or(SkillMetadata {
                name: None,
                description: None,
            });
        let created_at = Utc::now();
        let slug = Self::sanitize_backup_segment(directory);
        let app_segment = Self::sanitize_backup_segment(self.app.as_str());
        let timestamp = created_at.format("%Y%m%d_%H%M%S");
        let backup_root = Self::get_backup_dir()?;
        let mut backup_path = backup_root.join(format!("{timestamp}_{app_segment}_{slug}"));
        let mut counter = 1;
        while backup_path.exists() {
            backup_path = backup_root.join(format!("{timestamp}_{app_segment}_{slug}_{counter}"));
            counter += 1;
        }

        let skill_backup_dir = backup_path.join("skill");
        if let Err(err) = Self::copy_dir_recursive(source, &skill_backup_dir) {
            let _ = fs::remove_dir_all(&backup_path);
            return Err(err);
        }

        let backup_metadata = SkillBackupMetadata {
            created_at,
            app: self.app.as_str().to_string(),
            directory: directory.to_string(),
            name: metadata.name.unwrap_or_else(|| {
                Path::new(directory)
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| directory.to_string())
            }),
            description: metadata.description.unwrap_or_default(),
            source_path: source.to_string_lossy().to_string(),
        };
        let metadata_path = backup_path.join("meta.json");
        let metadata_json = serde_json::to_string_pretty(&backup_metadata)?;
        fs::write(&metadata_path, metadata_json)?;

        if let Err(err) = Self::cleanup_old_skill_backups(&backup_root) {
            log::warn!("清理旧 Skill 备份失败: {err:#}");
        }

        Self::backup_entry_from_path(&backup_path).map(Some)
    }

    pub fn restore_backup(&self, backup_id: &str, force: bool) -> Result<SkillBackupEntry> {
        let backup_path = Self::backup_path_for_id(backup_id)?;
        let metadata = Self::read_backup_metadata(&backup_path)?;
        Self::validate_skill_directory(&metadata.directory)?;
        let backup_skill_dir = backup_path.join("skill");
        if !backup_skill_dir.join("SKILL.md").is_file() {
            return Err(anyhow!(format_skill_error(
                "SKILL_BACKUP_INVALID",
                &[("backupId", backup_id)],
                Some("checkDirectory"),
            )));
        }

        let dest = self.install_dir.join(&metadata.directory);
        if dest.exists() {
            if !force {
                return Err(anyhow!(format_skill_error(
                    "SKILL_ALREADY_INSTALLED",
                    &[("directory", &metadata.directory)],
                    Some("confirmOverwrite"),
                )));
            }
            fs::remove_dir_all(&dest)?;
        }

        Self::copy_dir_recursive(&backup_skill_dir, &dest)?;
        self.sync_to_current_app(&metadata.directory)?;
        Self::backup_entry_from_path(&backup_path)
    }

    pub fn install_from_zip_file(&self, zip_path: &Path, force: bool) -> Result<Vec<Skill>> {
        let bytes = fs::read(zip_path)
            .with_context(|| format!("Failed to read ZIP file: {}", zip_path.display()))?;
        let archive_name = zip_path.file_stem().and_then(|value| value.to_str());
        self.install_from_zip_bytes(bytes, archive_name, force)
    }

    pub fn install_from_zip_bytes(
        &self,
        bytes: Vec<u8>,
        archive_name: Option<&str>,
        force: bool,
    ) -> Result<Vec<Skill>> {
        let temp_dir = tempfile::tempdir()?;
        Self::extract_zip_to_dir(bytes, temp_dir.path().to_path_buf(), Self::zip_limits())?;
        let skill_dirs = Self::scan_skill_dirs_in_dir(temp_dir.path())?;
        if skill_dirs.is_empty() {
            return Err(anyhow!(format_skill_error(
                "NO_SKILLS_IN_ZIP",
                &[],
                Some("checkZipContent"),
            )));
        }

        let mut installed = Vec::new();
        let mut seen_names = HashSet::new();
        for skill_dir in skill_dirs {
            let metadata = self
                .parse_skill_metadata(&skill_dir.join("SKILL.md"))
                .unwrap_or(SkillMetadata {
                    name: None,
                    description: None,
                });
            let dir_name = if skill_dir == temp_dir.path() {
                None
            } else {
                skill_dir.file_name().and_then(|value| value.to_str())
            };
            let install_name = dir_name
                .and_then(Self::sanitize_install_name)
                .or_else(|| {
                    metadata
                        .name
                        .as_deref()
                        .and_then(Self::sanitize_install_name)
                })
                .or_else(|| archive_name.and_then(Self::sanitize_install_name))
                .ok_or_else(|| {
                    anyhow!(format_skill_error(
                        "INVALID_SKILL_DIRECTORY",
                        &[("archive", archive_name.unwrap_or("uploaded.zip"))],
                        Some("checkZipContent"),
                    ))
                })?;

            if !seen_names.insert(install_name.to_lowercase()) {
                log::warn!("ZIP 内存在重复 Skill 安装目录 {install_name}，已跳过后续项");
                continue;
            }

            let dest = self.install_dir.join(&install_name);
            if dest.exists() && !force {
                log::warn!("Skill {install_name} 已存在，未启用 force，跳过导入");
                continue;
            }

            Self::install_from_source(&skill_dir, &dest, force)?;
            self.sync_to_current_app(&install_name)?;
            let commands = match self.scan_workflow_commands(&dest) {
                Ok(commands) => commands,
                Err(err) => {
                    log::warn!("扫描导入 Skill workflows 失败 {}: {err}", dest.display());
                    Vec::new()
                }
            };

            installed.push(Skill {
                key: format!("local:{install_name}"),
                name: metadata.name.unwrap_or_else(|| install_name.clone()),
                description: metadata.description.unwrap_or_default(),
                directory: install_name.clone(),
                parent_path: None,
                depth: 0,
                readme_url: None,
                installed: true,
                installed_apps: Self::installed_apps_for_directory(&install_name),
                repo_owner: None,
                repo_name: None,
                repo_branch: None,
                skills_path: None,
                commands,
            });
        }

        if installed.is_empty() {
            return Err(anyhow!(format_skill_error(
                "NO_SKILLS_INSTALLED_FROM_ZIP",
                &[],
                Some("confirmOverwrite"),
            )));
        }

        Ok(installed)
    }

    /// 卸载技能（仅负责文件操作，状态更新由上层负责）
    pub fn uninstall_skill(&self, directory: String) -> Result<()> {
        Self::validate_skill_directory(&directory)?;
        let dest = self.app_skill_dir_for_current_app(&directory)?;

        if dest.exists() || Self::is_symlink(&dest) {
            Self::remove_path(&dest)?;
        }

        if Self::installed_apps_for_directory(&directory).is_empty() {
            let ssot_dest = self.install_dir.join(&directory);
            if ssot_dest.exists() || Self::is_symlink(&ssot_dest) {
                Self::remove_path(&ssot_dest)?;
            }
        }

        Ok(())
    }

    /// 列出仓库
    pub fn list_repos(&self, store: &SkillStore) -> Vec<SkillRepo> {
        store.repos.clone()
    }

    /// 迁移 Skill SSOT 存储位置，并刷新已安装应用目录的同步目标。
    pub fn migrate_storage(target: SkillStorageLocation) -> Result<MigrationResult> {
        let current = settings::get_skill_storage_location();
        if current == target {
            return Ok(MigrationResult {
                migrated_count: 0,
                skipped_count: 0,
                errors: vec![],
            });
        }

        let old_dir = Self::storage_dir_for_location(current)?;
        let new_dir = Self::storage_dir_for_location(target)?;
        fs::create_dir_all(&new_dir)?;

        let skill_dirs = if old_dir.exists() {
            Self::scan_skill_dirs_in_dir(&old_dir)?
        } else {
            Vec::new()
        };
        let mut installed_targets: Vec<(String, Vec<AppType>)> = Vec::new();
        for skill_dir in &skill_dirs {
            let Ok(relative) = skill_dir.strip_prefix(&old_dir) else {
                continue;
            };
            if relative.as_os_str().is_empty() {
                continue;
            }
            let directory = relative.to_string_lossy().replace('\\', "/");
            let apps = [
                AppType::Claude,
                AppType::Codex,
                AppType::Gemini,
                AppType::Opencode,
            ]
            .into_iter()
            .filter(|app| {
                Self::get_install_dir_for_app(app)
                    .map(|dir| dir.join(&directory).join("SKILL.md").is_file())
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();
            installed_targets.push((directory, apps));
        }

        let mut result = MigrationResult {
            migrated_count: 0,
            skipped_count: 0,
            errors: vec![],
        };

        for skill_dir in skill_dirs {
            let Ok(relative) = skill_dir.strip_prefix(&old_dir) else {
                result.skipped_count += 1;
                continue;
            };
            if relative.as_os_str().is_empty() {
                result.skipped_count += 1;
                continue;
            }
            let dst = new_dir.join(relative);
            if dst.exists() {
                result.skipped_count += 1;
                continue;
            }
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            match fs::rename(&skill_dir, &dst) {
                Ok(()) => result.migrated_count += 1,
                Err(_) => match Self::copy_dir_recursive(&skill_dir, &dst) {
                    Ok(()) => {
                        let _ = fs::remove_dir_all(&skill_dir);
                        result.migrated_count += 1;
                    }
                    Err(err) => {
                        result.errors.push(format!(
                            "{}: {err:#}",
                            skill_dir
                                .strip_prefix(&old_dir)
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|_| skill_dir.display().to_string())
                        ));
                    }
                },
            }
        }

        settings::set_skill_storage_location(target)?;

        for (directory, apps) in installed_targets {
            for app in apps {
                match Self::new_for_app(&app)
                    .and_then(|service| service.sync_to_current_app(&directory))
                {
                    Ok(()) => {}
                    Err(err) => {
                        result
                            .errors
                            .push(format!("sync {}/{}: {err:#}", app.as_str(), directory))
                    }
                }
            }
        }

        Ok(result)
    }

    /// 添加仓库
    pub fn add_repo(&self, store: &mut SkillStore, repo: SkillRepo) -> Result<()> {
        // 检查重复
        if let Some(pos) = store
            .repos
            .iter()
            .position(|r| r.owner == repo.owner && r.name == repo.name)
        {
            store.repos[pos] = repo;
        } else {
            store.repos.push(repo);
        }

        Ok(())
    }

    /// 删除仓库
    pub fn remove_repo(&self, store: &mut SkillStore, owner: String, name: String) -> Result<()> {
        store
            .repos
            .retain(|r| !(r.owner == owner && r.name == name));

        Ok(())
    }

    pub fn normalize_default_repos(store: &mut SkillStore) -> bool {
        let mut updated = false;

        for repo in store.repos.iter_mut() {
            if repo.owner.eq_ignore_ascii_case("anthropics")
                && repo.name.eq_ignore_ascii_case("skills")
                && repo.skills_path.as_deref().unwrap_or("").trim().is_empty()
            {
                repo.skills_path = Some("skills".to_string());
                updated = true;
            }
        }

        updated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use serial_test::serial;
    use std::io::Write;
    use zip::write::FileOptions;

    struct EnvGuard {
        key: &'static str,
        original: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let original = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, original }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.original {
                std::env::set_var(self.key, value);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn build_service_with_install_dir(dir: PathBuf) -> SkillService {
        SkillService {
            http_client: Client::builder()
                .user_agent("cc-switch-test")
                .build()
                .expect("client build should succeed"),
            install_dir: dir,
            app: AppType::Claude,
            github_mirror_base_url: None,
        }
    }

    fn make_skill(key: &str, directory: &str) -> Skill {
        Skill {
            key: key.to_string(),
            name: directory.to_string(),
            description: String::new(),
            directory: directory.to_string(),
            parent_path: None,
            depth: 0,
            readme_url: None,
            installed: false,
            installed_apps: Vec::new(),
            repo_owner: None,
            repo_name: None,
            repo_branch: None,
            skills_path: None,
            commands: Vec::new(),
        }
    }

    #[test]
    fn test_normalize_skills_path() {
        let normalized = SkillService::normalize_skills_path("/skills\\nested//")
            .expect("normalize should succeed");
        assert_eq!(normalized, Some("skills/nested".to_string()));
    }

    #[test]
    fn test_default_anthropics_skills_repo_scans_skills_subdir() {
        let store = SkillStore::default();
        let repo = store
            .repos
            .iter()
            .find(|repo| repo.owner == "anthropics" && repo.name == "skills")
            .expect("default anthropics skills repo should exist");

        assert_eq!(repo.skills_path.as_deref(), Some("skills"));
    }

    #[test]
    fn test_github_archive_url_uses_origin_by_default() {
        assert_eq!(
            SkillService::github_archive_url("owner", "repo", "main"),
            "https://github.com/owner/repo/archive/refs/heads/main.zip"
        );
    }

    #[test]
    fn test_mirrored_github_archive_url_prefixes_origin_url() {
        let temp_dir = tempfile::tempdir().expect("temp dir should exist");
        let mut service = build_service_with_install_dir(temp_dir.path().to_path_buf());
        service.github_mirror_base_url = Some("https://ghproxy.net".to_string());

        assert_eq!(
            service.mirrored_github_archive_url("owner", "repo", "dev"),
            "https://ghproxy.net/https://github.com/owner/repo/archive/refs/heads/dev.zip"
        );
    }

    #[test]
    fn test_normalize_github_mirror_base_url_requires_http_url() {
        assert_eq!(
            SkillService::normalize_github_mirror_base_url(" https://ghproxy.net/ ").as_deref(),
            Some("https://ghproxy.net")
        );
        assert!(SkillService::normalize_github_mirror_base_url("ghproxy.net").is_none());
        assert!(SkillService::normalize_github_mirror_base_url("file:///tmp/mirror").is_none());
    }

    #[test]
    #[serial]
    fn test_install_dirs_are_app_specific_without_nested_skills() {
        let temp_dir = tempfile::tempdir().expect("temp dir should exist");
        let home = temp_dir.path().to_string_lossy().to_string();
        let _home_guard = EnvGuard::set("HOME", &home);
        let _user_profile_guard = EnvGuard::set("USERPROFILE", &home);

        let claude = SkillService::get_install_dir_for_app(&AppType::Claude)
            .expect("claude install dir should resolve");
        let codex = SkillService::get_install_dir_for_app(&AppType::Codex)
            .expect("codex install dir should resolve");
        let opencode = SkillService::get_install_dir_for_app(&AppType::Opencode)
            .expect("opencode install dir should resolve");

        assert_eq!(claude, temp_dir.path().join(".claude").join("skills"));
        assert_eq!(codex, temp_dir.path().join(".codex").join("skills"));
        assert_eq!(
            opencode,
            temp_dir
                .path()
                .join(".config")
                .join("opencode")
                .join("skills")
        );
        assert!(!claude.ends_with("skills/skills"));
        assert!(!codex.ends_with("skills/skills"));
    }

    #[test]
    #[serial]
    fn skill_backup_roundtrip_restores_uninstalled_skill() {
        let temp_dir = tempfile::tempdir().expect("temp dir should exist");
        let home = temp_dir.path().to_string_lossy().to_string();
        let _home_guard = EnvGuard::set("HOME", &home);
        let _user_profile_guard = EnvGuard::set("USERPROFILE", &home);
        let install_dir = temp_dir.path().join(".claude").join("skills");
        let service = build_service_with_install_dir(install_dir.clone());
        let skill_dir = install_dir.join("demo-skill");
        fs::create_dir_all(&skill_dir).expect("create installed skill");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: Demo Skill\ndescription: Backup me\n---\n",
        )
        .expect("write skill");

        let backup = service
            .backup_skill_before_uninstall("demo-skill")
            .expect("backup should succeed")
            .expect("backup should be created");
        service
            .uninstall_skill("demo-skill".to_string())
            .expect("uninstall should succeed");
        assert!(!skill_dir.exists());

        let restored = service
            .restore_backup(&backup.backup_id, false)
            .expect("restore should succeed");

        assert_eq!(restored.directory, "demo-skill");
        assert!(skill_dir.join("SKILL.md").is_file());
        assert_eq!(SkillService::list_backups().expect("list backups").len(), 1);
    }

    #[test]
    fn test_normalize_default_repos_migrates_anthropics_skills_path() {
        let mut store = SkillStore {
            skills: HashMap::new(),
            repos: vec![SkillRepo {
                owner: "anthropics".to_string(),
                name: "skills".to_string(),
                branch: "main".to_string(),
                enabled: true,
                skills_path: None,
            }],
            repo_cache: HashMap::new(),
        };

        assert!(SkillService::normalize_default_repos(&mut store));
        assert_eq!(store.repos[0].skills_path.as_deref(), Some("skills"));
    }

    #[test]
    fn test_normalize_default_repos_migrates_anthropics_skills_path_case_insensitive() {
        let mut store = SkillStore {
            skills: HashMap::new(),
            repos: vec![SkillRepo {
                owner: "Anthropics".to_string(),
                name: "Skills".to_string(),
                branch: "main".to_string(),
                enabled: true,
                skills_path: None,
            }],
            repo_cache: HashMap::new(),
        };

        assert!(SkillService::normalize_default_repos(&mut store));
        assert_eq!(store.repos[0].skills_path.as_deref(), Some("skills"));
    }

    #[test]
    fn test_normalize_skills_path_rejects_traversal() {
        let normalized = SkillService::normalize_skills_path("../skills");
        assert!(normalized.is_err());
    }

    #[test]
    fn test_validate_skill_directory_accepts_relative() {
        assert!(SkillService::validate_skill_directory("skills/subdir").is_ok());
        assert!(SkillService::validate_skill_directory("./skills/subdir").is_ok());
    }

    #[test]
    fn test_validate_skill_directory_rejects_traversal_or_absolute() {
        assert!(SkillService::validate_skill_directory("../skills").is_err());
        assert!(SkillService::validate_skill_directory("skills/../escape").is_err());
        assert!(SkillService::validate_skill_directory("..\\escape").is_err());
        assert!(SkillService::validate_skill_directory("/absolute").is_err());
        assert!(SkillService::validate_skill_directory("").is_err());
    }

    #[test]
    fn test_parse_skill_metadata() {
        let temp_dir = tempfile::tempdir().expect("temp dir should exist");
        let skill_md = temp_dir.path().join("SKILL.md");
        let content = r#"---
name: Demo Skill
description: Useful skill
---
# body
"#;
        fs::write(&skill_md, content).expect("should write skill metadata");
        let service = build_service_with_install_dir(temp_dir.path().to_path_buf());

        let metadata = service
            .parse_skill_metadata(&skill_md)
            .expect("metadata should parse");

        assert_eq!(metadata.name.as_deref(), Some("Demo Skill"));
        assert_eq!(metadata.description.as_deref(), Some("Useful skill"));
    }

    #[test]
    fn test_deduplicate_skills() {
        let mut skills = vec![
            make_skill("owner/name:skill", "SkillOne"),
            make_skill("Owner/Name:Skill", "SkillTwo"),
            make_skill("local:unique", "Unique"),
        ];

        SkillService::deduplicate_skills(&mut skills);

        assert_eq!(skills.len(), 2);
        assert!(skills.iter().any(|s| s.key == "owner/name:skill"));
        assert!(skills.iter().any(|s| s.key == "local:unique"));
    }

    #[test]
    fn test_resolve_install_target_conflict_same_directory() {
        let mut first = make_skill("owner1/repo1:alpha", "alpha");
        first.repo_owner = Some("owner1".to_string());
        first.repo_name = Some("repo1".to_string());
        first.repo_branch = Some("main".to_string());
        let mut second = make_skill("owner2/repo2:alpha", "alpha");
        second.repo_owner = Some("owner2".to_string());
        second.repo_name = Some("repo2".to_string());
        second.repo_branch = Some("dev".to_string());

        let err = SkillService::resolve_install_target(&[first, second], "alpha")
            .expect_err("should reject install path conflicts");
        let parsed: Value = serde_json::from_str(&err).expect("should parse conflict error json");
        assert_eq!(parsed["code"], "SKILL_INSTALL_PATH_CONFLICT");
        assert_eq!(parsed["context"]["directory"], "alpha");
        let sources = parsed["context"]["sources"].as_str().unwrap_or("");
        assert!(sources.contains("owner1/repo1@main"));
        assert!(sources.contains("owner2/repo2@dev"));
    }

    #[tokio::test]
    #[serial]
    async fn test_install_skill_skips_when_installed_without_force() {
        let temp_dir = tempfile::tempdir().expect("temp dir should exist");
        let home = temp_dir.path().to_string_lossy().to_string();
        let _home_guard = EnvGuard::set("HOME", &home);
        let _user_profile_guard = EnvGuard::set("USERPROFILE", &home);
        let install_dir = temp_dir.path().join("install");
        fs::create_dir_all(&install_dir).expect("install dir should exist");
        let service = build_service_with_install_dir(install_dir.clone());

        let dest = install_dir.join("demo");
        fs::create_dir_all(&dest).expect("dest should exist");
        fs::write(dest.join("SKILL.md"), "old").expect("write existing skill");

        let repo = SkillRepo {
            owner: "owner".to_string(),
            name: "repo".to_string(),
            branch: "main".to_string(),
            enabled: true,
            skills_path: None,
        };

        service
            .install_skill("demo".to_string(), repo, false)
            .await
            .expect("install should skip when already installed");

        let content = fs::read_to_string(dest.join("SKILL.md")).expect("read existing skill");
        assert_eq!(content, "old");
    }

    #[test]
    fn test_install_from_source_respects_force() {
        let temp_dir = tempfile::tempdir().expect("temp dir should exist");
        let source = temp_dir.path().join("source");
        let dest = temp_dir.path().join("dest");
        fs::create_dir_all(&source).expect("source dir should exist");
        fs::create_dir_all(&dest).expect("dest dir should exist");
        fs::write(source.join("SKILL.md"), "new").expect("write source skill");
        fs::write(dest.join("SKILL.md"), "old").expect("write dest skill");

        let skipped = SkillService::install_from_source(&source, &dest, false)
            .expect("install from source should succeed");
        assert!(!skipped, "expected install to be skipped without force");
        let content = fs::read_to_string(dest.join("SKILL.md")).expect("read dest skill");
        assert_eq!(content, "old");

        let installed = SkillService::install_from_source(&source, &dest, true)
            .expect("force install should succeed");
        assert!(installed, "expected install to proceed with force");
        let content = fs::read_to_string(dest.join("SKILL.md")).expect("read dest skill");
        assert_eq!(content, "new");
    }

    #[test]
    fn test_resolve_install_source_path_skills_path_edges() {
        let temp_dir = tempfile::tempdir().expect("temp dir should exist");

        let source =
            SkillService::resolve_install_source_path(temp_dir.path(), "foo", Some("skills/foo"))
                .expect("should resolve source for leaf match");
        assert_eq!(source, temp_dir.path().join("skills").join("foo"));

        let source =
            SkillService::resolve_install_source_path(temp_dir.path(), "foo", Some("skills"))
                .expect("should resolve source with skills path");
        assert_eq!(source, temp_dir.path().join("skills").join("foo"));

        let source = SkillService::resolve_install_source_path(temp_dir.path(), "foo", Some(" / "))
            .expect("should resolve source for empty skills path");
        assert_eq!(source, temp_dir.path().join("foo"));

        let err =
            SkillService::resolve_install_source_path(temp_dir.path(), "foo", Some("../skills"))
                .expect_err("should reject traversal skills path");
        let parsed: Value =
            serde_json::from_str(&err.to_string()).expect("should parse error json");
        assert_eq!(parsed["code"], "SKILL_PATH_INVALID");
    }

    #[test]
    fn state_source_matching_rejects_same_directory_from_other_repo() {
        let states = HashMap::from([(
            "claude:demo".to_string(),
            SkillState {
                installed: true,
                installed_at: Utc::now(),
                repo_owner: Some("owner-a".to_string()),
                repo_name: Some("repo-a".to_string()),
                repo_branch: Some("main".to_string()),
                skills_path: None,
            },
        )]);
        let matching = SkillRepo {
            owner: "owner-a".to_string(),
            name: "repo-a".to_string(),
            branch: "main".to_string(),
            enabled: true,
            skills_path: None,
        };
        let other = SkillRepo {
            owner: "owner-b".to_string(),
            name: "repo-b".to_string(),
            branch: "main".to_string(),
            enabled: true,
            skills_path: None,
        };

        assert!(SkillService::state_matches_source(
            &states, "demo", &matching
        ));
        assert!(!SkillService::state_matches_source(&states, "demo", &other));
    }

    #[test]
    fn compute_dir_hash_ignores_hidden_files_and_tracks_content() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(dir.path().join("SKILL.md"), "one").expect("write skill");
        let first = SkillService::compute_dir_hash(dir.path()).expect("hash directory");
        fs::write(dir.path().join(".local"), "ignored").expect("write hidden file");
        let hidden = SkillService::compute_dir_hash(dir.path()).expect("hash directory");
        assert_eq!(first, hidden);

        fs::write(dir.path().join("SKILL.md"), "two").expect("update skill");
        let changed = SkillService::compute_dir_hash(dir.path()).expect("hash directory");
        assert_ne!(first, changed);
    }

    #[test]
    fn test_anthropics_skills_source_installs_without_nested_skills_directory() {
        let temp_dir = tempfile::tempdir().expect("temp dir should exist");
        let repo_root = temp_dir.path().join("repo");
        let source = repo_root.join("skills").join("demo-skill");
        let install_dir = temp_dir.path().join("install");
        let dest = install_dir.join("demo-skill");
        fs::create_dir_all(&source).expect("source should exist");
        fs::write(source.join("SKILL.md"), "---\nname: Demo\n---\n").expect("write skill");

        let resolved =
            SkillService::resolve_install_source_path(&repo_root, "demo-skill", Some("skills"))
                .expect("source path should resolve");
        assert_eq!(resolved, source);

        SkillService::install_from_source(&resolved, &dest, false).expect("install should succeed");
        assert!(install_dir.join("demo-skill").join("SKILL.md").is_file());
        assert!(!install_dir.join("skills").join("demo-skill").exists());
    }

    #[test]
    fn test_scan_root_skill_md() {
        let temp_dir = tempfile::tempdir().expect("temp dir should exist");
        let skill_dir = temp_dir.path().join("skills").join("foo");
        fs::create_dir_all(&skill_dir).expect("should create skill dir");
        let skill_md = skill_dir.join("SKILL.md");
        let content = r#"---
name: Root Skill
description: Root level skill
---
"#;
        fs::write(&skill_md, content).expect("should write skill metadata");
        let service = build_service_with_install_dir(temp_dir.path().to_path_buf());
        let repo = SkillRepo {
            owner: "owner".to_string(),
            name: "repo".to_string(),
            branch: "main".to_string(),
            enabled: true,
            skills_path: Some("skills/foo".to_string()),
        };
        let mut skills = Vec::new();

        service
            .scan_skills_recursive(
                &skill_dir,
                &skill_dir,
                &repo,
                Some("skills/foo"),
                &mut skills,
            )
            .expect("scan should succeed");

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].directory, "foo");
        let readme_url = skills[0]
            .readme_url
            .as_deref()
            .expect("readme url should exist");
        assert!(readme_url.contains("/skills/foo"));
    }

    #[test]
    fn test_extract_zip_without_common_root() {
        let mut buffer = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut buffer);
            let mut zip_writer = zip::ZipWriter::new(cursor);
            let options: FileOptions<'_, ()> = FileOptions::default();
            zip_writer
                .start_file("skills/SKILL.md", options)
                .expect("start skill file");
            zip_writer
                .write_all(b"---\nname: Skill\n---\n")
                .expect("write skill file");
            zip_writer
                .start_file("README.md", options)
                .expect("start readme file");
            zip_writer.write_all(b"readme").expect("write readme file");
            zip_writer.finish().expect("finish zip");
        }

        let dest_dir = tempfile::tempdir().expect("temp dir should exist");
        SkillService::extract_zip_to_dir(
            buffer,
            dest_dir.path().to_path_buf(),
            SkillService::zip_limits(),
        )
        .expect("extract should succeed");

        assert!(dest_dir.path().join("skills/SKILL.md").is_file());
        assert!(dest_dir.path().join("README.md").is_file());
    }

    #[test]
    #[serial]
    fn install_from_zip_bytes_installs_root_skill() {
        let temp_dir = tempfile::tempdir().expect("temp dir should exist");
        let home = temp_dir.path().to_string_lossy().to_string();
        let _home_guard = EnvGuard::set("HOME", &home);
        let _user_profile_guard = EnvGuard::set("USERPROFILE", &home);
        let mut buffer = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut buffer);
            let mut zip_writer = zip::ZipWriter::new(cursor);
            let options: FileOptions<'_, ()> = FileOptions::default();
            zip_writer
                .start_file("SKILL.md", options)
                .expect("start skill file");
            zip_writer
                .write_all(b"---\nname: Imported Skill\ndescription: From zip\n---\n")
                .expect("write skill file");
            zip_writer.finish().expect("finish zip");
        }

        let install_dir = temp_dir.path().join("install");
        let service = build_service_with_install_dir(install_dir.clone());

        let installed = service
            .install_from_zip_bytes(buffer, Some("imported.skill"), false)
            .expect("install from zip should succeed");

        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].directory, "Imported-Skill");
        assert!(install_dir
            .join("Imported-Skill")
            .join("SKILL.md")
            .is_file());
    }
}

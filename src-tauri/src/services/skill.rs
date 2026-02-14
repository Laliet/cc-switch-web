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

use crate::app_config::AppType;
use crate::config::{get_app_config_dir, get_home_dir, write_json_file};
use crate::error::format_skill_error;

const MAX_SKILL_SCAN_DEPTH: usize = 32;
const DEFAULT_SKILL_CACHE_TTL_SECS: u64 = 0;
const DEFAULT_MAX_ZIP_BYTES: u64 = 50 * 1024 * 1024;
const DEFAULT_MAX_ZIP_ENTRIES: usize = 20_000;
const DEFAULT_MAX_TOTAL_UNCOMPRESSED_BYTES: u64 = 500 * 1024 * 1024;
const DEFAULT_MAX_SINGLE_FILE_BYTES: u64 = 50 * 1024 * 1024;
const DEFAULT_MAX_COMPRESSION_RATIO: u64 = 200;
const DEFAULT_MAX_PATH_COMPONENTS: usize = 64;
const DEFAULT_MAX_PATH_LENGTH: usize = 240;

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
                    skills_path: None, // 扫描根目录
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
        let install_dir = Self::get_install_dir_for_app(app)?;

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
        })
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
            AppType::Opencode | AppType::Omo => {
                return Err(anyhow!(format_skill_error(
                    "APP_NOT_SUPPORTED",
                    &[("app", app.as_str())],
                    None,
                )))
            }
        };
        Ok(home.join(dir).join("skills"))
    }

    pub fn state_key(app: &AppType, directory: &str) -> String {
        format!("{}:{directory}", app.as_str())
    }

    fn installed_apps_for_directory(directory: &str) -> Vec<String> {
        [AppType::Claude, AppType::Codex, AppType::Gemini]
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
        skills.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

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
            let url = format!(
                "https://github.com/{}/{}/archive/refs/heads/{}.zip",
                repo.owner, repo.name, branch
            );

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

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("所有分支下载失败")))
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

        // 若目标目录已存在，则视为已安装，避免重复下载
        if dest.exists() && !force {
            return Ok(());
        }

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
        let source =
            Self::resolve_install_source_path(&temp_path, &directory, repo.skills_path.as_deref())?;

        if !source.exists() {
            return Err(anyhow::anyhow!(format_skill_error(
                "SKILL_DIR_NOT_FOUND",
                &[("path", &source.display().to_string())],
                Some("checkRepoUrl"),
            )));
        }

        Self::install_from_source(&source, &dest, force)?;

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

            if path.is_dir() {
                Self::copy_dir_recursive(&path, &dest_path)?;
            } else {
                fs::copy(&path, &dest_path)?;
            }
        }

        Ok(())
    }

    /// 卸载技能（仅负责文件操作，状态更新由上层负责）
    pub fn uninstall_skill(&self, directory: String) -> Result<()> {
        Self::validate_skill_directory(&directory)?;
        let dest = self.install_dir.join(&directory);

        if dest.exists() {
            fs::remove_dir_all(&dest)?;
        }

        Ok(())
    }

    /// 列出仓库
    pub fn list_repos(&self, store: &SkillStore) -> Vec<SkillRepo> {
        store.repos.clone()
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::io::Write;
    use zip::write::FileOptions;

    fn build_service_with_install_dir(dir: PathBuf) -> SkillService {
        SkillService {
            http_client: Client::builder()
                .user_agent("cc-switch-test")
                .build()
                .expect("client build should succeed"),
            install_dir: dir,
            app: AppType::Claude,
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
    async fn test_install_skill_skips_when_installed_without_force() {
        let temp_dir = tempfile::tempdir().expect("temp dir should exist");
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
}

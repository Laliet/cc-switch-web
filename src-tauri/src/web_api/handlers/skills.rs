#![cfg(feature = "web-server")]

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use base64::{engine::general_purpose, Engine};
use chrono::Utc;
use serde::Serialize;

use crate::{
    app_config::AppType,
    error::format_skill_error,
    error::AppError,
    services::{
        skill::SkillCommand as ServiceSkillCommand, ImportInstalledSkillSelection,
        InstalledSkillDiscovery, InstalledSkillImportResult, InstalledSkillImportStatus,
        MigrationResult, Skill as ServiceSkill, SkillBackupEntry, SkillRepo, SkillService,
        SkillStorageLocation, SkillUpdateInfo, SkillsShSearchResult,
    },
    store::AppState,
};

use super::{ensure_app_feature, parse_known_app_type, ApiError, ApiResult};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsResponse {
    pub skills: Vec<SkillResponse>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    pub cache_hit: bool,
    pub refreshing: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillUninstallResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup: Option<SkillBackupEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillCommand {
    pub name: String,
    pub description: String,
    pub file_path: String,
}

impl From<ServiceSkillCommand> for SkillCommand {
    fn from(command: ServiceSkillCommand) -> Self {
        Self {
            name: command.name,
            description: command.description,
            file_path: command.file_path,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillResponse {
    pub key: String,
    pub name: String,
    pub description: String,
    pub directory: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_path: Option<String>,
    pub depth: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readme_url: Option<String>,
    pub installed: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub installed_apps: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills_path: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<SkillCommand>,
}

impl From<ServiceSkill> for SkillResponse {
    fn from(skill: ServiceSkill) -> Self {
        Self {
            key: skill.key,
            name: skill.name,
            description: skill.description,
            directory: skill.directory,
            parent_path: skill.parent_path,
            depth: skill.depth,
            readme_url: skill.readme_url,
            installed: skill.installed,
            installed_apps: skill.installed_apps,
            repo_owner: skill.repo_owner,
            repo_name: skill.repo_name,
            repo_branch: skill.repo_branch,
            skills_path: skill.skills_path,
            commands: skill.commands.into_iter().map(SkillCommand::from).collect(),
        }
    }
}

pub async fn install_skill(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<InstallPayload>,
) -> ApiResult<bool> {
    let InstallPayload {
        directory,
        force,
        app,
    } = payload;
    let force = force.unwrap_or(false);
    let app = parse_skill_app(app)?;
    let service = SkillService::new_for_app(&app).map_err(internal_error)?;

    // 收集仓库信息并查找目标技能
    let (repos, mut repo_cache) = {
        let cfg = state.load_config().map_err(ApiError::from)?;
        (cfg.skills.repos.clone(), cfg.skills.repo_cache.clone())
    };
    let skills = service
        .list_skills(repos, &mut repo_cache)
        .await
        .map_err(internal_error)?;
    let skill = SkillService::resolve_install_target(&skills.skills, &directory)
        .map_err(ApiError::bad_request)?;

    if !skill.installed || force {
        let repo = SkillRepo {
            owner: skill.repo_owner.clone().ok_or_else(|| {
                ApiError::bad_request(format_skill_error(
                    "MISSING_REPO_INFO",
                    &[("directory", directory.as_str()), ("field", "owner")],
                    None,
                ))
            })?,
            name: skill.repo_name.clone().ok_or_else(|| {
                ApiError::bad_request(format_skill_error(
                    "MISSING_REPO_INFO",
                    &[("directory", directory.as_str()), ("field", "name")],
                    None,
                ))
            })?,
            branch: skill
                .repo_branch
                .clone()
                .unwrap_or_else(|| "main".to_string()),
            enabled: true,
            skills_path: skill.skills_path.clone(),
        };

        service
            .install_skill(directory.clone(), repo, force)
            .await
            .map_err(internal_error)?;
    }

    // 写入状态
    state
        .update_config(|cfg| {
            cfg.skills.repo_cache = repo_cache;
            cfg.skills.skills.insert(
                SkillService::state_key(&app, &directory),
                crate::services::skill::SkillState {
                    installed: true,
                    installed_at: Utc::now(),
                    repo_owner: skill.repo_owner.clone(),
                    repo_name: skill.repo_name.clone(),
                    repo_branch: skill.repo_branch.clone(),
                    skills_path: skill.skills_path.clone(),
                },
            );
            Ok(())
        })
        .map_err(internal_error)?;

    Ok(Json(true))
}

pub async fn uninstall_skill(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<InstallPayload>,
) -> ApiResult<SkillUninstallResult> {
    let app = parse_skill_app(payload.app.clone())?;
    SkillService::validate_skill_directory(&payload.directory)
        .map_err(|err| ApiError::bad_request(err.to_string()))?;
    let service = SkillService::new_for_app(&app).map_err(internal_error)?;
    let backup = service
        .backup_skill_before_uninstall(&payload.directory)
        .map_err(internal_error)?;
    service
        .uninstall_skill(payload.directory.clone())
        .map_err(internal_error)?;

    state
        .update_config(|cfg| {
            cfg.skills
                .skills
                .remove(&SkillService::state_key(&app, &payload.directory));
            Ok(())
        })
        .map_err(internal_error)?;

    Ok(Json(SkillUninstallResult {
        success: true,
        backup,
    }))
}

pub async fn list_backups() -> ApiResult<Vec<SkillBackupEntry>> {
    let backups = SkillService::list_backups().map_err(internal_error)?;
    Ok(Json(backups))
}

pub async fn scan_unmanaged_skills(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Vec<InstalledSkillDiscovery>> {
    let config = state.load_config().map_err(ApiError::from)?;
    let service = SkillService::new().map_err(internal_error)?;
    let skill_states = config.skills.skills;
    let discoveries =
        tokio::task::spawn_blocking(move || service.discover_installed_skills(&skill_states))
            .await
            .map_err(internal_error)?
            .map_err(internal_error)?;
    Ok(Json(discoveries))
}

pub async fn import_skills_from_apps(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ImportInstalledSkillsPayload>,
) -> ApiResult<Vec<InstalledSkillImportResult>> {
    let states = state.load_config().map_err(ApiError::from)?.skills.skills;
    let service = SkillService::new().map_err(internal_error)?;
    let imports = payload.imports;
    let import_states = states;
    let results = tokio::task::spawn_blocking(move || {
        service.import_installed_skills(&import_states, imports)
    })
    .await
    .map_err(internal_error)?
    .map_err(internal_error)?;

    let mut state_updates = Vec::new();
    for result in &results {
        if !matches!(
            result.status,
            InstalledSkillImportStatus::Imported | InstalledSkillImportStatus::AlreadyManaged
        ) {
            continue;
        }
        for app in &result.enabled_apps {
            state_updates.push((
                parse_skill_app(Some(app.clone()))?,
                result.directory.clone(),
            ));
        }
    }

    let installed_at = Utc::now();
    state
        .update_config(|config| {
            for (app, directory) in &state_updates {
                let skill_state = config
                    .skills
                    .skills
                    .entry(SkillService::state_key(app, directory))
                    .or_insert_with(|| crate::services::skill::SkillState {
                        installed: true,
                        installed_at,
                        repo_owner: None,
                        repo_name: None,
                        repo_branch: None,
                        skills_path: None,
                    });
                skill_state.installed = true;
            }
            Ok(())
        })
        .map_err(internal_error)?;

    Ok(Json(results))
}

pub async fn delete_backup(Path(backup_id): Path<String>) -> ApiResult<bool> {
    SkillService::delete_backup(&backup_id).map_err(internal_error)?;
    Ok(Json(true))
}

pub async fn restore_backup(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RestoreBackupPayload>,
) -> ApiResult<SkillBackupEntry> {
    let app = parse_skill_app(payload.app.clone())?;
    let service = SkillService::new_for_app(&app).map_err(internal_error)?;
    let backup = service
        .restore_backup(&payload.backup_id, payload.force.unwrap_or(false))
        .map_err(internal_error)?;

    state
        .update_config(|cfg| {
            cfg.skills.skills.insert(
                SkillService::state_key(&app, &backup.directory),
                crate::services::skill::SkillState {
                    installed: true,
                    installed_at: Utc::now(),
                    repo_owner: None,
                    repo_name: None,
                    repo_branch: None,
                    skills_path: None,
                },
            );
            Ok(())
        })
        .map_err(internal_error)?;

    Ok(Json(backup))
}

pub async fn migrate_storage(
    Json(payload): Json<MigrateStoragePayload>,
) -> ApiResult<MigrationResult> {
    let result = SkillService::migrate_storage(payload.target).map_err(internal_error)?;
    Ok(Json(result))
}

pub async fn check_updates(State(state): State<Arc<AppState>>) -> ApiResult<Vec<SkillUpdateInfo>> {
    let config = state.load_config().map_err(ApiError::from)?;
    let service = SkillService::new().map_err(internal_error)?;
    let updates = service
        .check_updates(&config.skills.repos, &config.skills.skills)
        .await
        .map_err(internal_error)?;
    Ok(Json(updates))
}

pub async fn update_skill(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateSkillPayload>,
) -> ApiResult<SkillUpdateInfo> {
    let config = state.load_config().map_err(ApiError::from)?;
    let service = SkillService::new().map_err(internal_error)?;
    let updated = service
        .update_skill(&config.skills.repos, &config.skills.skills, &payload.id)
        .await
        .map_err(internal_error)?;
    Ok(Json(updated))
}

pub async fn search_skills_sh(
    Query(query): Query<SkillsShQuery>,
) -> ApiResult<SkillsShSearchResult> {
    let result = SkillService::search_skills_sh(
        &query.query,
        query.limit.unwrap_or(20),
        query.offset.unwrap_or(0),
    )
    .await
    .map_err(internal_error)?;
    Ok(Json(result))
}

pub async fn install_catalog_skill(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<InstallCatalogPayload>,
) -> ApiResult<bool> {
    let app = parse_skill_app(payload.app.clone())?;
    let repo = SkillRepo {
        owner: payload.repo_owner,
        name: payload.repo_name,
        branch: payload.repo_branch.unwrap_or_else(|| "main".to_string()),
        enabled: true,
        skills_path: None,
    };
    let service = SkillService::new_for_app(&app).map_err(internal_error)?;
    service
        .install_skill(
            payload.directory.clone(),
            repo.clone(),
            payload.force.unwrap_or(false),
        )
        .await
        .map_err(internal_error)?;
    state
        .update_config(|config| {
            if !config
                .skills
                .repos
                .iter()
                .any(|item| item.owner == repo.owner && item.name == repo.name)
            {
                config.skills.repos.push(repo.clone());
            }
            config.skills.skills.insert(
                SkillService::state_key(&app, &payload.directory),
                crate::services::skill::SkillState {
                    installed: true,
                    installed_at: Utc::now(),
                    repo_owner: Some(repo.owner.clone()),
                    repo_name: Some(repo.name.clone()),
                    repo_branch: Some(repo.branch.clone()),
                    skills_path: repo.skills_path.clone(),
                },
            );
            Ok(())
        })
        .map_err(internal_error)?;
    Ok(Json(true))
}

pub async fn install_from_zip(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<InstallZipPayload>,
) -> ApiResult<Vec<SkillResponse>> {
    let app = parse_skill_app(payload.app.clone())?;
    let service = SkillService::new_for_app(&app).map_err(internal_error)?;
    let force = payload.force.unwrap_or(false);

    let installed = match (payload.content_base64, payload.file_path) {
        (Some(content), _) => {
            let bytes = general_purpose::STANDARD
                .decode(content.trim().as_bytes())
                .map_err(|err| ApiError::bad_request(err.to_string()))?;
            service
                .install_from_zip_bytes(bytes, payload.file_name.as_deref(), force)
                .map_err(internal_error)?
        }
        (None, Some(path)) => service
            .install_from_zip_file(std::path::Path::new(&path), force)
            .map_err(internal_error)?,
        (None, None) => {
            return Err(ApiError::bad_request(format_skill_error(
                "ZIP_FILE_REQUIRED",
                &[],
                Some("selectFile"),
            )));
        }
    };

    state
        .update_config(|cfg| {
            for skill in &installed {
                cfg.skills.skills.insert(
                    SkillService::state_key(&app, &skill.directory),
                    crate::services::skill::SkillState {
                        installed: true,
                        installed_at: Utc::now(),
                        repo_owner: None,
                        repo_name: None,
                        repo_branch: None,
                        skills_path: None,
                    },
                );
            }
            Ok(())
        })
        .map_err(internal_error)?;

    Ok(Json(
        installed.into_iter().map(SkillResponse::from).collect(),
    ))
}

pub async fn list_repos(State(state): State<Arc<AppState>>) -> ApiResult<Vec<SkillRepo>> {
    let service = SkillService::new().map_err(internal_error)?;
    let repos = {
        let cfg = state.load_config().map_err(ApiError::from)?;
        service.list_repos(&cfg.skills)
    };
    Ok(Json(repos))
}

pub async fn add_repo(
    State(state): State<Arc<AppState>>,
    Json(repo): Json<SkillRepo>,
) -> ApiResult<bool> {
    let service = SkillService::new().map_err(internal_error)?;
    state
        .update_config(|cfg| {
            service
                .add_repo(&mut cfg.skills, repo)
                .map_err(|e| AppError::Config(e.to_string()))
        })
        .map_err(internal_error)?;
    Ok(Json(true))
}

pub async fn remove_repo(
    State(state): State<Arc<AppState>>,
    Path((owner, name)): Path<(String, String)>,
) -> ApiResult<bool> {
    let service = SkillService::new().map_err(internal_error)?;
    state
        .update_config(|cfg| {
            service
                .remove_repo(&mut cfg.skills, owner, name)
                .map_err(|e| AppError::Config(e.to_string()))
        })
        .map_err(internal_error)?;
    Ok(Json(true))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPayload {
    pub directory: String,
    #[serde(default)]
    pub force: Option<bool>,
    #[serde(default)]
    pub app: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportInstalledSkillsPayload {
    pub imports: Vec<ImportInstalledSkillSelection>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreBackupPayload {
    pub backup_id: String,
    #[serde(default)]
    pub force: Option<bool>,
    #[serde(default)]
    pub app: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallZipPayload {
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub content_base64: Option<String>,
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(default)]
    pub force: Option<bool>,
    #[serde(default)]
    pub app: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrateStoragePayload {
    pub target: SkillStorageLocation,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSkillPayload {
    pub id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallCatalogPayload {
    pub directory: String,
    pub repo_owner: String,
    pub repo_name: String,
    #[serde(default)]
    pub repo_branch: Option<String>,
    #[serde(default)]
    pub app: Option<String>,
    #[serde(default)]
    pub force: Option<bool>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsShQuery {
    #[serde(alias = "q")]
    pub query: String,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

fn internal_error(err: impl ToString) -> ApiError {
    ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSkillsQuery {
    pub app: Option<String>,
}

pub async fn list_skills(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListSkillsQuery>,
) -> ApiResult<SkillsResponse> {
    let app = parse_skill_app(query.app)?;
    let (repos, mut repo_cache) = {
        let cfg = state.load_config().map_err(ApiError::from)?;
        (cfg.skills.repos.clone(), cfg.skills.repo_cache.clone())
    };

    let service = SkillService::new_for_app(&app).map_err(internal_error)?;
    let result = service
        .list_skills(repos, &mut repo_cache)
        .await
        .map_err(internal_error)?;
    state
        .update_config(|cfg| {
            cfg.skills.repo_cache = repo_cache;
            Ok(())
        })
        .map_err(internal_error)?;
    let skills = result.skills.into_iter().map(SkillResponse::from).collect();
    Ok(Json(SkillsResponse {
        skills,
        warnings: result.warnings,
        cache_hit: result.cache_hit,
        refreshing: result.refreshing,
    }))
}

fn parse_skill_app(raw: Option<String>) -> Result<AppType, ApiError> {
    match raw {
        Some(value) => {
            let app = parse_known_app_type(&value)?;
            ensure_app_feature(&app, "skills")?;
            Ok(match app {
                AppType::Omo | AppType::OmoSlim => AppType::Opencode,
                other => other,
            })
        }
        None => Ok(AppType::Claude),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_skill_app;
    use crate::AppType;

    #[test]
    fn parse_skill_app_defaults_to_claude() {
        let app = parse_skill_app(None).expect("default app should parse");
        assert_eq!(app, AppType::Claude);
    }

    #[test]
    fn parse_skill_app_maps_omo_to_opencode() {
        let app = parse_skill_app(Some("omo".into())).expect("omo should reuse opencode skills");
        assert_eq!(app, AppType::Opencode);
    }
}

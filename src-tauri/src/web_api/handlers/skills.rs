#![cfg(feature = "web-server")]

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Serialize;

use crate::{
    app_config::AppType,
    error::format_skill_error,
    error::AppError,
    services::{
        skill::SkillCommand as ServiceSkillCommand, Skill as ServiceSkill, SkillRepo, SkillService,
    },
    store::AppState,
};

use super::{ApiError, ApiResult};

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
        let cfg = state
            .config
            .read()
            .map_err(AppError::from)
            .map_err(ApiError::from)?;
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
    {
        let mut cfg = state
            .config
            .write()
            .map_err(AppError::from)
            .map_err(ApiError::from)?;
        cfg.skills.repo_cache = repo_cache;
        cfg.skills.skills.insert(
            SkillService::state_key(&app, &directory),
            crate::services::skill::SkillState {
                installed: true,
                installed_at: Utc::now(),
            },
        );
    }
    state.save().map_err(internal_error)?;

    Ok(Json(true))
}

pub async fn uninstall_skill(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<InstallPayload>,
) -> ApiResult<bool> {
    let app = parse_skill_app(payload.app.clone())?;
    SkillService::validate_skill_directory(&payload.directory)
        .map_err(|err| ApiError::bad_request(err.to_string()))?;
    let service = SkillService::new_for_app(&app).map_err(internal_error)?;
    service
        .uninstall_skill(payload.directory.clone())
        .map_err(internal_error)?;

    {
        let mut cfg = state
            .config
            .write()
            .map_err(AppError::from)
            .map_err(ApiError::from)?;
        cfg.skills
            .skills
            .remove(&SkillService::state_key(&app, &payload.directory));
    }
    state.save().map_err(internal_error)?;

    Ok(Json(true))
}

pub async fn list_repos(State(state): State<Arc<AppState>>) -> ApiResult<Vec<SkillRepo>> {
    let service = SkillService::new().map_err(internal_error)?;
    let repos = {
        let cfg = state
            .config
            .read()
            .map_err(AppError::from)
            .map_err(ApiError::from)?;
        service.list_repos(&cfg.skills)
    };
    Ok(Json(repos))
}

pub async fn add_repo(
    State(state): State<Arc<AppState>>,
    Json(repo): Json<SkillRepo>,
) -> ApiResult<bool> {
    let service = SkillService::new().map_err(internal_error)?;
    {
        let mut cfg = state
            .config
            .write()
            .map_err(AppError::from)
            .map_err(ApiError::from)?;
        service
            .add_repo(&mut cfg.skills, repo)
            .map_err(internal_error)?;
    }
    state.save().map_err(internal_error)?;
    Ok(Json(true))
}

pub async fn remove_repo(
    State(state): State<Arc<AppState>>,
    Path((owner, name)): Path<(String, String)>,
) -> ApiResult<bool> {
    let service = SkillService::new().map_err(internal_error)?;
    {
        let mut cfg = state
            .config
            .write()
            .map_err(AppError::from)
            .map_err(ApiError::from)?;
        service
            .remove_repo(&mut cfg.skills, owner, name)
            .map_err(internal_error)?;
    }
    state.save().map_err(internal_error)?;
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
        let cfg = state
            .config
            .read()
            .map_err(AppError::from)
            .map_err(ApiError::from)?;
        (cfg.skills.repos.clone(), cfg.skills.repo_cache.clone())
    };

    let service = SkillService::new_for_app(&app).map_err(internal_error)?;
    let result = service
        .list_skills(repos, &mut repo_cache)
        .await
        .map_err(internal_error)?;
    {
        let mut cfg = state
            .config
            .write()
            .map_err(AppError::from)
            .map_err(ApiError::from)?;
        cfg.skills.repo_cache = repo_cache;
    }
    state.save().map_err(internal_error)?;
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
        Some(value) => AppType::parse_supported(&value)
            .map_err(|e: AppError| ApiError::bad_request(e.to_string())),
        None => Ok(AppType::Claude),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_skill_app;
    use crate::AppType;
    use axum::http::StatusCode;

    #[test]
    fn parse_skill_app_defaults_to_claude() {
        let app = parse_skill_app(None).expect("default app should parse");
        assert_eq!(app, AppType::Claude);
    }

    #[test]
    fn parse_skill_app_rejects_upcoming_app_ids() {
        let err = parse_skill_app(Some("omo".into()))
            .expect_err("upcoming app id should be rejected for now");
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
        assert!(
            err.message.contains("暂未支持") || err.message.contains("not supported yet"),
            "unexpected error message: {}",
            err.message
        );
    }
}

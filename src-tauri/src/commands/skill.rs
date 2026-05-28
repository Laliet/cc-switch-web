use crate::app_config::AppType;
use crate::error::{format_skill_error, AppError};
use crate::services::skill::SkillState;
use crate::services::{Skill, SkillRepo, SkillService};
use crate::store::AppState;
use chrono::Utc;
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

pub struct SkillServiceState(pub Arc<SkillService>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsResponse {
    pub skills: Vec<Skill>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    pub cache_hit: bool,
    pub refreshing: bool,
}

#[tauri::command]
pub async fn get_skills(
    app: Option<String>,
    _service: State<'_, SkillServiceState>,
    app_state: State<'_, AppState>,
) -> Result<SkillsResponse, String> {
    let app = parse_skill_app(app)?;
    let service_for_app = SkillService::new_for_app(&app).map_err(|e| e.to_string())?;

    let (repos, mut repo_cache) = {
        let config = app_state.load_config().map_err(|e| e.to_string())?;
        (
            config.skills.repos.clone(),
            config.skills.repo_cache.clone(),
        )
    };

    let result = service_for_app
        .list_skills(repos, &mut repo_cache)
        .await
        .map_err(|e| e.to_string())?;
    let skills = result.skills;
    let warnings = result.warnings;
    let cache_hit = result.cache_hit;
    let refreshing = result.refreshing;

    app_state
        .update_config(|config| {
            config.skills.repo_cache = repo_cache;
            Ok(())
        })
        .map_err(|e| e.to_string())?;

    Ok(SkillsResponse {
        skills,
        warnings,
        cache_hit,
        refreshing,
    })
}

#[tauri::command]
pub async fn install_skill(
    directory: String,
    force: Option<bool>,
    app: Option<String>,
    _service: State<'_, SkillServiceState>,
    app_state: State<'_, AppState>,
) -> Result<bool, String> {
    let force = force.unwrap_or(false);
    let app = parse_skill_app(app)?;
    let service_for_app = SkillService::new_for_app(&app).map_err(|e| e.to_string())?;

    // 先在不持有写锁的情况下收集仓库与技能信息
    let (repos, mut repo_cache) = {
        let config = app_state.load_config().map_err(|e| e.to_string())?;
        (
            config.skills.repos.clone(),
            config.skills.repo_cache.clone(),
        )
    };

    let skills = service_for_app
        .list_skills(repos, &mut repo_cache)
        .await
        .map_err(|e| e.to_string())?
        .skills;

    let skill =
        SkillService::resolve_install_target(&skills, &directory).map_err(|err| err.to_string())?;

    if !skill.installed || force {
        let repo = SkillRepo {
            owner: skill.repo_owner.clone().ok_or_else(|| {
                format_skill_error(
                    "MISSING_REPO_INFO",
                    &[("directory", &directory), ("field", "owner")],
                    None,
                )
            })?,
            name: skill.repo_name.clone().ok_or_else(|| {
                format_skill_error(
                    "MISSING_REPO_INFO",
                    &[("directory", &directory), ("field", "name")],
                    None,
                )
            })?,
            branch: skill
                .repo_branch
                .clone()
                .unwrap_or_else(|| "main".to_string()),
            enabled: true,
            skills_path: skill.skills_path.clone(), // 使用技能记录的 skills_path
        };

        service_for_app
            .install_skill(directory.clone(), repo, force)
            .await
            .map_err(|e| e.to_string())?;
    }

    app_state
        .update_config(|config| {
            config.skills.repo_cache = repo_cache;
            config.skills.skills.insert(
                SkillService::state_key(&app, &directory),
                SkillState {
                    installed: true,
                    installed_at: Utc::now(),
                },
            );
            Ok(())
        })
        .map_err(|e| e.to_string())?;

    Ok(true)
}

#[tauri::command]
pub fn uninstall_skill(
    directory: String,
    app: Option<String>,
    _service: State<'_, SkillServiceState>,
    app_state: State<'_, AppState>,
) -> Result<bool, String> {
    let app = parse_skill_app(app)?;
    let service_for_app = SkillService::new_for_app(&app).map_err(|e| e.to_string())?;

    service_for_app
        .uninstall_skill(directory.clone())
        .map_err(|e| e.to_string())?;

    app_state
        .update_config(|config| {
            config
                .skills
                .skills
                .remove(&SkillService::state_key(&app, &directory));
            Ok(())
        })
        .map_err(|e| e.to_string())?;

    Ok(true)
}

fn parse_skill_app(raw: Option<String>) -> Result<AppType, String> {
    match raw {
        Some(value) => AppType::parse_skills_app(&value).map_err(|e| e.to_string()),
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
    fn parse_skill_app_accepts_opencode() {
        let app = parse_skill_app(Some("opencode".into()))
            .expect("opencode should be supported for skills");
        assert_eq!(app, AppType::Opencode);
    }

    #[test]
    fn parse_skill_app_maps_omo_to_opencode() {
        let app = parse_skill_app(Some("omo".into())).expect("omo should reuse opencode skills");
        assert_eq!(app, AppType::Opencode);
    }
}

#[tauri::command]
pub fn get_skill_repos(
    _service: State<'_, SkillServiceState>,
    app_state: State<'_, AppState>,
) -> Result<Vec<SkillRepo>, String> {
    let config = app_state.load_config().map_err(|e| e.to_string())?;

    Ok(config.skills.repos.clone())
}

#[tauri::command]
pub fn add_skill_repo(
    repo: SkillRepo,
    service: State<'_, SkillServiceState>,
    app_state: State<'_, AppState>,
) -> Result<bool, String> {
    app_state
        .update_config(|config| {
            service
                .0
                .add_repo(&mut config.skills, repo)
                .map_err(|e| AppError::Config(e.to_string()))
        })
        .map_err(|e| e.to_string())?;

    Ok(true)
}

#[tauri::command]
pub fn remove_skill_repo(
    owner: String,
    name: String,
    service: State<'_, SkillServiceState>,
    app_state: State<'_, AppState>,
) -> Result<bool, String> {
    app_state
        .update_config(|config| {
            service
                .0
                .remove_repo(&mut config.skills, owner, name)
                .map_err(|e| AppError::Config(e.to_string()))
        })
        .map_err(|e| e.to_string())?;

    Ok(true)
}

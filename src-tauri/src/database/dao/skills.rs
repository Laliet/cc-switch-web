use super::super::{from_json_string, to_json_string, Database};
use crate::{
    error::AppError,
    services::skill::{SkillService, SkillStore},
};
use rusqlite::{params, Connection};
use std::collections::HashMap;

impl Database {
    pub(crate) fn load_skill_store(&self, conn: &Connection) -> Result<SkillStore, AppError> {
        let mut store = SkillStore {
            repos: Vec::new(),
            skills: HashMap::new(),
            repo_cache: HashMap::new(),
        };

        let mut repo_stmt = conn
            .prepare("SELECT owner, name, branch, enabled, skills_path FROM skill_repos")
            .map_err(|e| AppError::Database(e.to_string()))?;
        let repos = repo_stmt
            .query_map([], |row| {
                Ok(crate::services::skill::SkillRepo {
                    owner: row.get(0)?,
                    name: row.get(1)?,
                    branch: row.get(2)?,
                    enabled: row.get::<_, i64>(3)? != 0,
                    skills_path: {
                        let value: String = row.get(4)?;
                        if value.is_empty() {
                            None
                        } else {
                            Some(value)
                        }
                    },
                })
            })
            .map_err(|e| AppError::Database(e.to_string()))?;
        for repo in repos {
            store
                .repos
                .push(repo.map_err(|e| AppError::Database(e.to_string()))?);
        }
        if store.repos.is_empty() {
            store.repos = SkillStore::default().repos;
        }

        let mut state_stmt = conn
            .prepare(
                "SELECT state_key, installed, installed_at,
                        repo_owner, repo_name, repo_branch, skills_path
                 FROM skill_states",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let states = state_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            })
            .map_err(|e| AppError::Database(e.to_string()))?;
        for row in states {
            let (key, installed, installed_at, repo_owner, repo_name, repo_branch, skills_path) =
                row.map_err(|e| AppError::Database(e.to_string()))?;
            store.skills.insert(
                key,
                crate::services::skill::SkillState {
                    installed: installed != 0,
                    installed_at: chrono::DateTime::parse_from_rfc3339(&installed_at)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    repo_owner,
                    repo_name,
                    repo_branch,
                    skills_path,
                },
            );
        }

        let mut cache_stmt = conn
            .prepare("SELECT cache_key, value FROM skill_repo_cache")
            .map_err(|e| AppError::Database(e.to_string()))?;
        let caches = cache_stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| AppError::Database(e.to_string()))?;
        for row in caches {
            let (key, value) = row.map_err(|e| AppError::Database(e.to_string()))?;
            store
                .repo_cache
                .insert(key, from_json_string(&value, "skill repo cache")?);
        }

        if SkillService::normalize_default_repos(&mut store) {
            log::info!("Normalized default skill repos while loading SQLite store");
        }

        Ok(store)
    }

    pub(crate) fn save_skills_tx(
        tx: &rusqlite::Transaction<'_>,
        skills: &SkillStore,
    ) -> Result<(), AppError> {
        for repo in &skills.repos {
            tx.execute(
                "INSERT OR REPLACE INTO skill_repos
                 (owner, name, branch, enabled, skills_path)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    repo.owner,
                    repo.name,
                    repo.branch,
                    i64::from(repo.enabled),
                    repo.skills_path.as_deref().unwrap_or(""),
                ],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        }
        for (key, state) in &skills.skills {
            tx.execute(
                "INSERT OR REPLACE INTO skill_states
                 (state_key, installed, installed_at, repo_owner, repo_name, repo_branch, skills_path)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    key,
                    i64::from(state.installed),
                    state.installed_at.to_rfc3339(),
                    state.repo_owner,
                    state.repo_name,
                    state.repo_branch,
                    state.skills_path,
                ],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        }
        for (key, cache) in &skills.repo_cache {
            tx.execute(
                "INSERT OR REPLACE INTO skill_repo_cache (cache_key, value)
                 VALUES (?1, ?2)",
                params![key, to_json_string(cache)?],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        }
        Ok(())
    }
}

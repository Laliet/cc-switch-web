use super::super::Database;
use crate::{
    app_config::{MultiAppConfig, PromptConfig, PromptRoot},
    error::AppError,
    prompt::Prompt,
};
use rusqlite::{params, Connection};

impl Database {
    pub(crate) fn load_prompt_root(&self, conn: &Connection) -> Result<PromptRoot, AppError> {
        let mut root = PromptRoot::default();
        let mut stmt = conn
            .prepare(
                "SELECT id, app_type, name, content, description, enabled, created_at, updated_at
                 FROM prompts",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                ))
            })
            .map_err(|e| AppError::Database(e.to_string()))?;

        for row in rows {
            let (id, app_type, name, content, description, enabled, created_at, updated_at) =
                row.map_err(|e| AppError::Database(e.to_string()))?;
            let prompt = Prompt {
                id: id.clone(),
                name,
                content,
                description,
                enabled: enabled != 0,
                created_at,
                updated_at,
            };
            prompt_config_mut(&mut root, &app_type)?
                .prompts
                .insert(id, prompt);
        }
        Ok(root)
    }

    pub(crate) fn save_prompts_tx(
        tx: &rusqlite::Transaction<'_>,
        config: &MultiAppConfig,
    ) -> Result<(), AppError> {
        for (app_type, prompts) in [
            ("claude", &config.prompts.claude.prompts),
            ("codex", &config.prompts.codex.prompts),
            ("gemini", &config.prompts.gemini.prompts),
            ("opencode", &config.prompts.opencode.prompts),
        ] {
            for (id, prompt) in prompts {
                tx.execute(
                    "INSERT OR REPLACE INTO prompts (
                        id, app_type, name, content, description, enabled, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        id,
                        app_type,
                        prompt.name,
                        prompt.content,
                        prompt.description,
                        i64::from(prompt.enabled),
                        prompt.created_at,
                        prompt.updated_at,
                    ],
                )
                .map_err(|e| AppError::Database(e.to_string()))?;
            }
        }
        Ok(())
    }
}

fn prompt_config_mut<'a>(
    root: &'a mut PromptRoot,
    app_type: &str,
) -> Result<&'a mut PromptConfig, AppError> {
    match app_type {
        "claude" => Ok(&mut root.claude),
        "codex" => Ok(&mut root.codex),
        "gemini" => Ok(&mut root.gemini),
        "opencode" => Ok(&mut root.opencode),
        other => Err(AppError::Config(format!(
            "Unsupported prompt app in database: {other}"
        ))),
    }
}

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::app_config::AppType;
use crate::config::write_text_file;
use crate::error::AppError;
use crate::prompt::Prompt;
use crate::prompt_files::prompt_file_path;
use crate::store::AppState;

pub struct PromptService;

impl PromptService {
    pub fn get_prompts(
        state: &AppState,
        app: AppType,
    ) -> Result<HashMap<String, Prompt>, AppError> {
        let cfg = state.load_config()?;
        let prompts = match app {
            AppType::Claude => &cfg.prompts.claude.prompts,
            AppType::Codex => &cfg.prompts.codex.prompts,
            AppType::Gemini => &cfg.prompts.gemini.prompts,
            AppType::Opencode => &cfg.prompts.opencode.prompts,
            AppType::ClaudeDesktop | AppType::Omo | AppType::OmoSlim => {
                return Err(AppError::localized(
                    "app_not_supported_yet",
                    format!("应用 '{}' 暂未支持，敬请期待。", app.as_str()),
                    format!("App '{}' is not supported yet.", app.as_str()),
                ));
            }
        };
        Ok(prompts.clone())
    }

    pub fn upsert_prompt(
        state: &AppState,
        app: AppType,
        id: &str,
        prompt: Prompt,
    ) -> Result<(), AppError> {
        let (was_enabled, has_enabled) = state.update_config(|cfg| {
            let prompts = match app {
                AppType::Claude => &mut cfg.prompts.claude.prompts,
                AppType::Codex => &mut cfg.prompts.codex.prompts,
                AppType::Gemini => &mut cfg.prompts.gemini.prompts,
                AppType::Opencode => &mut cfg.prompts.opencode.prompts,
                AppType::ClaudeDesktop | AppType::Omo | AppType::OmoSlim => {
                    return Err(AppError::localized(
                        "app_not_supported_yet",
                        format!("应用 '{}' 暂未支持，敬请期待。", app.as_str()),
                        format!("App '{}' is not supported yet.", app.as_str()),
                    ));
                }
            };
            let was_enabled = prompts.get(id).map(|p| p.enabled).unwrap_or(false);
            prompts.insert(id.to_string(), prompt.clone());
            let has_enabled = prompts.values().any(|p| p.enabled);
            Ok((was_enabled, has_enabled))
        })?;

        // 如果是已启用的提示词，同步更新到对应的文件
        if prompt.enabled {
            let target_path = prompt_file_path(&app)?;
            write_text_file(&target_path, &prompt.content)?;
        } else if was_enabled && !has_enabled {
            let target_path = prompt_file_path(&app)?;
            write_text_file(&target_path, "")?;
        }

        Ok(())
    }

    pub fn delete_prompt(state: &AppState, app: AppType, id: &str) -> Result<(), AppError> {
        state.update_config(|cfg| {
            let prompts = match app {
                AppType::Claude => &mut cfg.prompts.claude.prompts,
                AppType::Codex => &mut cfg.prompts.codex.prompts,
                AppType::Gemini => &mut cfg.prompts.gemini.prompts,
                AppType::Opencode => &mut cfg.prompts.opencode.prompts,
                AppType::ClaudeDesktop | AppType::Omo | AppType::OmoSlim => {
                    return Err(AppError::localized(
                        "app_not_supported_yet",
                        format!("应用 '{}' 暂未支持，敬请期待。", app.as_str()),
                        format!("App '{}' is not supported yet.", app.as_str()),
                    ));
                }
            };

            if let Some(prompt) = prompts.get(id) {
                if prompt.enabled {
                    return Err(AppError::InvalidInput("无法删除已启用的提示词".to_string()));
                }
            }

            prompts.remove(id);
            Ok(())
        })?;
        Ok(())
    }

    pub fn enable_prompt(state: &AppState, app: AppType, id: &str) -> Result<(), AppError> {
        // 回填当前 live 文件内容到已启用的提示词，或创建备份
        let target_path = prompt_file_path(&app)?;
        if target_path.exists() {
            let live_content =
                std::fs::read_to_string(&target_path).map_err(|e| AppError::io(&target_path, e))?;
            if !live_content.is_empty() {
                state.update_config(|cfg| {
                    let prompts = match app {
                        AppType::Claude => &mut cfg.prompts.claude.prompts,
                        AppType::Codex => &mut cfg.prompts.codex.prompts,
                        AppType::Gemini => &mut cfg.prompts.gemini.prompts,
                        AppType::Opencode => &mut cfg.prompts.opencode.prompts,
                        AppType::ClaudeDesktop | AppType::Omo | AppType::OmoSlim => {
                            return Err(AppError::localized(
                                "app_not_supported_yet",
                                format!("应用 '{}' 暂未支持，敬请期待。", app.as_str()),
                                format!("App '{}' is not supported yet.", app.as_str()),
                            ));
                        }
                    };

                    if let Some((enabled_id, enabled_prompt)) = prompts
                        .iter_mut()
                        .find(|(_, p)| p.enabled)
                        .map(|(id, p)| (id.clone(), p))
                    {
                        let timestamp = Self::unix_timestamp()?;
                        enabled_prompt.content = live_content.clone();
                        enabled_prompt.updated_at = Some(timestamp);
                        log::info!("回填 live 提示词内容到已启用项: {enabled_id}");
                    } else {
                        let content_exists = prompts.values().any(|p| p.content == live_content);
                        if !content_exists {
                            let timestamp = Self::unix_timestamp()?;
                            let backup_id = format!("backup-{timestamp}");
                            let backup_prompt = Prompt {
                                id: backup_id.clone(),
                                name: format!(
                                    "原始提示词 {}",
                                    chrono::Local::now().format("%Y-%m-%d %H:%M")
                                ),
                                content: live_content,
                                description: Some("自动备份的原始提示词".to_string()),
                                enabled: false,
                                created_at: Some(timestamp),
                                updated_at: Some(timestamp),
                            };
                            prompts.insert(backup_id.clone(), backup_prompt);
                            log::info!("回填 live 提示词内容，创建备份: {backup_id}");
                        }
                    }
                    Ok(())
                })?;
            }
        }

        // 启用目标提示词并写入文件
        let content = state.update_config(|cfg| {
            let prompts = match app {
                AppType::Claude => &mut cfg.prompts.claude.prompts,
                AppType::Codex => &mut cfg.prompts.codex.prompts,
                AppType::Gemini => &mut cfg.prompts.gemini.prompts,
                AppType::Opencode => &mut cfg.prompts.opencode.prompts,
                AppType::ClaudeDesktop | AppType::Omo | AppType::OmoSlim => {
                    return Err(AppError::localized(
                        "app_not_supported_yet",
                        format!("应用 '{}' 暂未支持，敬请期待。", app.as_str()),
                        format!("App '{}' is not supported yet.", app.as_str()),
                    ));
                }
            };

            for prompt in prompts.values_mut() {
                prompt.enabled = false;
            }

            if let Some(prompt) = prompts.get_mut(id) {
                prompt.enabled = true;
                Ok(prompt.content.clone())
            } else {
                Err(AppError::InvalidInput(format!("提示词 {id} 不存在")))
            }
        })?;
        write_text_file(&target_path, &content)?;
        Ok(())
    }

    pub fn import_from_file(state: &AppState, app: AppType) -> Result<String, AppError> {
        let file_path = prompt_file_path(&app)?;

        if !file_path.exists() {
            return Err(AppError::Message("提示词文件不存在".to_string()));
        }

        let content =
            std::fs::read_to_string(&file_path).map_err(|e| AppError::io(&file_path, e))?;
        let timestamp = Self::unix_timestamp()?;

        let id = format!("imported-{timestamp}");
        let prompt = Prompt {
            id: id.clone(),
            name: format!(
                "导入的提示词 {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M")
            ),
            content,
            description: Some("从现有配置文件导入".to_string()),
            enabled: false,
            created_at: Some(timestamp),
            updated_at: Some(timestamp),
        };

        Self::upsert_prompt(state, app, &id, prompt)?;
        Ok(id)
    }

    pub fn get_current_file_content(app: AppType) -> Result<Option<String>, AppError> {
        let file_path = prompt_file_path(&app)?;
        if !file_path.exists() {
            return Ok(None);
        }
        let content =
            std::fs::read_to_string(&file_path).map_err(|e| AppError::io(&file_path, e))?;
        Ok(Some(content))
    }

    fn unix_timestamp() -> Result<i64, AppError> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .map_err(|err| AppError::Message(format!("获取系统时间戳失败: {err}")))
    }
}

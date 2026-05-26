use crate::app_config::MultiAppConfig;
use crate::database::Database;
use crate::error::AppError;
use std::sync::Arc;

/// 全局应用状态。
///
/// SQLite 是权威存储；服务层通过 `load_config` / `update_config`
/// 获取兼容的 `MultiAppConfig` 快照。`config.json` 只作为 legacy
/// import/export snapshot，不再承载运行时主状态。
pub struct AppState {
    pub db: Arc<Database>,
}

impl AppState {
    pub fn try_new() -> Result<Self, AppError> {
        Ok(Self {
            db: Arc::new(Database::init()?),
        })
    }

    pub fn new_for_tests(config: MultiAppConfig) -> Result<Self, AppError> {
        let db = Arc::new(Database::memory()?);
        db.replace_config(&config)?;
        Ok(Self { db })
    }

    pub fn load_config(&self) -> Result<MultiAppConfig, AppError> {
        self.db.load_config()
    }

    pub fn db_state(&self) -> Arc<AppState> {
        Arc::new(AppState {
            db: Arc::clone(&self.db),
        })
    }

    pub fn replace_config(&self, config: &MultiAppConfig) -> Result<(), AppError> {
        self.db.replace_config(config)
    }

    pub fn update_config<F, R>(&self, f: F) -> Result<R, AppError>
    where
        F: FnOnce(&mut MultiAppConfig) -> Result<R, AppError>,
    {
        self.db.update_config(f)
    }

    /// Compatibility helper for existing call sites. Runtime persistence is
    /// immediate on database writes, so this intentionally only verifies that a
    /// coherent snapshot can be loaded.
    pub fn save(&self) -> Result<(), AppError> {
        let _ = self.load_config()?;
        Ok(())
    }
}

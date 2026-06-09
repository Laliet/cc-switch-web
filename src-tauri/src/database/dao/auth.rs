use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use super::super::{lock_conn, to_json_string, token_crypto::TokenCipher, Database};
use crate::{
    auth::{
        ManagedAuthAccount, ManagedAuthAccountInput, ManagedAuthAccountSecret, ManagedAuthProvider,
        ManagedAuthTokenSet,
    },
    error::AppError,
};

impl Database {
    pub fn list_managed_auth_accounts(
        &self,
        provider: Option<ManagedAuthProvider>,
    ) -> Result<Vec<ManagedAuthAccount>, AppError> {
        let conn = lock_conn!(self.conn);
        list_accounts_with_conn(&conn, provider)
    }

    pub fn get_managed_auth_account(
        &self,
        provider: ManagedAuthProvider,
        account_id: &str,
    ) -> Result<Option<ManagedAuthAccountSecret>, AppError> {
        let conn = lock_conn!(self.conn);
        get_account_secret_with_conn(&conn, &self.token_cipher, provider, account_id)
    }

    pub fn get_default_managed_auth_account(
        &self,
        provider: ManagedAuthProvider,
    ) -> Result<Option<ManagedAuthAccountSecret>, AppError> {
        let conn = lock_conn!(self.conn);
        let account_id = conn
            .query_row(
                "SELECT id FROM managed_auth_accounts
                 WHERE provider = ?1
                   AND COALESCE(status, 'active') != 'logged_out'
                 ORDER BY is_default DESC, updated_at DESC
                 LIMIT 1",
                params![provider.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| AppError::Database(e.to_string()))?;

        account_id
            .map(|id| get_account_secret_with_conn(&conn, &self.token_cipher, provider, &id))
            .transpose()
            .map(|v| v.flatten())
    }

    pub fn upsert_managed_auth_account(
        &self,
        input: ManagedAuthAccountInput,
    ) -> Result<ManagedAuthAccount, AppError> {
        if input.tokens.access_token.trim().is_empty() {
            return Err(AppError::InvalidInput(
                "Managed account access token is required".to_string(),
            ));
        }
        if input.label.trim().is_empty() {
            return Err(AppError::InvalidInput(
                "Managed account label is required".to_string(),
            ));
        }

        let mut conn = lock_conn!(self.conn);
        let tx = conn
            .transaction()
            .map_err(|e| AppError::Database(e.to_string()))?;

        let now = Utc::now();
        let id = input
            .id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("{}-{}", input.provider.as_str(), now.timestamp_millis()));

        let existing_metadata = tx
            .query_row(
                "SELECT created_at, is_default, last_used_at FROM managed_auth_accounts
                 WHERE provider = ?1 AND id = ?2",
                params![input.provider.as_str(), id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)? != 0,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| AppError::Database(e.to_string()))?;
        let existing_is_default = existing_metadata
            .as_ref()
            .map(|(_, is_default, _)| *is_default)
            .unwrap_or(false);
        let existing_last_used_at = existing_metadata
            .as_ref()
            .and_then(|(_, _, last_used_at)| last_used_at.as_deref())
            .and_then(|raw| parse_datetime(raw).ok());
        let created_at = existing_metadata
            .as_ref()
            .and_then(|(raw, _, _)| parse_datetime(raw).ok())
            .unwrap_or(now);

        let should_make_default = input.make_default
            || tx
                .query_row(
                    "SELECT COUNT(*) FROM managed_auth_accounts WHERE provider = ?1",
                    params![input.provider.as_str()],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(|e| AppError::Database(e.to_string()))?
                == 0;
        let is_default = should_make_default || existing_is_default;

        if should_make_default {
            tx.execute(
                "UPDATE managed_auth_accounts SET is_default = 0 WHERE provider = ?1",
                params![input.provider.as_str()],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        }

        tx.execute(
            "INSERT OR REPLACE INTO managed_auth_accounts (
                id, provider, label, username, avatar_url, plan, is_default,
                created_at, updated_at, last_used_at, expires_at, scopes,
                token_type, access_token, refresh_token, status
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                id,
                input.provider.as_str(),
                input.label.trim(),
                input
                    .username
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty()),
                input
                    .avatar_url
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty()),
                input
                    .plan
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty()),
                i64::from(is_default),
                created_at.to_rfc3339(),
                now.to_rfc3339(),
                existing_last_used_at.map(|v| v.to_rfc3339()),
                input.tokens.expires_at.map(|v| v.to_rfc3339()),
                input.tokens.scope.as_deref(),
                input.tokens.token_type.as_deref(),
                self.token_cipher.encrypt(&input.tokens.access_token)?,
                input
                    .tokens
                    .refresh_token
                    .as_deref()
                    .map(|token| self.token_cipher.encrypt(token))
                    .transpose()?,
                Some("active"),
            ],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        tx.commit().map_err(|e| AppError::Database(e.to_string()))?;
        drop(conn);

        self.get_managed_auth_account(input.provider, &id)?
            .map(|secret| secret.account)
            .ok_or_else(|| AppError::Database("Managed auth account was not saved".to_string()))
    }

    pub fn set_default_managed_auth_account(
        &self,
        provider: ManagedAuthProvider,
        account_id: &str,
    ) -> Result<(), AppError> {
        let mut conn = lock_conn!(self.conn);
        let tx = conn
            .transaction()
            .map_err(|e| AppError::Database(e.to_string()))?;
        let status = tx
            .query_row(
                "SELECT status FROM managed_auth_accounts WHERE provider = ?1 AND id = ?2",
                params![provider.as_str(), account_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(|e| AppError::Database(e.to_string()))?;
        let Some(status) = status else {
            return Err(AppError::InvalidInput(format!(
                "Managed auth account '{account_id}' was not found"
            )));
        };
        if status
            .as_deref()
            .map(str::trim)
            .is_some_and(|status| status.eq_ignore_ascii_case("logged_out"))
        {
            return Err(AppError::InvalidInput(format!(
                "Managed auth account '{account_id}' is logged out"
            )));
        }
        tx.execute(
            "UPDATE managed_auth_accounts SET is_default = 0 WHERE provider = ?1",
            params![provider.as_str()],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        tx.execute(
            "UPDATE managed_auth_accounts
             SET is_default = 1, updated_at = ?3
             WHERE provider = ?1 AND id = ?2",
            params![provider.as_str(), account_id, Utc::now().to_rfc3339()],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        tx.commit().map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn delete_managed_auth_account(
        &self,
        provider: ManagedAuthProvider,
        account_id: &str,
    ) -> Result<bool, AppError> {
        let mut conn = lock_conn!(self.conn);
        let tx = conn
            .transaction()
            .map_err(|e| AppError::Database(e.to_string()))?;
        let was_default = tx
            .query_row(
                "SELECT is_default FROM managed_auth_accounts WHERE provider = ?1 AND id = ?2",
                params![provider.as_str(), account_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|e| AppError::Database(e.to_string()))?
            .unwrap_or(0)
            != 0;
        let deleted = tx
            .execute(
                "DELETE FROM managed_auth_accounts WHERE provider = ?1 AND id = ?2",
                params![provider.as_str(), account_id],
            )
            .map_err(|e| AppError::Database(e.to_string()))?
            > 0;
        if deleted && was_default {
            tx.execute(
                "UPDATE managed_auth_accounts
                 SET is_default = 1
                 WHERE rowid = (
                    SELECT rowid FROM managed_auth_accounts
                    WHERE provider = ?1
                      AND COALESCE(status, 'active') != 'logged_out'
                    ORDER BY updated_at DESC
                    LIMIT 1
                 )",
                params![provider.as_str()],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        }
        tx.commit().map_err(|e| AppError::Database(e.to_string()))?;
        Ok(deleted)
    }

    pub fn logout_managed_auth_account(
        &self,
        provider: ManagedAuthProvider,
        account_id: &str,
    ) -> Result<bool, AppError> {
        let mut conn = lock_conn!(self.conn);
        let tx = conn
            .transaction()
            .map_err(|e| AppError::Database(e.to_string()))?;
        let was_default = tx
            .query_row(
                "SELECT is_default FROM managed_auth_accounts WHERE provider = ?1 AND id = ?2",
                params![provider.as_str(), account_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|e| AppError::Database(e.to_string()))?;
        let Some(was_default) = was_default else {
            return Ok(false);
        };
        let encrypted_empty = self.token_cipher.encrypt("")?;
        let updated = tx
            .execute(
                "UPDATE managed_auth_accounts
                 SET access_token = ?3,
                     refresh_token = NULL,
                     is_default = 0,
                     status = 'logged_out',
                     updated_at = ?4
                 WHERE provider = ?1 AND id = ?2",
                params![
                    provider.as_str(),
                    account_id,
                    encrypted_empty,
                    Utc::now().to_rfc3339()
                ],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        if was_default != 0 {
            tx.execute(
                "UPDATE managed_auth_accounts
                 SET is_default = 1
                 WHERE rowid = (
                    SELECT rowid FROM managed_auth_accounts
                    WHERE provider = ?1
                      AND COALESCE(status, 'active') != 'logged_out'
                    ORDER BY updated_at DESC
                    LIMIT 1
                 )",
                params![provider.as_str()],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        }
        tx.commit().map_err(|e| AppError::Database(e.to_string()))?;
        Ok(updated > 0)
    }

    pub fn mark_managed_auth_account_used(
        &self,
        provider: ManagedAuthProvider,
        account_id: &str,
    ) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute(
            "UPDATE managed_auth_accounts
             SET last_used_at = ?3, updated_at = ?3
             WHERE provider = ?1 AND id = ?2",
            params![provider.as_str(), account_id, Utc::now().to_rfc3339()],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    #[cfg(any(test, feature = "test-hooks"))]
    pub fn get_raw_managed_auth_tokens_for_tests(
        &self,
        provider: ManagedAuthProvider,
        account_id: &str,
    ) -> Result<Option<(String, Option<String>)>, AppError> {
        let conn = lock_conn!(self.conn);
        conn.query_row(
            "SELECT access_token, refresh_token
             FROM managed_auth_accounts
             WHERE provider = ?1 AND id = ?2",
            params![provider.as_str(), account_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map_err(|e| AppError::Database(e.to_string()))
    }

    #[cfg(any(test, feature = "test-hooks"))]
    pub fn set_raw_managed_auth_tokens_for_tests(
        &self,
        provider: ManagedAuthProvider,
        account_id: &str,
        access_token: &str,
        refresh_token: Option<&str>,
    ) -> Result<bool, AppError> {
        let conn = lock_conn!(self.conn);
        let updated = conn
            .execute(
                "UPDATE managed_auth_accounts
                 SET access_token = ?3, refresh_token = ?4
                 WHERE provider = ?1 AND id = ?2",
                params![provider.as_str(), account_id, access_token, refresh_token],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(updated > 0)
    }
}

fn list_accounts_with_conn(
    conn: &Connection,
    provider: Option<ManagedAuthProvider>,
) -> Result<Vec<ManagedAuthAccount>, AppError> {
    let sql = if provider.is_some() {
        "SELECT id, provider, label, username, avatar_url, plan, is_default,
                created_at, updated_at, last_used_at, expires_at, scopes, status
         FROM managed_auth_accounts
         WHERE provider = ?1
         ORDER BY provider, is_default DESC, updated_at DESC"
    } else {
        "SELECT id, provider, label, username, avatar_url, plan, is_default,
                created_at, updated_at, last_used_at, expires_at, scopes, status
         FROM managed_auth_accounts
         ORDER BY provider, is_default DESC, updated_at DESC"
    };

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| AppError::Database(e.to_string()))?;
    let mut rows = if let Some(provider) = provider {
        stmt.query(params![provider.as_str()])
            .map_err(|e| AppError::Database(e.to_string()))?
    } else {
        stmt.query([])
            .map_err(|e| AppError::Database(e.to_string()))?
    };

    let mut accounts = Vec::new();
    while let Some(row) = rows.next().map_err(|e| AppError::Database(e.to_string()))? {
        accounts.push(ManagedAuthAccount {
            id: row.get(0).map_err(|e| AppError::Database(e.to_string()))?,
            provider: ManagedAuthProvider::parse(
                &row.get::<_, String>(1)
                    .map_err(|e| AppError::Database(e.to_string()))?,
            )?,
            label: row.get(2).map_err(|e| AppError::Database(e.to_string()))?,
            username: row.get(3).map_err(|e| AppError::Database(e.to_string()))?,
            avatar_url: row.get(4).map_err(|e| AppError::Database(e.to_string()))?,
            plan: row.get(5).map_err(|e| AppError::Database(e.to_string()))?,
            is_default: row
                .get::<_, i64>(6)
                .map_err(|e| AppError::Database(e.to_string()))?
                != 0,
            created_at: parse_datetime(
                &row.get::<_, String>(7)
                    .map_err(|e| AppError::Database(e.to_string()))?,
            )?,
            updated_at: parse_datetime(
                &row.get::<_, String>(8)
                    .map_err(|e| AppError::Database(e.to_string()))?,
            )?,
            last_used_at: parse_optional_datetime(row.get(9).ok().flatten())?,
            expires_at: parse_optional_datetime(row.get(10).ok().flatten())?,
            scopes: row.get(11).map_err(|e| AppError::Database(e.to_string()))?,
            status: row.get(12).map_err(|e| AppError::Database(e.to_string()))?,
        });
    }
    Ok(accounts)
}

fn get_account_secret_with_conn(
    conn: &Connection,
    token_cipher: &TokenCipher,
    provider: ManagedAuthProvider,
    account_id: &str,
) -> Result<Option<ManagedAuthAccountSecret>, AppError> {
    let accounts = list_accounts_with_conn(conn, Some(provider))?;
    let Some(account) = accounts.into_iter().find(|a| a.id == account_id) else {
        return Ok(None);
    };

    let (access_token, refresh_token, token_type) = conn
        .query_row(
            "SELECT access_token, refresh_token, token_type
             FROM managed_auth_accounts
             WHERE provider = ?1 AND id = ?2",
            params![provider.as_str(), account_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::Database("Managed auth account vanished".to_string()))?;
    let access_token = token_cipher.decrypt_legacy_ok(&access_token)?;
    let refresh_token = refresh_token
        .as_deref()
        .map(|token| token_cipher.decrypt_legacy_ok(token))
        .transpose()?;

    Ok(Some(ManagedAuthAccountSecret {
        tokens: ManagedAuthTokenSet {
            access_token,
            refresh_token,
            expires_at: account.expires_at,
            scope: account.scopes.clone(),
            token_type,
        },
        account,
    }))
}

fn parse_datetime(raw: &str) -> Result<DateTime<Utc>, AppError> {
    DateTime::parse_from_rfc3339(raw)
        .map(|v| v.with_timezone(&Utc))
        .map_err(|e| AppError::Config(format!("Invalid auth timestamp: {e}")))
}

fn parse_optional_datetime(raw: Option<String>) -> Result<Option<DateTime<Utc>>, AppError> {
    raw.map(|v| parse_datetime(&v)).transpose()
}

#[allow(dead_code)]
fn _assert_tokens_are_serializable(tokens: &ManagedAuthTokenSet) -> Result<String, AppError> {
    to_json_string(tokens)
}

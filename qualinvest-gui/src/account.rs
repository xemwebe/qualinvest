use cfg_if::cfg_if;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountOption {
    pub id: i32,
    pub broker: String,
    pub account_name: String,
    pub user_name: Option<String>, // Only for admin view
}

impl AccountOption {
    pub fn display_name(&self) -> String {
        if let Some(ref user) = self.user_name {
            format!("{} - {} ({})", self.broker, self.account_name, user)
        } else {
            format!("{} - {}", self.broker, self.account_name)
        }
    }
}

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use finql::postgres::PostgresDB;
        use qualinvest_core::{
            accounts::AccountHandler,
            user::UserHandler,
        };

        pub async fn get_accounts_for_user_ssr(user_id: i32, is_admin: bool, db: PostgresDB) -> Vec<AccountOption> {
            if is_admin {
                // Admin gets all accounts with user names
                let accounts = db.get_all_accounts().await;
                let mut account_options = Vec::new();

                for account in accounts {
                    if let Some(account_id) = account.id {
                        // Find which user owns this account
                        let user_name = get_account_owner_name(&db, account_id).await;
                        account_options.push(AccountOption {
                            id: account_id,
                            broker: account.broker,
                            account_name: account.account_name,
                            user_name,
                        });
                    }
                }
                account_options
            } else {
                // Regular user gets only their accounts
                if let Ok(accounts) = db.get_user_accounts(user_id).await {
                    accounts.into_iter().filter_map(|a| {
                        a.id.map(|id| AccountOption {
                            id,
                            broker: a.broker,
                            account_name: a.account_name,
                            user_name: None,
                        })
                    }).collect()
                } else {
                    Vec::new()
                }
            }
        }

        async fn get_account_owner_name(db: &PostgresDB, account_id: i32) -> Option<String> {
            // Query account_rights to find the user
            let result = sqlx::query!(
                "SELECT u.name FROM users u, account_rights ar WHERE ar.account_id = $1 AND ar.user_id = u.id LIMIT 1",
                account_id
            )
            .fetch_optional(&db.pool)
            .await;

            result.ok().flatten().map(|row| row.name)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountView {
    pub id: i32,
    pub broker: String,
    pub account_name: String,
    /// Populated only for admin users; for regular users this is None.
    pub user_name: Option<String>,
}

#[server(GetAccounts, "/api")]
pub async fn get_accounts() -> Result<Vec<AccountOption>, ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use log::debug;

    debug!("get accounts called");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    let db = crate::db::get_db()?;
    Ok(get_accounts_for_user_ssr(user.id, user.is_admin, db).await)
}

#[server(GetUserAccounts, "/api")]
pub async fn get_user_accounts() -> Result<(bool, Vec<AccountView>), ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use log::debug;
    use qualinvest_core::accounts::AccountHandler;
    use qualinvest_core::user::UserHandler;

    debug!("get_user_accounts called");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    let db = crate::db::get_db()?;

    let accounts = if user.is_admin {
        let all_accounts = db.get_all_accounts().await;
        let mut views = Vec::new();
        for account in all_accounts {
            if let Some(account_id) = account.id {
                let user_name = get_account_owner_name(&db, account_id).await;
                views.push(AccountView {
                    id: account_id,
                    broker: account.broker,
                    account_name: account.account_name,
                    user_name,
                });
            }
        }
        views
    } else {
        match db.get_user_accounts(user.id).await {
            Ok(accts) => accts
                .into_iter()
                .filter_map(|a| {
                    a.id.map(|id| AccountView {
                        id,
                        broker: a.broker,
                        account_name: a.account_name,
                        user_name: None,
                    })
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    };

    Ok((user.is_admin, accounts))
}

#[server(InsertAccount, "/api")]
pub async fn insert_account(account: AccountView) -> Result<i32, ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use log::debug;
    use qualinvest_core::accounts::{Account, AccountHandler};
    use qualinvest_core::user::UserHandler;

    debug!("insert_account called: {:?}", account);

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    let db = crate::db::get_db()?;

    let new_account = Account {
        id: None,
        broker: account.broker,
        account_name: account.account_name,
    };

    let account_id = db
        .insert_account_if_new(&new_account)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to insert account: {}", e)))?;

    // Determine which user to assign the account to
    let target_user_id = if user.is_admin {
        if let Some(ref user_name) = account.user_name {
            if !user_name.is_empty() {
                db.get_user_id(user_name)
                    .await
                    .ok_or_else(|| ServerFnError::new(format!("User '{}' not found", user_name)))?
            } else {
                user.id
            }
        } else {
            user.id
        }
    } else {
        user.id
    };

    db.add_account_right(target_user_id, account_id)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to add account right: {}", e)))?;

    Ok(account_id)
}

#[server(UpdateAccount, "/api")]
pub async fn update_account(account: AccountView) -> Result<(), ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use log::debug;
    use qualinvest_core::accounts::{Account, AccountHandler};
    use qualinvest_core::user::UserHandler;

    debug!("update_account called: {:?}", account);

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    let db = crate::db::get_db()?;

    // Verify access
    if !user.is_admin {
        let user_accounts = db
            .get_user_accounts(user.id)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to get user accounts: {}", e)))?;
        let user_account_ids: Vec<i32> = user_accounts.iter().filter_map(|a| a.id).collect();
        if !user_account_ids.contains(&account.id) {
            return Err(ServerFnError::new(format!(
                "Forbidden: Cannot access account {}",
                account.id
            )));
        }
    }

    let updated_account = Account {
        id: Some(account.id),
        broker: account.broker,
        account_name: account.account_name,
    };

    db.update_account(&updated_account)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to update account: {}", e)))?;

    // If admin specified a user_name, update account ownership
    if user.is_admin {
        if let Some(ref user_name) = account.user_name {
            if !user_name.is_empty() {
                let target_user_id = db
                    .get_user_id(user_name)
                    .await
                    .ok_or_else(|| ServerFnError::new(format!("User '{}' not found", user_name)))?;

                // Remove existing rights for this account
                let old_owner_name = get_account_owner_name(&db, account.id).await;
                if let Some(old_name) = old_owner_name {
                    if let Some(old_user_id) = db.get_user_id(&old_name).await {
                        if old_user_id != target_user_id {
                            let _ = db.remove_account_right(old_user_id, account.id).await;
                        }
                    }
                }

                db.add_account_right(target_user_id, account.id)
                    .await
                    .map_err(|e| {
                        ServerFnError::new(format!("Failed to update account right: {}", e))
                    })?;
            }
        }
    }

    Ok(())
}

#[server(DeleteAccount, "/api")]
pub async fn delete_account(account_id: i32) -> Result<(), ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use log::debug;
    use qualinvest_core::accounts::AccountHandler;
    use qualinvest_core::user::UserHandler;

    debug!("delete_account called for id={}", account_id);

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    let db = crate::db::get_db()?;

    // Verify access
    if !user.is_admin {
        let user_accounts = db
            .get_user_accounts(user.id)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to get user accounts: {}", e)))?;
        let user_account_ids: Vec<i32> = user_accounts.iter().filter_map(|a| a.id).collect();
        if !user_account_ids.contains(&account_id) {
            return Err(ServerFnError::new(format!(
                "Forbidden: Cannot access account {}",
                account_id
            )));
        }
    }

    db.delete_account(account_id)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to delete account: {}", e)))
}

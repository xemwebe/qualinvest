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

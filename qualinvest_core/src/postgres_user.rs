use async_trait::async_trait;
use finql::datatypes::DataError;
use finql::postgres::PostgresDB;

use crate::accounts::Account;
use crate::user::{User, UserHandler, UserSettings};

#[async_trait]
impl UserHandler for PostgresDB {
    /// Clean database by dropping all tables related to user management and run init_users
    async fn clean_users(&self) -> Result<(), DataError> {
        sqlx::query!("DROP TABLE IF EXISTS account_rights")
            .execute(&self.pool)
            .await?;
        self.init_users().await?;
        sqlx::query!("DROP TABLE IF EXISTS users")
            .execute(&self.pool)
            .await?;
        self.init_users().await?;
        Ok(())
    }

    /// Set up new tables for user management
    async fn init_users(&self) -> Result<(), DataError> {
        sqlx::query!(
            "CREATE TABLE IF NOT EXISTS users (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            display TEXT,
            salt_hash TEXT NOT NULL,
            is_admin BOOLEAN NOT NULL DEFAULT False,
            UNIQUE (name))"
        )
        .execute(&self.pool)
        .await?;
        sqlx::query!(
            "CREATE TABLE IF NOT EXISTS account_rights (
            id SERIAL PRIMARY KEY,
            user_id INTEGER NOT NULL,
            account_id INTEGER NOT NULL,
            FOREIGN KEY(user_id) REFERENCES users(id),
            FOREIGN KEY(account_id) REFERENCES accounts(id))"
        )
        .execute(&self.pool)
        .await?;
        sqlx::query!(
            "CREATE TABLE IF NOT EXISTS user_settings (
                id SERIAL PRIMARY KEY,
                user_id INTEGER UNIQUE,
                settings JSON,
                FOREIGN KEY(user_id) REFERENCES users(id))"
        )
        .execute(&self.pool)
        .await?;
        sqlx::query!("CREATE EXTENSION pgcrypto")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Insert new user into database, fails if user with same name already exists
    async fn insert_user(&self, user: &User, password: &str) -> Result<i32, DataError> {
        let row = sqlx::query!(
            "INSERT INTO users (name, display, salt_hash, is_admin)
                VALUES ($1, $2, crypt($3,gen_salt('bf',8)), $4) RETURNING id",
            user.name,
            user.display,
            password,
            user.is_admin,
        )
        .fetch_one(&self.pool)
        .await?;
        let id: i32 = row.id;
        Ok(id)
    }

    /// Get full user information if user name and password are valid
    async fn get_user_by_credentials(&self, name: &str, password: &str) -> Option<User> {
        match sqlx::query!(
            "SELECT id, name, display, is_admin FROM users WHERE name = $1 AND
                salt_hash = crypt($2, salt_hash)",
            name,
            password,
        )
        .fetch_one(&self.pool)
        .await
        {
            Err(_) => None,
            Ok(row) => {
                let id: i32 = row.id;
                Some(User {
                    id: Some(id),
                    name: row.name,
                    display: if row.display.is_empty() {
                        None
                    } else {
                        Some(row.display)
                    },
                    is_admin: row.is_admin,
                })
            }
        }
    }

    /// Get full user information for given user id
    async fn get_user_by_id(&self, user_id: i32) -> Option<User> {
        match sqlx::query!(
            "SELECT id, name, display, is_admin FROM users WHERE id = $1",
            user_id,
        )
        .fetch_one(&self.pool)
        .await
        {
            Err(_) => None,
            Ok(row) => {
                let id: i32 = row.id;
                Some(User {
                    id: Some(id),
                    name: row.name,
                    display: if row.display.is_empty() {
                        None
                    } else {
                        Some(row.display)
                    },
                    is_admin: row.is_admin,
                })
            }
        }
    }

    /// Get user id for given name if it exists
    async fn get_user_id(&self, name: &str) -> Option<i32> {
        match sqlx::query!("SELECT id FROM users WHERE name = $1", name,)
            .fetch_one(&self.pool)
            .await
        {
            Err(_) => None,
            Ok(row) => {
                let id: i32 = row.id;
                Some(id)
            }
        }
    }

    /// Get user id for given name if user exists and is admin
    async fn get_admin_id(&self, name: &str) -> Option<i32> {
        match sqlx::query!("SELECT id FROM users WHERE name = $1 AND is_admin", name,)
            .fetch_one(&self.pool)
            .await
        {
            Err(_) => None,
            Ok(row) => {
                let id: i32 = row.id;
                Some(id)
            }
        }
    }

    /// Get user id if user name and password are valid
    async fn get_user_id_by_credentials(&self, name: &str, password: &str) -> Option<i32> {
        match sqlx::query!(
            "SELECT id FROM users WHERE name = $1 AND
                salt_hash = crypt($2, salt_hash)",
            name,
            password,
        )
        .fetch_one(&self.pool)
        .await
        {
            Err(_) => None,
            Ok(row) => {
                let id: i32 = row.id;
                Some(id)
            }
        }
    }

    /// Get list of all users
    async fn get_all_users(&self) -> Vec<User> {
        let rows = sqlx::query!("SELECT id, name, display, is_admin FROM users")
            .fetch_all(&self.pool)
            .await;
        let mut users = Vec::new();
        if let Ok(rows) = rows {
            for row in rows {
                let id: i32 = row.id;
                users.push(User {
                    id: Some(id),
                    name: row.name,
                    display: if row.display.is_empty() {
                        None
                    } else {
                        Some(row.display)
                    },
                    is_admin: row.is_admin,
                });
            }
        }
        users
    }

    /// Update user
    async fn update_user(&self, user: &User) -> Result<(), DataError> {
        if user.id.is_none() {
            return Err(DataError::NotFound(
                "not yet stored to database".to_string(),
            ));
        }
        let id = user.id.unwrap();
        sqlx::query!(
            "UPDATE users SET name=$2, display=$3, is_admin=$4 WHERE id=$1",
            id,
            user.name,
            user.display,
            user.is_admin
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update user password
    async fn update_password(&self, user_id: i32, new_password: &str) -> Result<(), DataError> {
        sqlx::query!(
            "UPDATE users SET salt_hash=crypt($2, gen_salt('bf',8)) WHERE id=$1",
            user_id,
            new_password,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Remove all user information form data base
    async fn delete_user(&self, user_id: i32) -> Result<(), DataError> {
        sqlx::query!("DELETE FROM users WHERE id=$1;", user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Give user identified by user_id the right to access account given by account_id
    async fn add_account_right(&self, user_id: i32, account_id: i32) -> Result<i32, DataError> {
        let rows = sqlx::query!(
            "SELECT id FROM account_rights WHERE user_id=$1 AND account_id=$2",
            user_id,
            account_id
        )
        .fetch_all(&self.pool)
        .await?;
        if !rows.is_empty() {
            let id: i32 = rows[0].id;
            Ok(id)
        } else {
            let row = sqlx::query!(
                "INSERT INTO account_rights (user_id, account_id) VALUES ($1, $2) RETURNING id",
                user_id,
                account_id
            )
            .fetch_one(&self.pool)
            .await?;
            let id: i32 = row.id;
            Ok(id)
        }
    }

    /// Remove right to access account given by account_id from user with id user_id
    async fn remove_account_right(&self, user_id: i32, account_id: i32) -> Result<(), DataError> {
        sqlx::query!(
            "DELETE FROM account_rights WHERE user_id=$1 AND account_id=$2",
            user_id,
            account_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get list of account ids a user as access to
    async fn get_user_accounts(&self, user_id: i32) -> Result<Vec<Account>, DataError> {
        let rows = sqlx::query!("SELECT a.id, a.broker, a.account_name FROM accounts a, account_rights r WHERE r.account_id = a.id AND r.user_id=$1",
                user_id).fetch_all(&self.pool).await?;
        let mut accounts = Vec::new();
        for row in rows {
            let id: i32 = row.id;
            accounts.push(Account {
                id: Some(id),
                broker: row.broker,
                account_name: row.account_name,
            });
        }
        Ok(accounts)
    }

    /// Remove all ids form ids the user has no access to
    async fn valid_accounts(&self, user_id: i32, ids: &[i32]) -> Result<Vec<i32>, DataError> {
        let user_accounts = self.get_user_accounts(user_id).await?;
        let mut valid_ids = Vec::new();
        for id in ids {
            for account in &user_accounts {
                if account.id.unwrap() == *id {
                    valid_ids.push(*id);
                    continue;
                }
            }
        }
        Ok(valid_ids)
    }

    /// Get the account the transaction given by id belongs to,
    /// if the user given by user_id as the right to access this account
    async fn get_transaction_account_if_valid(
        &self,
        trans_id: i32,
        user_id: i32,
    ) -> Result<Account, DataError> {
        let row = sqlx::query!(
            r#"SELECT DISTINCT a.id, a.broker, a.account_name
            FROM
                accounts a,
                account_rights ar,
                account_transactions at,
                users u
            WHERE
                at.account_id = a.id
                AND at.transaction_id = $1
                AND u.id = $2
                AND ar.account_id = a.id
                AND (ar.user_id = u.id
                    OR u.is_admin
                    );
            "#,
            trans_id,
            user_id
        )
        .fetch_one(&self.pool)
        .await?;
        let id: i32 = row.id;
        Ok(Account {
            id: Some(id),
            broker: row.broker,
            account_name: row.account_name,
        })
    }

    /// Remove this transaction and all its dependencies, if it belongs to an account the user has
    /// access rights for
    async fn remove_transaction(&self, trans_id: i32, user_id: i32) -> Result<(), DataError> {
        // Get list of related trades
        let rows = sqlx::query!(r#"SELECT DISTINCT t.id FROM transactions t, users u, account_rights ar, accounts a, account_transactions at
        WHERE (t.id=$1 OR t.related_trans=$1)
        AND u.id=$2
        AND (u.is_admin
            OR
            (
                ar.user_id = u.id
                AND ar.account_id = a.id
                AND at.account_id = a.id
                AND at.transaction_id = t.id
            )
        );"#,
        trans_id, user_id)
        .fetch_all(&self.pool).await?;
        let mut ids = Vec::new();
        for row in rows {
            let id: i32 = row.id;
            ids.push(id);
        }
        for id in ids {
            sqlx::query!(
                "DELETE FROM account_transactions WHERE transaction_id=$1",
                id
            )
            .execute(&self.pool)
            .await?;
            sqlx::query!("DELETE FROM transactions WHERE id=$1", id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    /// Get user settings
    async fn get_user_settings(&self, user_id: i32) -> UserSettings {
        let row = sqlx::query!(
            "SELECT settings FROM user_settings WHERE user_id=$1",
            user_id
        )
        .fetch_one(&self.pool)
        .await;
        if let Ok(row) = row {
            if let Some(settings_value) = row.settings {
                let settings: UserSettings =
                    serde_json::value::from_value(settings_value).unwrap_or_default();
                return settings;
            }
        }
        Default::default()
    }

    /// Set user settings
    async fn set_user_settings(
        &self,
        user_id: i32,
        settings: &UserSettings,
    ) -> Result<(), DataError> {
        let settings_json = serde_json::to_value(settings)?;
        sqlx::query!(
            r"INSERT INTO user_settings (user_id, settings)
        VALUES($1,$2)
        ON CONFLICT (user_id)
        DO
        UPDATE SET settings = $2",
            user_id,
            settings_json
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

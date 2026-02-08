use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserView {
    pub id: i32,
    pub name: String,
    pub display: String,
    pub is_admin: bool,
    pub password: String,
}

#[server(GetAllUsers, "/api")]
pub async fn get_all_users() -> Result<Vec<UserView>, ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use log::debug;
    use qualinvest_core::user::UserHandler;

    debug!("get_all_users called");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    if !user.is_admin {
        return Err(ServerFnError::new("Forbidden: Admin access required"));
    }

    let db = crate::db::get_db()?;
    let users = db.get_all_users().await;

    Ok(users
        .into_iter()
        .map(|u| UserView {
            id: u.id.unwrap_or(0),
            name: u.name,
            display: u.display.unwrap_or_default(),
            is_admin: u.is_admin,
            password: String::new(),
        })
        .collect())
}

#[server(InsertUser, "/api")]
pub async fn insert_user(user: UserView) -> Result<i32, ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use log::debug;
    use qualinvest_core::user::{User, UserHandler};

    debug!("insert_user called for name={}", user.name);

    let auth: AuthSession<PostgresBackend> = expect_context();
    let current_user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    if !current_user.is_admin {
        return Err(ServerFnError::new("Forbidden: Admin access required"));
    }

    if user.password.is_empty() {
        return Err(ServerFnError::new("Password is required for new users"));
    }

    let db = crate::db::get_db()?;
    let new_user = User {
        id: None,
        name: user.name,
        display: if user.display.is_empty() {
            None
        } else {
            Some(user.display)
        },
        is_admin: user.is_admin,
    };

    db.insert_user(&new_user, &user.password)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to insert user: {}", e)))
}

#[server(UpdateUser, "/api")]
pub async fn update_user(user: UserView) -> Result<(), ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use log::debug;
    use qualinvest_core::user::{User, UserHandler};

    debug!("update_user called for id={}", user.id);

    let auth: AuthSession<PostgresBackend> = expect_context();
    let current_user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    if !current_user.is_admin {
        return Err(ServerFnError::new("Forbidden: Admin access required"));
    }

    let db = crate::db::get_db()?;
    let updated_user = User {
        id: Some(user.id),
        name: user.name,
        display: if user.display.is_empty() {
            None
        } else {
            Some(user.display)
        },
        is_admin: user.is_admin,
    };

    db.update_user(&updated_user)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to update user: {}", e)))?;

    if !user.password.is_empty() {
        db.update_password(user.id, &user.password)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to update password: {}", e)))?;
    }

    Ok(())
}

#[server(DeleteUser, "/api")]
pub async fn delete_user(user_id: i32) -> Result<(), ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use log::debug;
    use qualinvest_core::user::UserHandler;

    debug!("delete_user called for id={}", user_id);

    let auth: AuthSession<PostgresBackend> = expect_context();
    let current_user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    if !current_user.is_admin {
        return Err(ServerFnError::new("Forbidden: Admin access required"));
    }

    let db = crate::db::get_db()?;
    db.delete_user(user_id)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to delete user: {}", e)))
}

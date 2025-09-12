use cfg_if::cfg_if;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub is_admin: bool,
    #[cfg(feature = "ssr")]
    auth_hash: String,
}

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
        use axum_login::{AuthUser, AuthnBackend, UserId};
        use qualinvest_core::user::UserHandler;
        use finql::postgres::PostgresDB;
        use qualinvest_core::user::User as BEUser;
        use log::debug;
        use crate::db;
        //use tokio::task;

// for later: migrate to hasing with argon2 outside of postgresql
//        fn verify_password(&self, password: &str, password_hash: &str) -> Result<(), argon2::password_hash::Error> {
//            let parsed_hash = PasswordHash::new(password_hash)?;
//            Argon2::default().verify_password(password.as_bytes(), &parsed_hash)
//        }
        pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
            let salt = SaltString::generate(&mut rand::rngs::OsRng);
            let argon2 = Argon2::default();
            let password_hash = argon2.hash_password(password.as_bytes(), &salt)?;
            Ok(password_hash.to_string())
        }

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub struct Credentials {
            pub username: String,
            pub password: String,
        }

        impl std::fmt::Debug for User {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("User")
                    .field("id", &self.id)
                    .field("name", &self.name)
                    .field("is_admin", &self.is_admin)
                    .field("password", &"[redacted]")
                    .finish()
            }
        }

        impl From<BEUser> for User {
            fn from(user: BEUser) -> Self {
                let name = if let Some(name) = user.display {
                    name.clone()
                } else {
                    user.name.clone()
                };
                // verify how this is used
                let auth_hash = hash_password(&name).unwrap_or_default();
                debug!("auth_hash: {auth_hash}");
                User {
                    id: user.id.unwrap_or(-1),
                    name: name,
                    is_admin: user.is_admin,
                    auth_hash,
                }
            }
        }

        impl AuthUser for User {
            type Id = i32;

            fn id(&self) -> Self::Id {
                self.id
            }

            fn session_auth_hash(&self) -> &[u8] {
                self.auth_hash.as_bytes()
            }
        }

        #[derive(Debug, thiserror::Error)]
        pub enum Error {
            #[error(transparent)]
            Sqlx(#[from] sqlx::Error),

            #[error("unknown server error")]
            ServerError,

//            #[error(transparent)]
//            TaskJoin(#[from] task::JoinError),
        }

        #[derive(Clone)]
        pub struct PostgresBackend {
            //db: sqlx::PgPool,
        }

        impl PostgresBackend {
            pub fn new(//db: sqlx::PgPool
            ) -> Self {
                Self { //db
                }
            }
        }

        #[async_trait::async_trait]
        impl AuthnBackend for PostgresBackend {
            type User = User;
            type Credentials = Credentials;
            type Error = Error;

            async fn authenticate(
                &self,
                creds: Self::Credentials,
            ) -> Result<Option<Self::User>, Self::Error> {
                debug!("Authenticating user: {}", creds.username);
                let db = db::get_db().map_err(|_| Error::ServerError)?;
                Ok(db.get_user_by_credentials(&creds.username, &creds.password).await.map(|user| user.into()))

//                let user: Option<Self::BEUser> = sqlx::query_as("select * from users where username = $1")
//                    .bind(&creds.username)
//                    .fetch_optional(&self.db)
//                    .await?;
//
//                // Verifying the password is blocking and potentially slow, so we'll do so via
//                // `spawn_blocking`
//                task::spawn_blocking(|| {
//                    // We're suing passowrd-based authentication--this works by comparing our form
//                    // input with an argon2 password hash.
//                    Ok(user.filter(|user| verify_password(creds.password, &user.password).is_ok()).map(|user| user.into()))
//                })
//                .await?;
            }

            async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
                debug!("Get user info for user_id: {}", user_id);
                let db = db::get_db().map_err(|_| Error::ServerError)?;

                Ok(db.get_user_by_id(*user_id).await.map(|user| user.into()))
            }
        }

        pub type AuthSession = axum_login::AuthSession<PostgresBackend>;
    }
}

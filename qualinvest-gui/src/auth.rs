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
        use axum_login::{AuthUser, AuthnBackend, UserId};
        use log::debug;
        //use tokio::task;

// for later: migrate to hasing with argon2 outside of postgresql
//        fn verify_password(&self, password: &str, password_hash: &str) -> Result<(), argon2::password_hash::Error> {
//            let parsed_hash = PasswordHash::new(password_hash)?;
//            Argon2::default().verify_password(password.as_bytes(), &parsed_hash)
//        }
//        pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
//            let salt = SaltString::generate(&mut rand::rngs::OsRng);
//            let argon2 = Argon2::default();
//            let password_hash = argon2.hash_password(password.as_bytes(), &salt)?;
//            Ok(password_hash.to_string())
//        }

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
            db: sqlx::PgPool,
        }

        impl PostgresBackend {
            pub fn new(db: sqlx::PgPool
            ) -> Self {
                Self { db
                }
            }
        }

        #[async_trait::async_trait]
        impl AuthnBackend for PostgresBackend {
            type User = User;
            type Credentials = Credentials;
            type Error = Error;
//            async fn authenticate(
//                &self,
//                creds: Self::Credentials,
//            ) -> Result<Option<Self::User>, Self::Error> {
//                let user: Option<Self::User> = sqlx::query_as("select * from users where username = $1")
//                    .bind(creds.username)
//                    .fetch_optional(&self.db)
//                    .await?;
//
//                // Verifying the password is blocking and potentially slow, so we'll do so via
//                // `spawn_blocking`.
//                task::spawn_blocking(|| {
//                    // We're using password-based authentication--this works by comparing our form
//                    // input with an argon2 password hash.
//                    Ok(user.filter(|user| verify_password(creds.password, &user.password).is_ok()))
//                })
//                .await?
//            }
//
//            async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
//                let user = sqlx::query_as("select * from users where id = $1")
//                    .bind(user_id)
//                    .fetch_optional(&self.db)
//                    .await?;
//
//                Ok(user)
//            }

            async fn authenticate(
                &self,
                creds: Self::Credentials,
            ) -> Result<Option<Self::User>, Self::Error> {
                debug!("Authenticating user: {}", creds.username);
                let row = sqlx::query!(
                    "SELECT id, name, display, is_admin, salt_hash FROM users WHERE name = $1 AND
                        salt_hash = crypt($2, salt_hash)",
                    creds.username,
                    creds.password,
                )
                .fetch_one(&self.db)
                .await?;

                let id: i32 = row.id;
                Ok(Some(User {
                    id: id,
                    name: if row.display.is_empty() {
                        row.name
                    } else {
                        row.display
                    },
                    is_admin: row.is_admin,
                    auth_hash: row.salt_hash,
                }))

                //let db = db::get_db().map_err(|_| Error::ServerError)?;
                //Ok(db.get_user_by_credentials_with_hash(&creds.username, &creds.password).await.map(|user| user.into()))

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
                let row = sqlx::query!(
                    "SELECT id, name, display, is_admin, salt_hash FROM users WHERE id = $1",
                    user_id,
                )
                .fetch_one(&self.db)
                .await?;

                let id: i32 = row.id;
                Ok(Some(User {
                    id: id,
                    name: if row.display.is_empty() {
                        row.name
                    } else {
                        row.display
                    },
                    is_admin: row.is_admin,
                    auth_hash: row.salt_hash,
                }))
//                debug!("Get user info for user_id: {}", user_id);
//                let db = db::get_db().map_err(|e| {
//                    debug!("Error getting database connection: {}", e);
//                    Error::ServerError
//                })?;
//                Ok(db.get_user_by_id_with_hash(*user_id).await.map(|user| user.into()))
            }
        }
    } else {

        impl std::fmt::Debug for User {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("User")
                    .field("id", &self.id)
                    .field("name", &self.name)
                    .field("is_admin", &self.is_admin)
                    .finish()
            }
        }
    }
}

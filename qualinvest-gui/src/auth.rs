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
                    id,
                    name: if row.display.is_empty() {
                        row.name
                    } else {
                        row.display
                    },
                    is_admin: row.is_admin,
                    auth_hash: row.salt_hash,
                }))

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
                    id,
                    name: if row.display.is_empty() {
                        row.name
                    } else {
                        row.display
                    },
                    is_admin: row.is_admin,
                    auth_hash: row.salt_hash,
                }))
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

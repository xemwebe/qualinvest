use cfg_if::cfg_if;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i32,
    pub username: String,
    #[cfg(feature = "ssr")]
    pub password_hash: String,
}

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
        use axum_login::{AuthUser, AuthnBackend, UserId};

        impl std::fmt::Debug for User {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("User")
                    .field("id", &self.id)
                    .field("username", &self.username)
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
                self.password_hash.as_bytes()
            }
        }

        #[derive(Clone)]
        pub struct Backend {
            users: Vec<User>,
        }

        impl Backend {
            pub fn new() -> Self {
                // For demo purposes, create a hardcoded user
                let password_hash = hash_password("admin123").unwrap();
                let users = vec![User {
                    id: 1,
                    username: "admin".to_string(),
                    password_hash,
                }];

                Self { users }
            }

            fn verify_password(&self, password: &str, password_hash: &str) -> Result<(), argon2::password_hash::Error> {
                let parsed_hash = PasswordHash::new(password_hash)?;
                Argon2::default().verify_password(password.as_bytes(), &parsed_hash)
            }
        }

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

        #[async_trait::async_trait]
        impl AuthnBackend for Backend {
            type User = User;
            type Credentials = Credentials;
            type Error = std::convert::Infallible;

            async fn authenticate(
                &self,
                creds: Self::Credentials,
            ) -> Result<Option<Self::User>, Self::Error> {
                let user = self
                    .users
                    .iter()
                    .find(|user| user.username == creds.username)
                    .cloned();

                if let Some(user) = user {
                    if self.verify_password(&creds.password, &user.password_hash).is_ok() {
                        return Ok(Some(user));
                    }
                }

                Ok(None)
            }

            async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
                let user = self
                    .users
                    .iter()
                    .find(|user| user.id == *user_id)
                    .cloned();

                Ok(user)
            }
        }

        pub type AuthSession = axum_login::AuthSession<Backend>;
    }
}

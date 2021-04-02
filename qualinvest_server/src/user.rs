use std::sync::Arc;
use std::collections::HashMap;
use async_trait::async_trait;

use rocket::Request;
use rocket::request::{FromRequest, Outcome};

use super::auth::authorization::*;
use qualinvest_core::user::UserHandler;
use qualinvest_core::accounts::Account;

/// The UserCookie type is used to indicate a user has logged in as an user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCookie {
    pub userid: usize,
    pub username: String,
    pub display: Option<String>,
    pub is_admin: bool,
}

impl UserCookie {
    pub async fn get_accounts(&self, db: Arc<dyn UserHandler+Send+Sync>) -> Option<Vec<Account>> {
        if self.is_admin {
            db.get_all_accounts().await.ok()
        } else {
            db.get_user_accounts(self.userid).await.ok()
        }
    }
}

/// The UserForm type is used to process a user attempting to login as an user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserForm {
    pub username: String,
    pub password: String,
    pub redirect: String,
}

impl CookieId for UserCookie {
    fn cookie_id<'a>() -> &'a str {
        "plain_acid"
    }
}

impl CookieId for UserForm {
    fn cookie_id<'a>() -> &'a str {
        "plain_acid"
    }
} 

impl AuthorizeCookie for UserCookie {
    /// The store_cookie() method should contain code that
    /// converts the specified data structure into a string
    fn store_cookie(&self) -> String {
        ::serde_json::to_string(self).expect("Could not serialize")
    }
    
    
    /// The retrieve_cookie() method deserializes a string
    /// into a cookie data type.
    #[allow(unused_variables)]
    fn retrieve_cookie(string: String) -> Option<Self> {
        let mut des_buf = string.clone();
        let des: Result<UserCookie, _> = ::serde_json::from_str(&mut des_buf);
        if let Ok(cooky) = des {
            Some(cooky)
        } else {
            None
        }
    }
}

#[async_trait]
impl AuthorizeForm for UserForm {
    type CookieType = UserCookie;
    
    /// Authenticate the credentials inside the login form
    async fn authenticate(&self, db: Arc<dyn UserHandler+Send+Sync>) -> Result<Self::CookieType, AuthFail> {
        let user = db.get_user_by_credentials(&self.username, &self.password).await
            .ok_or(AuthFail::new(self.username.clone(), "Authentication failed.".to_string()))?;
        if user.id.is_none() { return Err(AuthFail::new(self.username.clone(), "Authentication failed.".to_string())); }
        Ok(UserCookie {
            userid: user.id.unwrap(),
            username: user.name,
            display: user.display,
            is_admin: user.is_admin,
        })
    }
    
    /// Create a new login form instance
    fn new_form(user: &str, pass: &str, extras: Option<HashMap<String, String>>) -> Self {
        let redirect = match extras {
            Some(map) => {
                match map.get("redirect") {
                    Some(redirect) => redirect.clone(),
                    None => "/login".to_string(),
                }
            },
            None => "/login".to_string(),
        };
        UserForm {
            username: user.to_string(),
            password: pass.to_string(),
            redirect,
        }
    }
    
}

#[async_trait]
impl<'r> FromRequest<'r> for UserCookie {
    type Error = ();
    
    /// The from_request inside the file defining the custom data types
    /// enables the type to be checked directly in a route as a request guard
    /// 
    /// This is not needed but highly recommended.  Otherwise you would need to use:
    /// 
    /// `#[get("/protected")] fn admin_page(admin: AuthCont<UserCookie>)`
    /// 
    /// instead of:
    /// 
    /// `#[get("/protected")] fn admin_page(admin: UserCookie)`
    async fn from_request(request: &'r Request<'_>) -> ::rocket::request::Outcome<UserCookie,Self::Error>{
        let cid = UserCookie::cookie_id();
        let mut cookies = request.cookies();
        
        match cookies.get_private(cid) {
            Some(cookie) => {
                if let Some(cookie_deserialized) = UserCookie::retrieve_cookie(cookie.value().to_string()) {
                    Outcome::Success(
                        cookie_deserialized
                    )
                } else {
                    Outcome::Forward(())
                }
            },
            None => Outcome::Forward(())
        }
    }
}
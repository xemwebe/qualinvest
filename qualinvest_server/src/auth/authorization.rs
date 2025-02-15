use std::collections::HashMap;
use std::marker::Sized;
use std::sync::Arc;

use async_trait::async_trait;
use rocket::form::FromForm;
use rocket::http::{Cookie, CookieJar, Status};
use rocket::request::{FromRequest, Outcome};
use rocket::response::{Flash, Redirect};
use rocket::Request;

use qualinvest_core::{
    sanitization::{sanitize, sanitize_password},
    user::UserHandler,
};

#[derive(Debug, Clone, FromForm)]
pub struct UserQuery {
    pub user: String,
}

#[derive(Debug, Clone)]
pub struct AuthCont<T: AuthorizeCookie> {
    pub cookie: T,
}

#[derive(Debug, Clone, FromForm)]
pub struct AuthFail {
    pub user: String,
    pub msg: String,
}

impl AuthFail {
    pub fn new(user: String, msg: String) -> AuthFail {
        AuthFail { user, msg }
    }
}

pub trait AuthorizeCookie: CookieId {
    /// Serialize the cookie data type - must be implemented by cookie data type
    fn store_cookie(&self) -> String;

    /// Deserialize the cookie data type - must be implemented by cookie data type
    fn retrieve_cookie(cookie_string: &str) -> Option<Self>
    where
        Self: Sized;

    /// Deletes a cookie.  This does not need to be implemented, it defaults to removing
    /// the private key with the named specified by cookie_id() method.

    fn delete_cookie(cookies: &CookieJar) {
        cookies.remove_private(Cookie::from(Self::cookie_id()));
    }
}

/// The CookieId trait contains a single method, `cookie_id()`.
/// The `cookie_id()` function returns the name or id of the cookie.
/// Note: if you have another cookie of the same name that is secured
/// that already exists (say created by running the tls_example then database_example)
/// if your cookies have the same name it will not work.  This is because
/// if the existing cookie is set to secured you attempt to login without
/// using tls the cookie will not work correctly and login will fail.
pub trait CookieId {
    /// Ensure `cookie_id()` does not conflict with other cookies that
    /// may be set using secured when not using tls.  Secured cookies
    /// will only work using tls and cookies of the same name could
    /// create problems.
    fn cookie_id<'a>() -> &'a str {
        "sid"
    }
}

#[async_trait]
pub trait AuthorizeForm: CookieId {
    type CookieType: AuthorizeCookie;

    /// Determine whether the login form structure containts
    /// valid credentials, otherwise send back the username and
    /// a message indicating why it failed in the `AuthFail` struct
    ///
    /// Must be implemented on the login form structure
    async fn authenticate(
        &self,
        db: Arc<dyn UserHandler + Send + Sync>,
    ) -> Result<Self::CookieType, AuthFail>;

    /// Create a new login form Structure with
    /// the specified username and password.
    /// The first parameter is the username, then password,
    /// and then optionally a HashMap containing any extra fields.
    ///
    /// Must be implemented on the login form structure
    ///
    // /// The password is a u8 slice, allowing passwords to be stored without
    // /// being converted to hex.  The slice is sufficient because new_form()
    // /// is called within the from_form() function, so when the password is
    // /// collected as a vector of bytes the reference to those bytes are sent
    // /// to the new_form() method.
    fn new_form(
        user_name: &str,
        password: &str,
        extra_fields: Option<HashMap<String, String>>,
    ) -> Self;

    /// The `fail_url()` method is used to create a url that the user is sent
    /// to when the authentication fails.  The default implementation
    /// redirects the user to the /page?user=<ateempted_username>
    /// which enables the form to display the username that was attempted
    /// and unlike FlashMessages it will persist across refreshes
    fn fail_url(user: &str) -> String {
        let mut output = String::with_capacity(user.len() + 10);
        output.push_str("?user=");
        output.push_str(user);
        output
    }

    /// Sanitizes the username before storing in the login form structure.
    /// The default implementation uses the `sanitize()` function in the
    /// `sanitization` module.  This can be overriden in your
    /// `impl AuthorizeForm for {login structure}` implementation to
    /// customize how the text is cleaned/sanitized.
    fn clean_username(string: &str) -> String {
        sanitize(string)
    }
    /// Sanitizes the password before storing in the login form structure.
    /// The default implementation uses the `sanitize_oassword()` function in the
    /// `sanitization` module.  This can be overriden in your
    /// `impl AuthorizeForm for {login structure}` implementation to
    /// customize how the text is cleaned/sanitized.
    fn clean_password(string: &str) -> String {
        sanitize_password(string)
    }
    /// Sanitizes any extra variables before storing in the login form structure.
    /// The default implementation uses the `sanitize()` function in the
    /// `sanitization` module.  This can be overriden in your
    /// `impl AuthorizeForm for {login structure}` implementation to
    /// customize how the text is cleaned/sanitized.
    fn clean_extras(string: &str) -> String {
        sanitize(string)
    }

    /// Redirect the user to one page on successful authentication or
    /// another page (with a `FlashMessage` indicating why) if authentication fails.
    ///
    /// `FlashMessage` is used to indicate why the authentication failed
    /// this is so that the user can see why it failed but when they refresh
    /// it will disappear, enabling a clean start, but with the user name
    /// from the url's query string (determined by `fail_url()`)
    async fn flash_redirect<'a>(
        &self,
        ok_redir: impl Into<String> + Send + 'a,
        err_redir: impl Into<String> + Send + 'a,
        cookies: &CookieJar<'a>,
        db: Arc<dyn UserHandler + Send + Sync>,
    ) -> Result<Redirect, Flash<Redirect>> {
        match self.authenticate(db).await {
            Ok(cooky) => {
                let cid = Self::cookie_id();
                let contents = cooky.store_cookie();
                cookies.add_private(Cookie::new(cid, contents));
                Ok(Redirect::to(ok_redir.into()))
            }
            Err(fail) => {
                let mut furl = err_redir.into();
                if !&fail.user.is_empty() {
                    let furl_qrystr = Self::fail_url(&fail.user);
                    furl.push_str(&furl_qrystr);
                }
                Err(Flash::error(Redirect::to(furl), &fail.msg))
            }
        }
    }

    /// Redirect the user to one page on successful authentication or
    /// another page if authentication fails.
    async fn redirect(
        &self,
        ok_redir: &str,
        err_redir: &str,
        cookies: &CookieJar,
        db: Arc<dyn UserHandler + Send + Sync>,
    ) -> Result<Redirect, Redirect> {
        match self.authenticate(db).await {
            Ok(cooky) => {
                let cid = Self::cookie_id();
                let contents = cooky.store_cookie();
                cookies.add_private(Cookie::new(cid, contents));
                Ok(Redirect::to(ok_redir.to_string()))
            }
            Err(fail) => {
                let mut furl = String::from(err_redir);
                if !&fail.user.is_empty() {
                    let furl_qrystr = Self::fail_url(&fail.user);
                    furl.push_str(&furl_qrystr);
                }
                Err(Redirect::to(furl))
            }
        }
    }
}

/// # Request Guard
/// Request guard for the AuthCont (Authentication Container).
/// This allows a route to call a user type like:
///
/// ```rust,no_run
///
///     # #![feature(proc_macro_hygiene, decl_macro)]
///     # #[macro_use]
///     # extern crate rocket;
///     # #[macro_use]
///     # extern crate serde;
///     # use rocket::response::content::Html;
///     use rocket_auth_login::authorization::*;
///
///
///     # #[derive(Deserialize)]
///     # pub struct AdministratorCookie {
///     # }
///     # impl CookieId for AdministratorCookie {
///     #  fn cookie_id<'a>() -> &'a str { "acid" }
///     # }
///     # impl AuthorizeCookie for AdministratorCookie {
///     #     fn store_cookie(&self) -> String {
///     #         unimplemented!()
///     #     }
///     #     fn retrieve_cookie(string: String) -> Option<Self> {
///     #         unimplemented!()
///     #     }
///     # }
///
///     #[get("/protected")]
///     fn protected(container: AuthCont<AdministratorCookie>) -> Html<String> {
///         let admin = container.cookie;
///         Html(String::new())
///     }
///
///     # fn main() {
///     #    rocket::ignite().mount("/", routes![]).launch();
///     # }
///
/// ```
///
#[rocket::async_trait]
impl<'r, T: AuthorizeCookie> FromRequest<'r> for AuthCont<T> {
    type Error = ();

    async fn from_request(
        request: &'r Request<'_>,
    ) -> ::rocket::request::Outcome<AuthCont<T>, Self::Error> {
        let cid = T::cookie_id();
        let cookies = request.cookies();

        match cookies.get_private(cid) {
            Some(cookie) => {
                if let Some(cookie_deserialized) = T::retrieve_cookie(cookie.value()) {
                    Outcome::Success(AuthCont {
                        cookie: cookie_deserialized,
                    })
                } else {
                    Outcome::Forward(Status::Forbidden)
                }
            }
            None => Outcome::Forward(Status::Forbidden),
        }
    }
}

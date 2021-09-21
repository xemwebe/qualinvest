/// Viewing and analyzing assets

use rocket::State;
use rocket::response::Redirect;
use rocket_dyn_templates::Template;
use rocket::form::{Form, FromForm};
use super::rocket_uri_macro_error_msg;

use qualinvest_core::accounts::AccountHandler;
use qualinvest_core::user::UserHandler;
use crate::user::UserCookie;
use crate::layout::layout;
use super::ServerState;

/// Structure for storing information in accounts form
#[derive(Debug,Serialize,Deserialize,FromForm)]
pub struct AccountForm {
    pub id: Option<usize>,
    pub broker: Option<String>,
    pub account_name: Option<String>
}

/// Structure for storing information in accounts form
#[derive(Debug,Serialize,Deserialize,FromForm)]
pub struct UserForm {
    pub id: Option<usize>,
    pub name: String,
    pub display: Option<String>,
    pub is_admin: bool,
}

#[get("/accounts")]
pub async fn accounts(user: UserCookie, state: &State<ServerState>) -> Result<Template,Redirect> {
    if !user.is_admin {
        return  Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="You must be admin user to edit assets!")))));
    }

    let db = state.postgres_db.clone();
    let accounts = db.get_all_accounts().await;
    let users = db.get_all_users().await;

    let mut context = state.default_context();
    context.insert("accounts", &accounts);
    context.insert("users", &users);
    context.insert("user", &user);
    Ok(layout("accounts_and_users", &context.into_json()))
}

#[post("/account/add", data = "<form>")]
pub async fn add_account(form: Form<AccountForm>, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {
    if !user.is_admin {
        return  Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="You must be admin user to add accounts!")))));
    }

    Ok(Redirect::to(format!("/{}accounts", state.rel_path)))
}

#[post("/account/update", data = "<form>")]
pub async fn update_account(form: Form<AccountForm>, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {
    if !user.is_admin {
        return  Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="You must be admin user to update accounts!")))));
    }

    Ok(Redirect::to(format!("/{}accounts", state.rel_path)))
}

#[get("/account/delete?<account_id>")]
pub async fn delete_account(account_id: usize, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {
    if !user.is_admin {
        return  Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="You must be admin user to delete accounts!")))));
    }

    Ok(Redirect::to(format!("/{}accounts", state.rel_path)))
}

#[post("/user/add", data = "<form>")]
pub async fn add_user(form: Form<UserForm>, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {
    if !user.is_admin {
        return  Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="You must be admin user to add users!")))));
    }

    Ok(Redirect::to(format!("/{}accounts", state.rel_path)))
}

#[post("/user/update", data = "<form>")]
pub async fn update_user(form: Form<UserForm>, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {
    if !user.is_admin {
        return  Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="You must be admin user to add users!")))));
    }

    Ok(Redirect::to(format!("/{}accounts", state.rel_path)))
}


#[get("/user/delete?<user_id>")]
pub async fn delete_user(user_id: usize, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {
    if !user.is_admin {
        return  Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="You must be admin user to delete users!")))));
    }

    Ok(Redirect::to(format!("/{}accounts", state.rel_path)))
}

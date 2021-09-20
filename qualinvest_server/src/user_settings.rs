use std::collections::HashSet;
use std::str::FromStr;

use rocket_dyn_templates::{Template};
use rocket::{
    State,
    response::Redirect,
    form::{
        Form,
        FromForm,
    },
};
use super::{rocket_uri_macro_login,rocket_uri_macro_error_msg};

use crate::layout::layout;
use crate::form_types::NaiveDateForm;
use qualinvest_core::user::{UserHandler, UserSettings};
use super::ServerState;
use crate::user::UserCookie;

#[derive(Debug,Serialize,Deserialize,FromForm)]
pub struct UserSettingsForm {
    pub account_ids: Vec<String>,
    pub start_date: NaiveDateForm,
    pub end_date: NaiveDateForm,
}

#[get("/settings")]
pub async fn show_settings(user_opt: Option<UserCookie>, state: &State<ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(format!("{}{}",state.rel_path, uri!(login(redirect=Some("transactions"))))));
    }
    let user = user_opt.unwrap();

    let db = state.postgres_db.clone();
    let user_accounts = user.get_accounts(db.clone()).await;
    if user_accounts.is_none() {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="No user account found".to_string())))));
    }
    let user_accounts = user_accounts.unwrap();
    let settings = db.get_user_settings(user.userid).await;

    let mut context = state.default_context();
    context.insert("valid_accounts", &user_accounts);
    context.insert("settings", &settings);
    context.insert("user", &user);
    Ok(layout("user_settings", &context.into_json()))
}

#[post("/save_settings", data="<form>")]
pub async fn save_settings(form: Form<UserSettingsForm>, user_opt: Option<UserCookie>, 
    state: &State<ServerState>) -> Redirect {
    if user_opt.is_none() {
        return Redirect::to(format!("{}{}", state.rel_path, uri!(login(redirect=Some("position")))));
    }
    let user = user_opt.unwrap();

    let db = state.postgres_db.clone();
    let user_accounts = user.get_accounts(db.clone()).await;
    if user_accounts.is_none() {
        return Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="No user account found".to_string()))));
    }

    let mut all_user_account_ids = HashSet::new();
    for account in user_accounts.unwrap() {
        if let Some(id) = account.id {
            all_user_account_ids.insert(id);
        }
    }

    let filter_form = form.into_inner();
    let mut selected_accounts = HashSet::new();
    for account in filter_form.account_ids {
        if let Ok(id) = usize::from_str(&account) {
            selected_accounts.insert(id);
        }
    }
    let account_ids: Vec<_> = all_user_account_ids.intersection(&selected_accounts).cloned().collect();

    let user_settings = UserSettings{
        period_start: Some(filter_form.start_date.date),
        period_end: Some(filter_form.end_date.date),
        account_ids,
    };
    let result = db.set_user_settings(user.userid, &user_settings).await;

    match result {
        Ok(()) => Redirect::to("settings"),
        Err(_) => Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="Failed ot save user settings".to_string())))),
    }
}


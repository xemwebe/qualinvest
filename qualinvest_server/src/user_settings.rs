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

#[get("/settings?<err_msg>")]
pub async fn show_settings(err_msg: Option<String>, user_opt: Option<UserCookie>, state: &State<ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(uri!(ServerState::base(), super::retry_login_flash(redirect=Some("transactions".to_string()), err_msg=Some("Please log-in first.".to_string())))));
    }
    let user = user_opt.unwrap();

    let db = state.postgres_db.clone();
    let user_accounts = user.get_accounts(db.clone()).await;
    let settings = db.get_user_settings(user.userid).await;

    let mut context = state.default_context();
    context.insert("valid_accounts", &user_accounts);
    context.insert("settings", &settings);
    context.insert("user", &user);
    context.insert("err_msg", &err_msg);
    Ok(layout("user_settings", &context.into_json()))
}

#[post("/save_settings", data="<form>")]
pub async fn save_settings(form: Form<UserSettingsForm>, user_opt: Option<UserCookie>, 
    state: &State<ServerState>) -> Result<Redirect, Redirect> {
    let user = user_opt.ok_or_else(||
        Redirect::to(uri!(ServerState::base(), super::retry_login_flash(redirect=Some("save_settings".to_string()), err_msg=Some("Please log-in first.".to_string()))))
    )?;

    let db = state.postgres_db.clone();
    let user_accounts = user.get_accounts(db.clone()).await
        .ok_or_else(|| Redirect::to(uri!(ServerState::base(), show_settings(Some("No user account found".to_string())))))?;

    let mut all_user_account_ids = HashSet::new();
    for account in user_accounts {
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
        Ok(()) => Ok(Redirect::to(uri!(ServerState::base(), crate::position::position()))),
        Err(_) => Err(Redirect::to(uri!(ServerState::base(), show_settings(Some("Failed ot save user settings".to_string()))))),
    }
}


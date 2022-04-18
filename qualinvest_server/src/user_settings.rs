use std::collections::HashSet;
use std::str::FromStr;

use rocket::{
    form::{Form, FromForm},
    response::Redirect,
    State,
};
use rocket_dyn_templates::Template;

use super::ServerState;
use crate::form_types::NaiveDateForm;
use crate::layout::layout;
use crate::user::UserCookie;
use finql::period_date::PeriodDate;
use qualinvest_core::user::{UserHandler, UserSettings};

#[derive(Debug, Serialize, Deserialize, FromForm)]
pub struct UserSettingsForm {
    pub account_ids: Vec<String>,
    pub start_date_type: String,
    pub start_date: Option<NaiveDateForm>,
    pub end_date_type: String,
    pub end_date: Option<NaiveDateForm>,
}

#[get("/settings?<err_msg>")]
pub async fn show_settings(
    err_msg: Option<String>,
    user_opt: Option<UserCookie>,
    state: &State<ServerState>,
) -> Result<Template, Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(uri!(
            ServerState::base(),
            super::retry_login_flash(
                redirect = Some("transactions".to_string()),
                err_msg = Some("Please log-in first.".to_string())
            )
        )));
    }
    let user = user_opt.unwrap();

    let db = state.postgres_db.clone();
    let user_accounts = user.get_accounts(db.clone()).await;
    let mut context = state.default_context();

    let settings = db.get_user_settings(user.userid).await;
    context.insert("selected_accounts", &settings.account_ids);
    context.insert("start_date_type", &settings.period_start.to_string());
    context.insert("end_date_type", &settings.period_end.to_string());
    if let Ok(start_date) = settings.period_start.date(None) {
        context.insert("start_date", &start_date.format("%Y-%m-%d").to_string());
    }
    if let Ok(end_date) = settings.period_end.date(None) {
        context.insert("end_date", &end_date.format("%Y-%m-%d").to_string());
    }

    context.insert("valid_accounts", &user_accounts);
    context.insert("user", &user);
    context.insert("err_msg", &err_msg);
    Ok(layout("user_settings", &context.into_json()))
}

#[post("/save_settings", data = "<form>")]
pub async fn save_settings(
    form: Form<UserSettingsForm>,
    user_opt: Option<UserCookie>,
    state: &State<ServerState>,
) -> Result<Redirect, Redirect> {
    let user = user_opt.ok_or_else(|| {
        Redirect::to(uri!(
            ServerState::base(),
            super::retry_login_flash(
                redirect = Some("save_settings".to_string()),
                err_msg = Some("Please log-in first.".to_string())
            )
        ))
    })?;

    let db = state.postgres_db.clone();
    let user_accounts = user.get_accounts(db.clone()).await.ok_or_else(|| {
        Redirect::to(uri!(
            ServerState::base(),
            show_settings(Some("No user account found".to_string()))
        ))
    })?;

    let mut all_user_account_ids = HashSet::new();
    for account in user_accounts {
        if let Some(id) = account.id {
            all_user_account_ids.insert(id);
        }
    }

    let filter_form = form.into_inner();
    let mut selected_accounts = HashSet::new();
    for account in filter_form.account_ids {
        if let Ok(id) = i32::from_str(&account) {
            selected_accounts.insert(id);
        }
    }
    let account_ids: Vec<_> = all_user_account_ids
        .intersection(&selected_accounts)
        .cloned()
        .collect();
    if account_ids.is_empty() {
        return Err(Redirect::to(uri!(
            ServerState::base(),
            show_settings(Some("List of account ids is empty".to_string()))
        )));
    }
    let period_start = if let Ok(date) = PeriodDate::new(
        &filter_form.start_date_type,
        filter_form.start_date.map(|d| d.date),
    ) {
        date
    } else {
        return Err(Redirect::to(uri!(
            ServerState::base(),
            show_settings(Some("Invalid start date".to_string()))
        )));
    };
    let period_end = if let Ok(date) = PeriodDate::new(
        &filter_form.end_date_type,
        filter_form.end_date.map(|d| d.date),
    ) {
        date
    } else {
        return Err(Redirect::to(uri!(
            ServerState::base(),
            show_settings(Some("Invalid end date".to_string()))
        )));
    };
    let user_settings = UserSettings {
        period_start,
        period_end,
        account_ids,
    };

    let result = db.set_user_settings(user.userid, &user_settings).await;
    match result {
        Ok(()) => Ok(Redirect::to(uri!(
            ServerState::base(),
            crate::position::position()
        ))),
        Err(_) => Err(Redirect::to(uri!(
            ServerState::base(),
            show_settings(Some("Failed to save user settings".to_string()))
        ))),
    }
}

use std::str::FromStr;

use rocket::State;
use rocket::response::Redirect;
use rocket_dyn_templates::Template;

use finql_data::Currency;
use qualinvest_core::user::UserHandler;
use qualinvest_core::position::calculate_position_for_period_for_accounts;
use crate::user::UserCookie;
use crate::layout::layout;
use super::{rocket_uri_macro_login};
use super::ServerState;

#[get("/position")]
pub async fn position(user_opt: Option<UserCookie>, 
    state: &State<ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(uri!(ServerState::base(), login(Some("position")))));
    }
    let user = user_opt.unwrap();

    let currency = Currency::from_str("EUR").unwrap();
    let db = state.postgres_db.clone();
    let user_settings = db.get_user_settings(user.userid).await;
    let period_end = user_settings.period_end.unwrap_or_else(|| chrono::Utc::now().naive_local().date());
    let period_start = user_settings.period_start.unwrap_or(period_end);

    let mut context = state.default_context();

    if let Ok((position, totals)) = calculate_position_for_period_for_accounts(currency, 
        &user_settings.account_ids, period_start, period_end, db).await {
        context.insert("positions", &position);
        context.insert("totals", &totals);
    } else {
        context.insert("err_msg", "Calculation of position failed");
    }

    context.insert("user", &user);
    Ok(layout("position", &context.into_json()))
}

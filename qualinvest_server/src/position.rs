use std::str::FromStr;

use rocket::State;
use rocket::response::Redirect;
use rocket_dyn_templates::Template;

use finql_data::Currency;
use qualinvest_core::user::UserHandler;
use qualinvest_core::position::calculate_position_for_period_for_accounts;
use crate::user::UserCookie;
use crate::layout::layout;
use super::{rocket_uri_macro_login,rocket_uri_macro_error_msg};
use super::ServerState;

#[get("/position")]
pub async fn position(user_opt: Option<UserCookie>, 
    state: &State<ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(login(redirect=Some("position"))))));
    }
    let user = user_opt.unwrap();

    let currency = Currency::from_str("EUR").unwrap();
    let db = state.postgres_db.clone();
    let user_settings = db.get_user_settings(user.userid).await;
    let period_end = user_settings.period_end.unwrap_or(chrono::Utc::now().naive_local().date());
    let period_start = user_settings.period_start.unwrap_or(period_end);

    println!("Calculate position as of {} with PnL since {} for account ids {:?}", period_end, period_start, user_settings.account_ids);
    let (position, totals) = calculate_position_for_period_for_accounts(currency, 
        &user_settings.account_ids, period_start, period_end, db).await
        .map_err(|e| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg=format!("Calculation of position failed: {:?}",e))))))?;

    let mut context = state.default_context();
    context.insert("positions", &position);
    context.insert("totals", &totals);
    context.insert("user", &user);
    Ok(layout("position", &context.into_json()))
}

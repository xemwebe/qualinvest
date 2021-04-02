use std::str::FromStr;

use rocket::State;
use rocket::response::Redirect;
use rocket_contrib::templates::Template;

use finql_data::Currency;
use crate::user::UserCookie;
use crate::layout::layout;
use crate::filter;
use qualinvest_core::position::calculate_position_for_period;
use super::{rocket_uri_macro_login,rocket_uri_macro_error_msg};
use super::ServerState;

#[get("/position?<accounts>&<start>&<end>")]
pub async fn position(accounts: Option<String>, start: Option<String>, end: Option<String>, user_opt: Option<UserCookie>, 
    state: State<'_,ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to( uri!(login: redirect=Some("position"))));
    }
    let user = user_opt.unwrap();

    let currency = Currency::from_str("EUR").unwrap();
    let db = state.postgres_db.clone();
    let user_accounts = user.get_accounts(db.clone()).await;
    if user_accounts.is_none() {
        return Err(Redirect::to(uri!(error_msg: msg="No user account found")));
    }
    let user_accounts = user_accounts.unwrap();

    let filter = filter::FilterForm::from_query(accounts, start, end, &user, &user_accounts, db.clone()).await?;
    let (position, totals) = calculate_position_for_period(currency, 
        &filter.account_ids, filter.start_date.date, filter.end_date.date, db).await
        .map_err(|e| Redirect::to(uri!(error_msg: msg=format!("Calculation of position failed: {:?}",e))))?;

    let mut context = state.default_context();
    context.insert("positions", &position);
    context.insert("totals", &totals);
    context.insert("user", &user);
    context.insert("valid_accounts", &user_accounts);
    context.insert("filter", &filter);
    Ok(layout("position", &context.into_json()))
}

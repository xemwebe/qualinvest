use std::str::FromStr;
use std::ops::DerefMut;

use rocket::State;
use rocket::response::Redirect;
use rocket_contrib::templates::Template;

use finql::Currency;
use finql::postgres_handler::PostgresDB;
use crate::user::UserCookie;
use super::{QlInvestDbConn,ServerState};
use crate::layout::layout;
use crate::filter;
use qualinvest_core::position::calculate_position_for_period;
use super::{rocket_uri_macro_login,rocket_uri_macro_error_msg};

#[get("/position?<accounts>&<start>&<end>")]
pub fn position(accounts: Option<String>, start: Option<String>, end: Option<String>, user_opt: Option<UserCookie>, mut qldb: QlInvestDbConn, state: State<ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(login: redirect="position"))));
    }
    let user = user_opt.unwrap();

    let currency = Currency::from_str("EUR").unwrap();
    let mut db = PostgresDB{ conn: qldb.0.deref_mut() };
    let user_accounts = user.get_accounts(&mut db);
    if user_accounts.is_none() {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="No user account found"))));
    }
    let user_accounts = user_accounts.unwrap();

    let filter = filter::FilterForm::from_query(accounts, start, end, &user, &user_accounts, &mut db)?;
    let (position, totals) = calculate_position_for_period(currency, &filter.account_ids, filter.start_date, filter.end_date, &mut db)
        .map_err(|e| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg=format!("Calculation of position failed: {:?}",e)))))?;

    let mut context = state.default_context();
    context.insert("positions", &position);
    context.insert("totals", &totals);
    context.insert("user", &user);
    context.insert("valid_accounts", &user_accounts);
    context.insert("filter", &filter);
    Ok(layout("position", &context.into_json()))
}

use std::str::FromStr;
use std::ops::DerefMut;
use chrono::{DateTime,Local};

use rocket::State;
use rocket::response::Redirect;
use rocket_contrib::templates::Template;

use finql::Currency;
use finql::postgres_handler::PostgresDB;
use qualinvest_core::position::{calc_position};
use qualinvest_core::accounts::AccountHandler;
use crate::user::UserCookie;
use super::{QlInvestDbConn,ServerState};
use crate::layout::layout;
use crate::filter;

#[get("/position?<accounts>&<start>&<end>")]
pub fn position(accounts: Option<String>, start: Option<String>, end: Option<String>, user_opt: Option<UserCookie>, mut qldb: QlInvestDbConn, state: State<ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to("/login?redirect=position"));
    }
    let user = user_opt.unwrap();

    let currency = Currency::from_str("EUR").unwrap();
    let mut db = PostgresDB{ conn: qldb.0.deref_mut() };
    let user_accounts = user.get_accounts(&mut db);
    if user_accounts.is_none() {
        return Err(Redirect::to("/err/no_user_account_found"));
    }
    let user_accounts = user_accounts.unwrap();

    let filter = filter::FilterForm::from_query(accounts, start, end, &user, &user_accounts, &mut db)?;

    let transactions = db.get_all_transactions_with_accounts(&filter.account_ids)
        .map_err(|_| Redirect::to("/err/get_all_transactions_with_accounts"))?;
    let mut position = calc_position(currency, &transactions)
        .map_err(|_| Redirect::to("/err/calc_positions"))?;
    position.get_asset_names(&mut db).unwrap();
    let time = DateTime::from(Local::now());
    position.add_quote(time, &mut db).unwrap();
    let totals = position.calc_totals();

    let mut context = state.default_context();
    context.insert("positions", &position);
    context.insert("totals", &totals);
    context.insert("user", &user);
    context.insert("valid_accounts", &user_accounts);
    context.insert("filter", &filter);
    Ok(layout("position", &context.into_json()))
}

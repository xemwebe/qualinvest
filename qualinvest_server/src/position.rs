use std::str::FromStr;
use std::ops::DerefMut;
use chrono::{DateTime,Local};

use rocket::State;
use rocket::response::Redirect;
use rocket_contrib::json::Json;
use rocket_contrib::templates::Template;

use finql::Currency;
use finql::data_handler::TransactionHandler;
use finql::postgres_handler::PostgresDB;
use qualinvest_core::position::{calc_position,PortfolioPosition};
use qualinvest_core::accounts::AccountHandler;
use qualinvest_core::user::UserHandler;
use qualinvest_core::Config;
use crate::helper;
use crate::user::UserCookie;
use super::{default_context, QlInvestDbConn};
use crate::layout::layout;

#[get("/raw_position?<account>")]
pub fn raw_position(user_opt: Option<UserCookie>, account: Option<usize>, mut qldb: QlInvestDbConn) -> Result<Json<PortfolioPosition>,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to("/"));
    }
    let currency = Currency::from_str("EUR").unwrap();
    let mut db = PostgresDB{ conn: qldb.0.deref_mut() };
    let transactions = match account {
        Some(account_id) => db
            .get_all_transactions_with_account(account_id)
            .unwrap(),
        None => db.get_all_transactions().unwrap(),
    };
    let mut position = calc_position(currency, &transactions).unwrap();
    position.get_asset_names(&mut db).unwrap();
    
    let time = DateTime::from(Local::now());
    position.add_quote(time, &mut db).unwrap();
    
    Ok(Json(position))
}

#[get("/position?<accounts>")]
pub fn position(accounts: Option<String>, user_opt: Option<UserCookie>, mut qldb: QlInvestDbConn, state: State<Config>) -> Result<Template,Redirect> {
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
    
    let selected_accounts =
        if let Some(accounts) = accounts {
            let accounts = helper::parse_ids(&accounts);
            if user.is_admin {
                accounts
            } else {
                db.valid_accounts(user.userid, &accounts)
                    .map_err(|_| Redirect::to("/err/valid_accounts"))?
            }
        } else {
            let mut account_ids = Vec::new();
            for account in &user_accounts {
                account_ids.push(account.id.unwrap());
            }
            account_ids
        };

    let transactions = db.get_all_transactions_with_accounts(&selected_accounts)
        .map_err(|_| Redirect::to("/err/get_all_transactions_with_accounts"))?;
    let mut position = calc_position(currency, &transactions)
        .map_err(|_| Redirect::to("/err/calc_positions"))?;
    position.get_asset_names(&mut db).unwrap();
    let time = DateTime::from(Local::now());
    position.add_quote(time, &mut db).unwrap();
    let totals = position.calc_totals();

    let mut context = default_context(&state);
    context.insert("positions", &position);
    context.insert("totals", &totals);
    context.insert("user", &user);
    context.insert("selected_accounts", &selected_accounts);
    context.insert("valid_accounts", &user_accounts);
    Ok(layout("position", &context.into_json()))
}

use std::ops::DerefMut;

use rocket::State;
use rocket::response::Redirect;
use rocket_contrib::templates::Template;

use finql::postgres_handler::PostgresDB;
use qualinvest_core::accounts::AccountHandler;
use qualinvest_core::user::UserHandler;
use qualinvest_core::Config;
use crate::helper;
use crate::user::UserCookie;
use super::{default_context, QlInvestDbConn};
use crate::layout::layout;

#[get("/transactions?<accounts>")]
pub fn transactions(accounts: Option<String>, user_opt: Option<UserCookie>, mut qldb: QlInvestDbConn, state: State<Config>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to("/login?redirect=transactions"));
    }

    let user = user_opt.unwrap();
    let mut db = PostgresDB{ conn: qldb.0.deref_mut() };

    let user_accounts;
    if user.is_admin {
        user_accounts = db.get_all_accounts()
            .map_err(|_| Redirect::to("/err/get_all_account_ids"))?;
    } else {
        user_accounts = db.get_user_accounts(user.userid)
            .map_err(|_| Redirect::to("/err/get_user_accounts"))?;
    }
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

    let transactions = db.get_transaction_view_for_accounts(&selected_accounts)
        .map_err(|_| Redirect::to("/err/get_transaction_view_for_accounts"))?;

    let mut context = default_context(&state);
    context.insert("transactions", &transactions);
    context.insert("user", &user);
    context.insert("selected_accounts", &selected_accounts);
    context.insert("valid_accounts", &user_accounts);
    Ok(layout("transactions", &context.into_json()))
}

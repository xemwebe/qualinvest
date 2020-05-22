/// Viewing and analyzing assets

use std::ops::DerefMut;

use rocket::State;
use rocket::response::Redirect;
use rocket_contrib::templates::Template;

use finql::postgres_handler::PostgresDB;
use finql::data_handler::asset_handler::AssetHandler;
use finql::data_handler::quote_handler::QuoteHandler;
use qualinvest_core::accounts::AccountHandler;

use super::rocket_uri_macro_login;
use super::rocket_uri_macro_error_msg;
use crate::user::UserCookie;
use super::{QlInvestDbConn,ServerState};
use crate::layout::layout;


#[get("/asset?<asset_id>")]
pub fn analyze_asset(asset_id: Option<usize>, user_opt: Option<UserCookie>, mut qldb: QlInvestDbConn, state: State<ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(login: redirect="asset"))));
    }
    let user = user_opt.unwrap();

    let mut db = PostgresDB{ conn: qldb.0.deref_mut() };
    let assets = db.get_all_assets()
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="found no assets"))))?;

    let user_accounts = user.get_accounts(&mut db);

    let mut context = state.default_context();
    context.insert("assets", &assets);
    context.insert("asset_id", &asset_id);
    context.insert("user", &user);
    
    if let Some(asset_id) = asset_id {
        let ticker = db.get_all_ticker_for_asset(asset_id)
            .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="found no ticker"))))?;
        let mut all_quotes = Vec::new();
        for t in ticker {
            let mut quotes = db.get_all_quotes_for_ticker(t.id.unwrap())
                .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="found no quotes"))))?;
            all_quotes.append(&mut quotes);
        }
        context.insert("quotes", &all_quotes);
        if let Some(user_accounts) = user_accounts {
            let mut account_ids = Vec::new();
            for a in user_accounts {
                account_ids.push(a.id.unwrap());
            }
            let transactions = db.get_transaction_view_for_accounts_and_asset(&account_ids, asset_id)
            .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="found no quotes"))))?;
            context.insert("transactions", &transactions);
        }
    }
    Ok(layout("analyzeAsset", &context.into_json()))
}

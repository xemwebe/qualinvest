/// Viewing and analyzing assets

use rocket::State;
use rocket::response::Redirect;
use rocket_dyn_templates::Template;
use super::rocket_uri_macro_error_msg;
use super::rocket_uri_macro_login;

use finql_data::{AssetHandler, QuoteHandler};
use qualinvest_core::accounts::AccountHandler;

use crate::user::UserCookie;
use crate::layout::layout;
use super::ServerState;

#[get("/asset?<asset_id>")]
pub async fn analyze_asset(asset_id: Option<usize>, user_opt: Option<UserCookie>, state: &State<ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(uri!(login(redirect=Some("asset")))));
    }
    let user = user_opt.unwrap();

    let db = state.postgres_db.clone();
    let assets = db.get_all_assets().await
        .map_err(|_| Redirect::to(uri!(error_msg(msg="getting assets list failed".to_string()))))?;

    let user_accounts = user.get_accounts(db.clone()).await;

    let mut context = state.default_context();
    context.insert("assets", &assets);
    context.insert("asset_id", &asset_id);
    context.insert("user", &user);
    
    if let Some(asset_id) = asset_id {
        let ticker = db.get_all_ticker_for_asset(asset_id).await
            .map_err(|e| Redirect::to(uri!(error_msg(msg=format!("getting ticker failed: {}",e)))))?;
        let mut all_quotes = Vec::new();
        for t in ticker {
            let mut quotes = db.get_all_quotes_for_ticker(t.id.unwrap()).await
                .map_err(|_| Redirect::to(uri!(error_msg(msg="getting quotes failed".to_string()))))?;
            all_quotes.append(&mut quotes);
        }
        context.insert("quotes", &all_quotes);
        if let Some(user_accounts) = user_accounts {
            let mut account_ids = Vec::new();
            for a in user_accounts {
                account_ids.push(a.id.unwrap());
            }
            let transactions = db.get_transaction_view_for_accounts_and_asset(&account_ids, asset_id).await
            .map_err(|e| Redirect::to(uri!(error_msg(msg=format!("building transactions view failed: {}", e)))))?;
            context.insert("transactions", &transactions);
        }
    }
    Ok(layout("analyzeAsset", &context.into_json()))
}

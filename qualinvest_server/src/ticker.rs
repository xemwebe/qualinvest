/// Viewing and editing market quote ticker

use rocket::State;
use rocket::response::Redirect;
use rocket_dyn_templates::Template;
use rocket::form::{Form, FromForm};
use super::rocket_uri_macro_error_msg;
use super::rocket_uri_macro_login;

use finql_data::{AssetHandler, QuoteHandler};
use qualinvest_core::accounts::AccountHandler;

use crate::user::UserCookie;
use crate::layout::layout;
use super::ServerState;

#[get("/asset/ticker?<asset_id>")]
pub async fn show_ticker(asset_id: Option<usize>, user_opt: Option<UserCookie>, state: &State<ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(login(redirect=Some("asset"))))));
    }
    let user = user_opt.unwrap();

    let db = state.postgres_db.clone();
    let assets = db.get_all_assets().await
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="getting assets list failed".to_string())))))?;

    let user_accounts = user.get_accounts(db.clone()).await;

    let mut context = state.default_context();
    context.insert("assets", &assets);
    context.insert("asset_id", &asset_id);
    context.insert("user", &user);

    Ok(layout("tickers", &context.into_json()))
}

#[get("/asset/ticker/edit?<asset_id>")]
pub async fn edit_ticker(asset_id: Option<usize>, user_opt: Option<UserCookie>, state: &State<ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(login(redirect=Some("asset"))))));
    }
    let user = user_opt.unwrap();

    let db = state.postgres_db.clone();
    let assets = db.get_all_assets().await
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="getting assets list failed".to_string())))))?;

    let user_accounts = user.get_accounts(db.clone()).await;

    let mut context = state.default_context();
    context.insert("assets", &assets);
    context.insert("asset_id", &asset_id);
    context.insert("user", &user);

    Ok(layout("ticker_form", &context.into_json()))
}

#[post("/asset/ticker/save?<asset_id>")]
pub async fn save_ticker(asset_id: Option<usize>, user_opt: Option<UserCookie>, state: &State<ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(login(redirect=Some("asset"))))));
    }
    let user = user_opt.unwrap();

    let db = state.postgres_db.clone();
    let assets = db.get_all_assets().await
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="getting assets list failed".to_string())))))?;

    let user_accounts = user.get_accounts(db.clone()).await;

    let mut context = state.default_context();
    context.insert("assets", &assets);
    context.insert("asset_id", &asset_id);
    context.insert("user", &user);

    Ok(layout("tickers", &context.into_json()))
}

/// Viewing and editing market quote ticker
use std::str::FromStr;
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

#[get("/tickers?<asset_id>")]
pub async fn show_ticker(asset_id: usize, user: UserCookie, state: &State<ServerState>) -> Result<Template,Redirect> {
    let db = state.postgres_db.clone();
    let asset = db.get_asset_by_id(asset_id).await
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="Invalid asset id".to_string())))))?;
    
    let tickers = db.get_all_ticker_for_asset(asset_id).await
        .map_err(|_| Redirect::to(format!("{}{}", 
        state.rel_path, uri!(error_msg(msg="failed to get list of tickers".to_string())))))?;

    let mut context = state.default_context();
    context.insert("tickers", &tickers);
    context.insert("asset_name", &asset.name);
    context.insert("asset_id", &asset_id);
    context.insert("user", &user);

    Ok(layout("tickers", &context.into_json()))
}

#[get("/ticker/edit?<asset_id>&<ticker_id>")]
pub async fn edit_ticker(asset_id: usize, ticker_id: Option<usize>, user: UserCookie, state: &State<ServerState>) -> Result<Template,Redirect> {
    if !user.is_admin {
        return Err(Redirect::to(uri!(error_msg(msg="Admin rights are required to add quotes"))));
    }

    let db = state.postgres_db.clone();
    let asset = db.get_asset_by_id(asset_id).await
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="Invalid asset ID".to_string())))))?;

    let ticker = if let Some(ticker_id) = ticker_id {
        db.get_ticker_by_id(ticker_id).await
            .map_err(|_| Redirect::to(format!("{}{}", 
            state.rel_path, uri!(error_msg(msg="Invalid ticker ID".to_string())))))?
    } else {
        finql_data::Ticker{
            id: None,
            asset: asset_id,
            name: "".to_string(),
            currency: finql_data::Currency::from_str("EUR").unwrap(),
            source: "".to_string(),
            priority: 10,
            factor: 1.0
        }
    };

    let sources = finql::market_quotes::MarketDataSource::extern_sources();

    let mut context = state.default_context();
    context.insert("asset_name", &asset.name);
    context.insert("asset_id", &asset_id);
    context.insert("ticker", &ticker);
    context.insert("sources", &sources);
    context.insert("user", &user);

    Ok(layout("ticker_form", &context.into_json()))
}


#[get("/ticker/delete?<ticker_id>&<asset_id>")]
pub async fn delete_ticker(ticker_id: usize, asset_id: usize, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {
    let db = state.postgres_db.clone();
    if !user.is_admin {
        return Err(Redirect::to(uri!(error_msg(msg="Admin rights are required to delete ticker"))));
    }
    db.delete_ticker(ticker_id).await
        .map_err(|_| Redirect::to(uri!(error_msg(msg="Deleting of ticker failed."))))?;
    Ok(Redirect::to(format!("{}/tickers?asset_id={}", state.rel_path, asset_id)))
}

/// Structure for storing information in ticker form
#[derive(Debug,Serialize,Deserialize,FromForm)]
pub struct TickerForm {
    pub ticker_id: Option<usize>,
    pub asset_id: usize,
    pub name: String,
    pub source: String,
    pub priority: i32,
    pub currency: String,
    pub factor: f64,
}

#[post("/ticker/edit", data="<form>")]
pub async fn save_ticker(form: Form<TickerForm>, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {
    if !user.is_admin {
        return Err(Redirect::to(uri!(error_msg(msg="Admin rights are required to add quotes"))));
    }

    let ticker_form = form.into_inner();
    let db = state.postgres_db.clone();
    // Try to get asset just to make sure it does exist
    let _asset = db.get_asset_by_id(ticker_form.asset_id).await
        .map_err(|_| Redirect::to(format!("{}{}", 
        state.rel_path, uri!(error_msg(msg="Asset does not seem to extist.".to_string())))))?;
    
    let currency = finql_data::Currency::from_str(&ticker_form.currency)
        .map_err(|_| Redirect::to(format!("{}{}", 
        state.rel_path, uri!(error_msg(msg="Invalid currency".to_string())))))?;

    let ticker = finql_data::Ticker{
        id: ticker_form.ticker_id,
        asset: ticker_form.asset_id,
        name: ticker_form.name,
        currency,
        source: ticker_form.source,
        priority: ticker_form.priority,
        factor: ticker_form.factor,
    };

    if ticker.id.is_none()  {
        let _ticker_id = db.insert_if_new_ticker(&ticker).await
            .map_err(|_| Redirect::to(format!("{}{}", 
            state.rel_path, uri!(error_msg(msg="failed to store ticker in database.".to_string())))))?;    
    } else {
        db.update_ticker(&ticker).await
            .map_err(|_| Redirect::to(format!("{}{}", 
            state.rel_path, uri!(error_msg(msg="failed to store ticker in database.".to_string())))))?;
    }

    Ok(Redirect::to(format!("{}/tickers?asset_id={}", state.rel_path, ticker_form.asset_id)))
}

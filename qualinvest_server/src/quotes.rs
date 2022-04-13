/// Viewing and editing quotes
use std::{collections::BTreeMap, str::FromStr};
use futures::{future::join_all};
use rocket::State;
use rocket::response::Redirect;
use rocket_dyn_templates::Template;
use rocket::form::{Form, FromForm};
use super::rocket_uri_macro_error_msg;

use finql::datatypes::{AssetHandler, Quote, QuoteHandler, date_time_helper::date_time_from_str_standard};

use crate::user::UserCookie;
use crate::layout::layout;
use super::ServerState;

#[derive(Serialize)]
struct QuoteView {
    quote: Quote,
    currency: String,
    ticker_name: String,
    ticker_source: String,
}

#[get("/quotes?<asset_id>&<err_msg>")]
pub async fn show_quotes(asset_id: i32, err_msg: Option<String>, user: UserCookie, state: &State<ServerState>) -> Result<Template,Redirect> {
    let db = state.postgres_db.clone();
    let f_asset = db.get_asset_by_id(asset_id);
    let f_tickers = db.get_all_ticker_for_asset(asset_id);
    let (asset, tickers) = futures::join!(f_asset, f_tickers);
    let mut f_quotes = Vec::new();
    let asset = asset.map_err(|_| 
        Redirect::to(uri!(ServerState::base(), crate::asset::analyze_asset(Some(asset_id), Some("Invalid asset.")))))?;
    let tickers = tickers.map_err(|_| 
        Redirect::to(uri!(ServerState::base(), crate::asset::analyze_asset(Some(asset_id), Some("Could not get ticker for asset.")))))?;
    for ticker in &tickers {
        if let Some(ticker_id) = ticker.id {
            f_quotes.push(db.get_all_quotes_for_ticker(ticker_id));
        }
    }
    let all_quotes = join_all(f_quotes).await;
    let mut quotes = Vec::new();
    for q in all_quotes.into_iter().flatten() {
        quotes.extend(q);
    }
    quotes.sort_by(|a,b|b.cmp(a));

    let mut ticker_map = BTreeMap::new();
    for ticker in &tickers {
        ticker_map.insert(ticker.id.unwrap(), ticker);
    }

    let mut quotes_view = Vec::new();
    for q in quotes {
        if let Some(ticker) = ticker_map.get(&q.ticker) {
            quotes_view.push(QuoteView{
                currency: format!("{}", ticker.currency),
                ticker_name: ticker.name.clone(),
                ticker_source: ticker.source.clone(),
                quote: q, 
            });
        } else {
            quotes_view.push(QuoteView{
                currency: String::new(),
                ticker_name: String::new(),
                ticker_source: String::new(),
                quote: q, 
            });
        }
    }
    let mut context = state.default_context();
    context.insert("quotes", &quotes_view);
    context.insert("asset_id", &asset_id);
    context.insert("asset_name", &asset.name());
    context.insert("tickers", &tickers);
    context.insert("user", &user);
    context.insert("err_msg", &err_msg);
    Ok(layout("quotes", &context.into_json()))
}


#[get("/quote/update?<asset_id>&<ticker_id>")]
pub async fn update_asset_quote(asset_id: i32, ticker_id: i32, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {
    if !user.is_admin {
        return  Err(Redirect::to(uri!(ServerState::base(), super::index(Some("You must be admin user to edit quotes!".to_string())))));
    }

    let db = state.postgres_db.clone();

    qualinvest_core::update_ticker(ticker_id, db, &state.market_data).await
        .map_err(|_| Redirect::to(uri!(ServerState::base(), show_quotes(asset_id, Some("Updating prices for ticker failed".to_string())))))?;

    Ok(Redirect::to(uri!(ServerState::base(), show_quotes(asset_id, Option::<String>::None))))
}

#[get("/quote/renew_history?<asset_id>&<ticker_id>&<start>&<end>")]
pub async fn renew_history(asset_id: i32, ticker_id: i32, 
    start: String, end: String, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {

    if !user.is_admin {
        return  Err(Redirect::to(uri!(ServerState::base(), show_quotes(asset_id, Some("You must be admin user to update quotes!".to_string())))));
    }

    let db = state.postgres_db.clone();
    let start_date = date_time_from_str_standard(&start, 0, None)
        .map_err(|_| Redirect::to(uri!(ServerState::base(), show_quotes(asset_id, Some("Invalid start date.".to_string())))))?;
    let end_date = date_time_from_str_standard(&end, 0, None)
        .map_err(|_| Redirect::to(uri!(ServerState::base(), show_quotes(asset_id, Some("Invalid end date.".to_string())))))?;

    qualinvest_core::update_quote_history(ticker_id, start_date, end_date, db, &state.market_data).await
        .map_err(|_| Redirect::to(uri!(ServerState::base(), show_quotes(asset_id, Some("Updating quote history failed.".to_string())))))?;

    Ok(Redirect::to(uri!(ServerState::base(), show_quotes(asset_id, Option::<String>::None))))
}


#[get("/quote/delete?<quote_id>&<asset_id>")]
pub async fn delete_quote(quote_id: i32, asset_id: i32, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {
    let db = state.postgres_db.clone();
    if !user.is_admin {
        return Err(Redirect::to(uri!(ServerState::base(), show_quotes(asset_id, Some("You must be admin user to update quote history!".to_string())))));
    }
    db.delete_quote(quote_id).await
        .map_err(|_| Redirect::to(uri!(ServerState::base(), show_quotes(asset_id, Some("Deleting of quote failed.")))))?;
    Ok(Redirect::to(uri!(ServerState::base(), show_quotes(asset_id, Option::<String>::None))))
}

#[get("/quote/new?<asset_id>")]
pub async fn new_quote(asset_id: i32, user: UserCookie, state: &State<ServerState>) -> Result<Template,Redirect> {
    if !user.is_admin {
        return Err(Redirect::to(uri!(ServerState::base(), show_quotes(asset_id, Some("You must be admin user to add new quotes!".to_string())))));
    }
    let db = state.postgres_db.clone();
    let asset = db.get_asset_by_id(asset_id).await
        .map_err(|_| 
            Redirect::to(uri!(ServerState::base(), show_quotes(asset_id, Some("Failed to get asset!".to_string())))))?;
    
    let mut context = state.default_context();
    context.insert("asset_id", &asset_id);
    context.insert("asset_name", &asset.name());

    Ok(layout("quote_form", &context.into_json()))
}

/// Structure for storing information in quote formular
#[derive(Debug,Serialize,Deserialize,FromForm)]
pub struct QuoteForm {
    pub asset_id: i32,
    pub date: String,
    pub hour: u32,
    pub quote: f64,
    pub currency: String,
}

#[post("/quote/new", data = "<form>")]
pub async fn add_new_quote(form: Form<QuoteForm>, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {
    if !user.is_admin {
        return Err(Redirect::to(uri!(error_msg(msg="Admin rights are required to add quotes"))));
    }

    let quote_form = form.into_inner();
    let db = state.postgres_db.clone();
    // Try to get asset just to make sure it does exist
    let _asset = db.get_asset_by_id(quote_form.asset_id).await
        .map_err(|_| Redirect::to(uri!(ServerState::base(), show_quotes(quote_form.asset_id, Some("Failed to get asset".to_string())))))?;
    
    let currency = finql::datatypes::Currency::from_str(&quote_form.currency)
        .map_err(|_| Redirect::to(uri!(ServerState::base(), show_quotes(quote_form.asset_id, Some("Invalid currency".to_string())))))?;
    
    let ticker = finql::datatypes::Ticker{
        id: None,
        asset: quote_form.asset_id,
        name: format!("asset_{}", quote_form.asset_id),
        currency,
        source: "manual".to_string(),
        priority: 1,
        factor: 1.0,
        tz: None,
        cal: None,
    };
    let ticker_id = db.insert_if_new_ticker(&ticker).await
        .map_err(|_| 
            Redirect::to(uri!(ServerState::base(), show_quotes(quote_form.asset_id, Some("Failed to store manual ticker".to_string())))))?;

    
    let date = date_time_from_str_standard(&quote_form.date, quote_form.hour, None)
        .map_err(|_| 
            Redirect::to(uri!(ServerState::base(), show_quotes(quote_form.asset_id, Some("Invalid date".to_string())))))?;

    let quote = Quote{
        id: None,
        ticker: ticker_id,
        price: quote_form.quote,
        time: date,
        volume: None,    
    };
    db.insert_quote(&quote).await
        .map_err(|_| Redirect::to(uri!(ServerState::base(), show_quotes(quote_form.asset_id, Some("Failed to store quote".to_string())))))?;
        
    Ok(Redirect::to(uri!(ServerState::base(), show_quotes(quote_form.asset_id, Option::<String>::None))))
}

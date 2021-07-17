/// Viewing and editing quotes
use std::{collections::BTreeMap, str::FromStr};
use futures::{future::join_all};
use rocket::State;
use rocket::response::Redirect;
use rocket_dyn_templates::Template;
use rocket::form::{Form, FromForm};
use super::rocket_uri_macro_error_msg;
use super::rocket_uri_macro_login;

use finql_data::{AssetHandler, Quote, QuoteHandler, date_time_helper::date_time_from_str_standard};

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

#[get("/quotes?<asset_id>")]
pub async fn show_quotes(asset_id: usize, user_opt: Option<UserCookie>, state: &State<ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(login(redirect=Some("asset"))))));
    }
    let user = user_opt.unwrap();

    let db = state.postgres_db.clone();
    let f_asset = db.get_asset_by_id(asset_id);
    let f_tickers = db.get_all_ticker_for_asset(asset_id);
    let (asset, tickers) = futures::join!(f_asset, f_tickers);
    let mut f_quotes = Vec::new();
    let asset = asset.map_err(|_| Redirect::to(format!("{}{}", 
        state.rel_path, uri!(error_msg(msg="failed to get asset name".to_string())))))?;
    let tickers = tickers.map_err(|_| Redirect::to(format!("{}{}", 
        state.rel_path, uri!(error_msg(msg="failed to get list of tickers".to_string())))))?;
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
    context.insert("asset_name", &asset.name);
    context.insert("tickers", &tickers);
    context.insert("user", &user);

    Ok(layout("quotes", &context.into_json()))
}


#[get("/quote/update?<asset_id>&<ticker_id>")]
pub async fn update_asset_quote(asset_id: usize, ticker_id: usize, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {
    if !user.is_admin {
        return  Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="You must be admin user to edit assets!")))));
    }

    let db = state.postgres_db.clone();

    qualinvest_core::update_ticker(ticker_id, db, &state.market_data).await.unwrap();

    Ok(Redirect::to(format!("{}/quotes?asset_id={}",state.rel_path, asset_id)))
}

#[get("/quote/renew_history?<asset_id>&<ticker_id>&<start>&<end>")]
pub async fn renew_history(asset_id: usize, ticker_id: usize, 
    start: String, end: String, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {

    if !user.is_admin {
        return  Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="You must be admin user to edit assets!")))));
    }

    let db = state.postgres_db.clone();
    let start_date = date_time_from_str_standard(&start, 0)
        .map_err(|_| Redirect::to(uri!(error_msg(msg="Invalid start date."))))?;
    let end_date = date_time_from_str_standard(&end, 0)
        .map_err(|_| Redirect::to(uri!(error_msg(msg="Invalid end date."))))?;

    qualinvest_core::update_quote_history(ticker_id, start_date, end_date, db, &state.market_data).await
        .map_err(|_| Redirect::to(uri!(error_msg(msg="Updating quote history failedcl."))))?;

    Ok(Redirect::to(format!("{}/quotes?asset_id={}",state.rel_path, asset_id)))
}


#[get("/quote/delete?<quote_id>&<asset_id>")]
pub async fn delete_quote(quote_id: usize, asset_id: usize, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {
    let db = state.postgres_db.clone();
    if !user.is_admin {
        return Err(Redirect::to(uri!(error_msg(msg="Admin rights are required to delete quotes"))));
    }
    db.delete_quote(quote_id).await
        .map_err(|_| Redirect::to(uri!(error_msg(msg="Deleting of quote failed."))))?;
    Ok(Redirect::to(format!("{}/quotes?asset_id={}", state.rel_path, asset_id)))
}

#[get("/quote/new?<asset_id>")]
pub async fn new_quote(asset_id: usize, user: UserCookie, state: &State<ServerState>) -> Result<Template,Redirect> {
    if !user.is_admin {
        return Err(Redirect::to(uri!(error_msg(msg="Admin rights are required to add quotes"))));
    }
    let db = state.postgres_db.clone();
    let asset = db.get_asset_by_id(asset_id).await
        .map_err(|_| Redirect::to(format!("{}{}", 
        state.rel_path, uri!(error_msg(msg="failed to get asset name".to_string())))))?;
    
    let mut context = state.default_context();
    context.insert("asset_id", &asset_id);
    context.insert("asset_name", &asset.name);

    Ok(layout("quote_form", &context.into_json()))
}

/// Structure for storing information in quote formular
#[derive(Debug,Serialize,Deserialize,FromForm)]
pub struct QuoteForm {
    pub asset_id: usize,
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
        .map_err(|_| Redirect::to(format!("{}{}", 
        state.rel_path, uri!(error_msg(msg="failed to get asset".to_string())))))?;
    
    let currency = finql_data::Currency::from_str(&quote_form.currency)
        .map_err(|_| Redirect::to(format!("{}{}", 
        state.rel_path, uri!(error_msg(msg="Invalid currency".to_string())))))?;
    
    let ticker = finql_data::Ticker{
        id: None,
        asset: quote_form.asset_id,
        name: format!("asset_{}", quote_form.asset_id),
        currency,
        source: "manual".to_string(),
        priority: 1,
        factor: 1.0,
    };
    let ticker_id = db.insert_if_new_ticker(&ticker).await
        .map_err(|_| Redirect::to(format!("{}{}", 
        state.rel_path, uri!(error_msg(msg="Failed to store manual ticker".to_string())))))?;

    
    let date = date_time_from_str_standard(&quote_form.date, quote_form.hour)
        .map_err(|_| Redirect::to(format!("{}{}", 
        state.rel_path, uri!(error_msg(msg="Invalid date".to_string())))))?;
    let quote = Quote{
        id: None,
        ticker: ticker_id,
        price: quote_form.quote,
        time: date,
        volume: None,    
    };
    db.insert_quote(&quote).await
        .map_err(|_| Redirect::to(format!("{}{}", 
        state.rel_path, uri!(error_msg(msg="Failed to store quote".to_string())))))?;

    Ok(Redirect::to(format!("{}/quotes?asset_id={}", state.rel_path, quote_form.asset_id)))
}

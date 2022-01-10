/// Viewing and editing market quote ticker
use std::str::FromStr;
use rocket::State;
use rocket::response::Redirect;
use rocket_dyn_templates::Template;
use rocket::form::{Form, FromForm};

use finql_data::{AssetHandler, QuoteHandler};

use crate::user::UserCookie;
use crate::layout::layout;
use super::ServerState;

#[get("/tickers?<asset_id>&<err_msg>")]
pub async fn show_ticker(asset_id: usize, err_msg: Option<String>, user: UserCookie, state: &State<ServerState>) -> Template {
    let db = state.postgres_db.clone();
    let mut context = state.default_context();
    context.insert("asset_id", &asset_id);
    context.insert("user", &user);

    if let Ok(asset) = db.get_asset_by_id(asset_id).await {
        context.insert("asset_name", &asset.name);
    } else {
        context.insert("err_msg", "Invalid asset ID");
    }
   
    if let Ok(tickers) = db.get_all_ticker_for_asset(asset_id).await {
        context.insert("tickers", &tickers);
    } else {
        let _ = context.try_insert("err_msg", "Failed to get ticker list for asset id");
    }

    let _ = context.try_insert("err_msg", &err_msg);
    layout("tickers", &context.into_json())
}

#[get("/ticker/edit?<asset_id>&<ticker_id>&<err_msg>")]
pub async fn edit_ticker(asset_id: usize, ticker_id: Option<usize>, err_msg: Option<String>, user: UserCookie, state: &State<ServerState>) -> Result<Template,Redirect> {
    if !user.is_admin {
        return Err(Redirect::to(uri!(ServerState::base(), super::index(Some("Admin rights are required to add quotes")))));
    }

    let db = state.postgres_db.clone();
    let mut context = state.default_context();
    context.insert("asset_id", &asset_id);
    context.insert("user", &user);

    if let Ok(asset) = db.get_asset_by_id(asset_id).await {
        context.insert("asset_name", &asset.name);
    } else {
        context.insert("err_msg", "Invalid asset ID");
    }

    if let Some(ticker_id) = ticker_id {
        if let Ok(ticker) = db.get_ticker_by_id(ticker_id).await {
            context.insert("ticker", &ticker);
        } else {
            context.insert("err_msg", "Invalid ticker ID");
        }
    }

    let sources = finql::market_quotes::MarketDataSource::extern_sources();
    context.insert("sources", &sources);

    let _ = context.try_insert("err_msg", &err_msg);
    Ok(layout("ticker_form", &context.into_json()))
}


#[get("/ticker/delete?<ticker_id>&<asset_id>")]
pub async fn delete_ticker(ticker_id: usize, asset_id: usize, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {
    let db = state.postgres_db.clone();
    if !user.is_admin {
        return Err(Redirect::to(uri!(ServerState::base(), super::index(Some("Admin rights are required to delete ticker")))));
    }
    let message = if db.delete_ticker(ticker_id).await.is_err() {
        Some("Deleting of ticker failed".to_string())
    } else {
        Option::<String>::None
    };
    Ok(Redirect::to(uri!(ServerState::base(), show_ticker(asset_id, message))))
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
        return Err(Redirect::to(uri!(ServerState::base(), super::index(Some("Admin rights are required to update or insert ticker")))));
    }

    let ticker_form = form.into_inner();
    let db = state.postgres_db.clone();
    // Try to get asset just to make sure it does exist
    if let Ok(_asset) = db.get_asset_by_id(ticker_form.asset_id).await {
        if let Ok(currency) = finql_data::Currency::from_str(&ticker_form.currency) {
            let ticker = finql_data::Ticker {
                id: ticker_form.ticker_id,
                asset: ticker_form.asset_id,
                name: ticker_form.name,
                currency,
                source: ticker_form.source,
                priority: ticker_form.priority,
                factor: ticker_form.factor,
                tz: None,
                cal: None,
            };
            if ticker.id.is_none()  {
                let ticker_id = db.insert_if_new_ticker(&ticker).await;
                if ticker_id.is_err() {
                    Err(Redirect::to(uri!(ServerState::base(), edit_ticker(ticker_form.asset_id, ticker_id, Some("Failed to store new ticker in database.")))))
                } else {
                    Ok(Redirect::to(uri!(ServerState::base(), crate::asset::assets(Option::<String>::None))))
                }
            } else if db.update_ticker(&ticker).await.is_err() {
                Err(Redirect::to(uri!(ServerState::base(), edit_ticker(ticker_form.asset_id, Option::<usize>::None, Some("Failed to store ticker in database.")))))
            } else {
                Ok(Redirect::to(uri!(ServerState::base(), crate::asset::assets(Option::<String>::None))))
            }
        } else {
            Err(Redirect::to(uri!(ServerState::base(), edit_ticker(ticker_form.asset_id, Option::<usize>::None, Some("Invalid currency!")))))
        }
        
    } else {
        Err(Redirect::to(uri!(ServerState::base(), crate::asset::assets(Some("Invalid asset id!")))))
    }

}

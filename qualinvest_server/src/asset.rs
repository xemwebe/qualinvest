use super::rocket_uri_macro_login;
use rocket::form::{Form, FromForm};
use rocket::response::Redirect;
/// Viewing and analyzing assets
use rocket::State;
use rocket_dyn_templates::Template;
use serde::{Deserialize, Serialize};

use finql::datatypes::{Asset, AssetHandler, DataItem, QuoteHandler, Stock};
use qualinvest_core::accounts::AccountHandler;

use super::ServerState;
use crate::layout::layout;
use crate::user::UserCookie;

/// Structure for storing information in asset formular
#[derive(Debug, Serialize, Deserialize, FromForm)]
pub struct AssetForm {
    pub id: Option<i32>,
    pub name: String,
    pub isin: Option<String>,
    pub wkn: Option<String>,
    pub note: Option<String>,
}

impl AssetForm {
    pub fn to_asset(&self) -> Asset {
        Asset::Stock(Stock::new(
            self.id,
            self.name.clone(),
            self.isin.clone(),
            self.wkn.clone(),
            self.note.clone(),
        ))
    }
}

#[derive(Serialize)]
pub struct Source {
    name: String,
    start_idx: usize,
}

#[get("/asset?<asset_id>&<message>")]
pub async fn analyze_asset(
    asset_id: Option<i32>,
    message: Option<String>,
    user_opt: Option<UserCookie>,
    state: &State<ServerState>,
) -> Result<Template, Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(uri!(
            ServerState::base(),
            login(Some("asset"))
        )));
    }
    let user = user_opt.unwrap();

    let mut message = message;
    let mut context = state.default_context();
    let db = state.postgres_db.clone();
    if let Ok(mut asset_list) = db.get_asset_list().await {
        asset_list.sort_by(|a, b| a.name.cmp(&b.name));
        context.insert("assets", &asset_list);
    } else {
        message = Some("No assets found!".to_string());
    }

    let user_accounts = user.get_accounts(db.clone()).await;

    context.insert("asset_id", &asset_id);
    context.insert("user", &user);

    if let Some(asset_id) = asset_id {
        let ticker = if let Ok(ticker) = db.get_all_ticker_for_asset(asset_id).await {
            ticker
        } else {
            message = Some("Failed to get ticker".to_string());
            Vec::new()
        };

        let mut all_quotes = Vec::new();
        let mut quote_idx = 0;
        let mut sources = Vec::new();
        for t in ticker {
            let mut quotes = if let Ok(quotes) = db.get_all_quotes_for_ticker(t.id.unwrap()).await {
                quotes
            } else {
                message = Some("Failed to get quotes".to_string());
                Vec::new()
            };
            if quotes.is_empty() {
                continue;
            }
            all_quotes.append(&mut quotes);
            sources.push(Source {
                name: format!("{}:{}", t.source, t.name),
                start_idx: quote_idx,
            });
            quote_idx = all_quotes.len();
        }
        context.insert("quotes", &all_quotes);
        context.insert("sources", &sources);
        if let Some(user_accounts) = user_accounts {
            let mut account_ids = Vec::new();
            for a in user_accounts {
                account_ids.push(a.id.unwrap());
            }
            let transactions = if let Ok(transactions) = db
                .get_transaction_view_for_accounts_and_asset(&account_ids, asset_id)
                .await
            {
                transactions
            } else {
                message = Some("Building transactions view failed".to_string());
                Vec::new()
            };
            context.insert("transactions", &transactions);
        }
    }
    context.insert("err_msg", &message);
    Ok(layout("analyzeAsset", &context.into_json()))
}

#[get("/asset/edit?<asset_id>&<asset_class>&<message>")]
pub async fn edit_asset(
    asset_id: Option<i32>,
    asset_class: Option<String>,
    message: Option<String>,
    user: UserCookie,
    state: &State<ServerState>,
) -> Result<Template, Redirect> {
    if !user.is_admin {
        return Err(Redirect::to(uri!(
            ServerState::base(),
            analyze_asset(
                asset_id,
                Some("You must be admin user to edit assets!".to_string())
            )
        )));
    }

    let db = state.postgres_db.clone();

    let mut context = state.default_context();
    context.insert("user", &user);
    context.insert("err_msg", &message);

    if let Some(asset_id) = asset_id {
        let asset = db.get_asset_by_id(asset_id).await.map_err(|e| {
            Redirect::to(uri!(
                ServerState::base(),
                analyze_asset(
                    Some(asset_id),
                    Some(format!("Couldn't get asset, error was {}", e))
                )
            ))
        })?;
        context.insert("asset_class", &asset.class());
        context.insert("asset_id", &Some(asset_id));
        match asset {
            Asset::Currency(c) => {
                context.insert("iso_code", &c.iso_code.to_string());
                context.insert("rounding_digits", &c.rounding_digits);
            }
            Asset::Stock(s) => {
                context.insert("stock", &s);
            }
        }
    } else {
        let asset_class = asset_class.unwrap_or_else(|| "new".to_string());
        match asset_class.as_ref() {
            "currency" => {
                context.insert("iso_code", "");
                context.insert("rounding_digits", &2);
                context.insert("asset_class", "currency");
            }
            "stock" => {
                context.insert("stock", &Stock::new(None, "".to_string(), None, None, None));
                context.insert("asset_class", "stock");
            }
            _ => {
                context.insert("asset_class", &"new");
            }
        }
        context.insert("asset_id", &Option::<i32>::None);
    };

    Ok(layout("asset_form", &context.into_json()))
}

#[get("/asset/delete?<asset_id>")]
pub async fn delete_asset(
    asset_id: i32,
    user: UserCookie,
    state: &State<ServerState>,
) -> Result<Redirect, Redirect> {
    if !user.is_admin {
        return Err(Redirect::to(uri!(
            ServerState::base(),
            super::index(Some("You must be admin user to delete assets!"))
        )));
    }

    state
        .postgres_db
        .delete_asset(asset_id)
        .await
        .map_err(|_| {
            Redirect::to(uri!(
                ServerState::base(),
                analyze_asset(Some(asset_id), Some("Failed to delete asset"))
            ))
        })?;

    Ok(Redirect::to(uri!(
        ServerState::base(),
        analyze_asset(Some(asset_id), Option::<String>::None)
    )))
}

#[post("/asset", data = "<form>")]
pub async fn save_asset(
    form: Form<AssetForm>,
    user: UserCookie,
    state: &State<ServerState>,
) -> Result<Redirect, Redirect> {
    if !user.is_admin {
        return Err(Redirect::to(uri!(
            ServerState::base(),
            super::index(Some("You must be admin user to edit assets!"))
        )));
    }

    let mut asset = form.into_inner().to_asset();
    let db = state.postgres_db.clone();

    let asset_id = db.get_asset_id(&asset).await;
    if let Some(id) = asset_id {
        asset.set_id(id).map_err(|e| {
            Redirect::to(uri!(
                ServerState::base(),
                edit_asset(
                    asset_id,
                    Option::<String>::None,
                    Some(format!("Updating asset failed, error was {}", e))
                )
            ))
        })?;
        db.update_asset(&asset).await.map_err(|e| {
            Redirect::to(uri!(
                ServerState::base(),
                edit_asset(
                    asset_id,
                    Option::<String>::None,
                    Some(format!("Updating asset failed, error was {}", e))
                )
            ))
        })?;
    } else {
        let _asset_id = db.insert_asset(&asset).await.map_err(|e| {
            Redirect::to(uri!(
                ServerState::base(),
                edit_asset(
                    asset_id,
                    Option::<String>::None,
                    Some(format!("Couldn't insert assert, error was {}", e))
                )
            ))
        })?;
    }

    Ok(Redirect::to(uri!(
        ServerState::base(),
        analyze_asset(Option::<i32>::None, Option::<String>::None)
    )))
}

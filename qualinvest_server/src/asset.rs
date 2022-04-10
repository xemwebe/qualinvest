/// Viewing and analyzing assets

use rocket::State;
use rocket::response::Redirect;
use rocket_dyn_templates::Template;
use rocket::form::{Form, FromForm};
use super::rocket_uri_macro_login;

use finql::datatypes::{Asset, Stock, AssetHandler, DataItem, QuoteHandler};
use qualinvest_core::accounts::AccountHandler;

use crate::user::UserCookie;
use crate::layout::layout;
use super::ServerState;

/// Structure for storing information in asset formular
#[derive(Debug,Serialize,Deserialize,FromForm)]
pub struct AssetForm {
    pub id: Option<i32>,
    pub name: String,
    pub isin: Option<String>,
    pub wkn: Option<String>, 
    pub note: Option<String>,
}

impl AssetForm {
    pub fn to_asset(&self) -> Asset {
        Asset::Stock(Stock::new(self.id, self.name.clone(), self.isin.clone(), self.wkn.clone(), self.note.clone()))
    }
}

#[get("/asset?<asset_id>")]
pub async fn analyze_asset(asset_id: Option<i32>, user_opt: Option<UserCookie>, state: &State<ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(uri!(ServerState::base(), login(Some("asset")))));
    }
    let user = user_opt.unwrap();

    let db = state.postgres_db.clone();
    let assets = db.get_all_assets().await
        .map_err(|_| Redirect::to(uri!(ServerState::base(), assets(Some("getting assets list failed")))))?;

    let user_accounts = user.get_accounts(db.clone()).await;

    let mut context = state.default_context();
    context.insert("assets", &assets);
    context.insert("asset_id", &asset_id);
    context.insert("user", &user);
    
    if let Some(asset_id) = asset_id {
        let ticker = db.get_all_ticker_for_asset(asset_id).await
            .map_err(|e| Redirect::to(uri!(ServerState::base(), assets(Some(format!("getting ticker failed: {}",e))))))?;
        let mut all_quotes = Vec::new();
        for t in ticker {
            let mut quotes = db.get_all_quotes_for_ticker(t.id.unwrap()).await
                .map_err(|_| Redirect::to(uri!(ServerState::base(), assets(Some("getting quotes failed")))))?;
            all_quotes.append(&mut quotes);
            context.insert(&t.source, &t.name);
        }
        context.insert("quotes", &all_quotes);
        if let Some(user_accounts) = user_accounts {
            let mut account_ids = Vec::new();
            for a in user_accounts {
                account_ids.push(a.id.unwrap());
            }
            let transactions = db.get_transaction_view_for_accounts_and_asset(&account_ids, asset_id).await
            .map_err(|e| Redirect::to(uri!(ServerState::base(), assets(Some(format!("building transactions view failed: {}", e))))))?;
            context.insert("transactions", &transactions);
        }
    }
    Ok(layout("analyzeAsset", &context.into_json()))
}

#[get("/assets?<message>")]
pub async fn assets(message: Option<String>, user_opt: Option<UserCookie>, state: &State<ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(uri!(ServerState::base(), login(Some("assets")))));
    }
    let user = user_opt.unwrap();

    let db = state.postgres_db.clone();

    let assets = db.get_all_assets().await
        .map_err(|e| Redirect::to(uri!(ServerState::base(), super::index(Some(format!("Couldn't get asset list, error was {}", e))))))?;

    let mut context = state.default_context();
    context.insert("err_msg", &message);
    context.insert("assets", &assets);
    context.insert("user", &user);
    Ok(layout("assets", &context.into_json()))
}


#[get("/asset/edit?<asset_id>&<message>")]
pub async fn edit_asset(asset_id: Option<i32>, message: Option<String>, user: UserCookie, state: &State<ServerState>) -> Result<Template,Redirect> {
    if !user.is_admin {
        return  Err(Redirect::to(uri!(ServerState::base(), assets(Some("You must be admin user to edit assets!")))));
    }

    let db = state.postgres_db.clone();

    let asset = if let Some(asset_id) = asset_id {
        db.get_asset_by_id(asset_id).await
            .map_err(|e| Redirect::to(uri!(ServerState::base(), assets(Some(format!("Couldn't get asset, error was {}", e))))))?
    } else {
        Asset::Stock(Stock::new(None, String::new(), None, None, None))
    };

    let mut context = state.default_context();   
    context.insert("asset", &asset);
    context.insert("user", &user);
    context.insert("err_msg", &message);
    Ok(layout("asset_form", &context.into_json()))
}

#[get("/asset/delete?<asset_id>")]
pub async fn delete_asset(asset_id: i32, user: UserCookie, state: &State<ServerState>) -> Result<Redirect, Redirect> {
    if !user.is_admin {
        return  Err(Redirect::to(uri!(ServerState::base(), super::index(Some("You must be admin user to delete assets!")))));
    }

    state.postgres_db.delete_asset(asset_id).await
        .map_err(|_| Redirect::to(uri!(ServerState::base(), assets(Some("Failed to delete asset")))))?;

    Ok(Redirect::to(uri!(ServerState::base(), assets(Option::<String>::None))))
}

#[post("/asset", data = "<form>")]
pub async fn save_asset(form: Form<AssetForm>, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {
    if !user.is_admin {
        return  Err(Redirect::to(uri!(ServerState::base(), super::index(Some("You must be admin user to edit assets!")))));
    }

    let mut asset = form.into_inner().to_asset();
    let db = state.postgres_db.clone();

    let asset_id = db.get_asset_id(&asset).await;
    if let Some(id) = asset_id {
        asset.set_id(id)
            .map_err(|e| Redirect::to(uri!(ServerState::base(), edit_asset(asset_id, Some(format!("Updating asset failed, error was {}", e))))))?;
        db.update_asset(&asset).await
            .map_err(|e| Redirect::to(uri!(ServerState::base(), edit_asset(asset_id, Some(format!("Updating asset failed, error was {}", e))))))?;
    } else {
        let _asset_id = db.insert_asset(&asset).await
            .map_err(|e| Redirect::to(uri!(ServerState::base(), edit_asset(asset_id, Some(format!("Couldn't insert assert, error was {}", e))))))?;
    }

    Ok(Redirect::to(uri!(ServerState::base(), assets(Option::<String>::None))))
}

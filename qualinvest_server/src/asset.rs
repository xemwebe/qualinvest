/// Viewing and analyzing assets

use rocket::State;
use rocket::response::Redirect;
use rocket_dyn_templates::Template;
use rocket::form::{Form, FromForm};
use super::rocket_uri_macro_error_msg;
use super::rocket_uri_macro_login;

use finql_data::{Asset, AssetHandler, QuoteHandler};
use qualinvest_core::accounts::AccountHandler;

use crate::user::UserCookie;
use crate::layout::layout;
use super::ServerState;

/// Structure for storing information in asset formular
#[derive(Debug,Serialize,Deserialize,FromForm)]
pub struct AssetForm {
    pub id: Option<usize>,
    pub name: String,
    pub isin: Option<String>,
    pub wkn: Option<String>, 
    pub note: Option<String>,
}

impl AssetForm {
    pub fn to_asset(&self) -> Asset {
        Asset{
            id: self.id,
            name: self.name.clone(),
            isin: self.isin.clone(),
            wkn: self.wkn.clone(),
            note: self.note.clone(),
        }
    }
}

#[get("/asset?<asset_id>")]
pub async fn analyze_asset(asset_id: Option<usize>, user_opt: Option<UserCookie>, state: &State<ServerState>) -> Result<Template,Redirect> {
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
    
    if let Some(asset_id) = asset_id {
        let ticker = db.get_all_ticker_for_asset(asset_id).await
            .map_err(|e| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg=format!("getting ticker failed: {}",e))))))?;
        let mut all_quotes = Vec::new();
        for t in ticker {
            let mut quotes = db.get_all_quotes_for_ticker(t.id.unwrap()).await
                .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="getting quotes failed".to_string())))))?;
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
            .map_err(|e| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg=format!("building transactions view failed: {}", e))))))?;
            context.insert("transactions", &transactions);
        }
    }
    Ok(layout("analyzeAsset", &context.into_json()))
}

#[get("/assets")]
pub async fn assets(user_opt: Option<UserCookie>, state: &State<ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(format!("{}{}",state.rel_path, uri!(login(redirect=Some("assets"))))));
    }
    let user = user_opt.unwrap();

    let db = state.postgres_db.clone();

    let assets = db.get_all_assets().await
        .map_err(|e| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg=format!("Couldn't get asset list, error was {}", e))))))?;

    let mut context = state.default_context();
    context.insert("assets", &assets);
    context.insert("user", &user);
    Ok(layout("assets", &context.into_json()))
}


#[get("/asset/edit?<asset_id>")]
pub async fn edit_asset(asset_id: Option<usize>, user_opt: Option<UserCookie>, state: &State<ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(format!("{}{}",state.rel_path, uri!(login(redirect=Some("assets"))))));
    }
    let user = user_opt.unwrap();
    if !user.is_admin {
        return  Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="You must be admin user to edit assets!")))));
    }

    let db = state.postgres_db.clone();

    let asset = if let Some(asset_id) = asset_id {
        db.get_asset_by_id(asset_id).await
            .map_err(|e| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg=format!("Couldn't get asset, error was {}", e))))))?
    } else {
        Asset::default()
    };

    let mut context = state.default_context();   
    context.insert("asset", &asset);
    context.insert("user", &user);
    Ok(layout("asset_form", &context.into_json()))
}

#[post("/asset", data = "<form>")]
pub async fn save_asset(form: Form<AssetForm>, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {
    if !user.is_admin {
        return  Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="You must be admin user to edit assets!")))));
    }

    let mut asset = form.into_inner().to_asset();
    let db = state.postgres_db.clone();

    let asset_id = db.get_asset_id(&asset).await;
    if let Some(id) = asset_id {
        asset.id = Some(id);
        db.update_asset(&asset).await
            .map_err(|e| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg=format!("Updating asset failed, error was {}", e))))))?;
    } else {
        let _asset_id = db.insert_asset_if_new(&asset, true).await
            .map_err(|e| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg=format!("Couldn't insert assert, error was {}", e))))))?;
    }

    Ok(Redirect::to(format!("{}{}", state.rel_path, uri!(assets()))))
}

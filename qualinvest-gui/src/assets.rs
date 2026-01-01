use cfg_if::cfg_if;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetFilter {
    pub user_id: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssetView {
    pub id: i32,
    pub name: String,
    pub class: String,
}

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use finql::postgres::PostgresDB;
        use finql::datatypes::AssetHandler;

        pub async fn get_assets_ssr(db: PostgresDB) -> Vec<AssetView> {
            if let Ok(assets) = db.get_asset_list().await {
                assets.into_iter().map(|a| AssetView {
                    id: a.id,
                    name: a.name,
                    class: a.class,
                }).collect()
            } else {
                Vec::new()
            }
        }
    }
}

#[server(Assets, "/api")]
pub async fn get_assets() -> Result<RwSignal<Vec<AssetView>>, ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use log::debug;

    debug!("get assets called");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let _user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    // Security Note: Assets are reference/master data (stocks, currencies) that all
    // authenticated users have read-only access to. This is intentional - users need
    // to browse available assets when creating transactions. Write access to assets
    // is controlled separately. Authorization is enforced at transaction/account level.

    let db = crate::db::get_db()?;
    Ok(RwSignal::new(get_assets_ssr(db).await))
}

#[server(InsertAsset, "/api")]
pub async fn insert_asset(asset: AssetView) -> Result<i32, ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use finql::datatypes::{Asset, AssetHandler, Currency, CurrencyISOCode, Stock};
    use log::debug;

    debug!("insert asset called with asset {asset:?}");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    if !user.is_admin {
        return Err(ServerFnError::new("Admin access required"));
    }

    let db = crate::db::get_db()?;

    let new_asset = if asset.class == "stock" {
        Asset::Stock(Stock {
            id: None,
            name: asset.name,
            wkn: None,
            isin: None,
            note: None,
        })
    } else if asset.class == "currency" {
        Asset::Currency(Currency {
            id: None,
            iso_code: CurrencyISOCode::new(&asset.name)
                .map_err(|_| ServerFnError::new("Invalid currency code"))?,
            rounding_digits: 4,
        })
    } else {
        return Err(ServerFnError::new("Invalid asset class"));
    };

    db.insert_asset(&new_asset)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to insert asset: {}", e)))
}

#[server(UpdateAsset, "/api")]
pub async fn update_asset(asset: AssetView) -> Result<(), ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use finql::datatypes::{Asset, AssetHandler, Currency, CurrencyISOCode, Stock};
    use log::debug;

    debug!("update asset called with asset {asset:?}");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    if !user.is_admin {
        return Err(ServerFnError::new("Admin access required"));
    }

    let db = crate::db::get_db()?;

    let updated_asset = if asset.class == "stock" {
        Asset::Stock(Stock {
            id: Some(asset.id),
            name: asset.name,
            wkn: None,
            isin: None,
            note: None,
        })
    } else if asset.class == "currency" {
        Asset::Currency(Currency {
            id: Some(asset.id),
            iso_code: CurrencyISOCode::new(&asset.name)
                .map_err(|_| ServerFnError::new("Invalid currency code"))?,
            rounding_digits: 4,
        })
    } else {
        return Err(ServerFnError::new("Invalid asset class"));
    };

    db.update_asset(&updated_asset)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to update asset: {}", e)))
}

#[server(DeleteAsset, "/api")]
pub async fn delete_asset(asset_id: i32) -> Result<(), ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use finql::datatypes::AssetHandler;
    use log::debug;

    debug!("delete asset called with id {asset_id}");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    if !user.is_admin {
        return Err(ServerFnError::new("Admin access required"));
    }

    let db = crate::db::get_db()?;

    db.delete_asset(asset_id)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to delete asset: {}", e)))
}

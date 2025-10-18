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

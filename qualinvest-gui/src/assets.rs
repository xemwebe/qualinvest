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
pub async fn get_assets(filter: AssetFilter) -> Result<RwSignal<Vec<AssetView>>, ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use log::debug;

    debug!("get assets called with filter {filter:?}");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    // Verify the authenticated user matches the requested user_id
    if user.id != filter.user_id as i32 {
        return Err(ServerFnError::new(
            "Forbidden: Cannot access other user's assets",
        ));
    }

    let db = crate::db::get_db()?;
    Ok(RwSignal::new(get_assets_ssr(db).await))
}

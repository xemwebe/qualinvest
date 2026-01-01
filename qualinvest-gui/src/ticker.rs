use cfg_if::cfg_if;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerFilter {
    pub asset_id: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TickerView {
    pub id: i32,
    pub name: String,
    pub asset: i32,
    pub currency_iso_code: String,
    pub source: String,
    pub priority: i32,
    pub factor: f64,
}

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use finql::postgres::PostgresDB;
        use finql::datatypes::QuoteHandler;

        pub async fn get_tickers_ssr(asset_id: i32, db: PostgresDB) -> Vec<TickerView> {
            if let Ok(tickers) = db.get_all_ticker_for_asset(asset_id).await {
                tickers.into_iter().map(|t| TickerView {
                    id: t.id.unwrap_or(0),
                    name: t.name,
                    asset: t.asset,
                    currency_iso_code: t.currency.iso_code.to_string(),
                    source: t.source,
                    priority: t.priority,
                    factor: t.factor,
                }).collect()
            } else {
                Vec::new()
            }
        }
    }
}

#[server(Tickers, "/api")]
pub async fn get_tickers(filter: TickerFilter) -> Result<RwSignal<Vec<TickerView>>, ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use log::debug;

    debug!("get tickers called with filter {filter:?}");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let _user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    // Security Note: Tickers are reference/market data that all authenticated users
    // have read-only access to. This is intentional - users need to view ticker
    // information for available assets. Authorization is enforced at transaction/account level.

    let db = crate::db::get_db()?;
    Ok(RwSignal::new(get_tickers_ssr(filter.asset_id, db).await))
}

#[server(InsertTicker, "/api")]
pub async fn insert_ticker(ticker: TickerView) -> Result<i32, ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use finql::datatypes::{CurrencyISOCode, QuoteHandler, Ticker};
    use log::debug;

    debug!("insert ticker called with ticker {ticker:?}");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    if !user.is_admin {
        return Err(ServerFnError::new("Admin access required"));
    }

    let db = crate::db::get_db()?;
    let market = crate::db::get_market()?;
    let currency = market
        .get_currency(CurrencyISOCode::new(&ticker.currency_iso_code)?)
        .await?;
    let new_ticker = Ticker {
        id: None,
        name: ticker.name,
        asset: ticker.asset,
        currency,
        source: ticker.source,
        priority: ticker.priority,
        factor: ticker.factor,
        cal: Some("target".to_string()),
        tz: None,
    };

    db.insert_ticker(&new_ticker)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to insert ticker: {}", e)))
}

#[server(UpdateTicker, "/api")]
pub async fn update_ticker(ticker: TickerView) -> Result<(), ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use finql::datatypes::{CurrencyISOCode, QuoteHandler, Ticker};
    use log::debug;

    debug!("update ticker called with ticker {ticker:?}");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    if !user.is_admin {
        return Err(ServerFnError::new("Admin access required"));
    }

    let db = crate::db::get_db()?;
    let market = crate::db::get_market()?;
    let currency = market
        .get_currency(CurrencyISOCode::new(&ticker.currency_iso_code)?)
        .await?;
    let updated_ticker = Ticker {
        id: Some(ticker.id),
        name: ticker.name,
        asset: ticker.asset,
        currency,
        source: ticker.source,
        priority: ticker.priority,
        factor: ticker.factor,
        cal: Some("target".to_string()),
        tz: None,
    };

    db.update_ticker(&updated_ticker)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to update ticker: {}", e)))
}

#[server(DeleteTicker, "/api")]
pub async fn delete_ticker(ticker_id: i32) -> Result<(), ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use finql::datatypes::QuoteHandler;
    use log::debug;

    debug!("delete ticker called with id {ticker_id}");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    if !user.is_admin {
        return Err(ServerFnError::new("Admin access required"));
    }

    let db = crate::db::get_db()?;

    db.delete_ticker(ticker_id)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to delete ticker: {}", e)))
}

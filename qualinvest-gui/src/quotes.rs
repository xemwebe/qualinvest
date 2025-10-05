use cfg_if::cfg_if;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteFilter {
    pub ticker_id: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QuoteView {
    pub id: i32,
    pub ticker: i32,
    pub price: f64,
    pub time: String,
    pub volume: Option<f64>,
}

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use finql::postgres::PostgresDB;
        use finql::datatypes::QuoteHandler;

        pub async fn get_quotes_ssr(ticker_id: i32, db: PostgresDB) -> Vec<QuoteView> {
            if let Ok(quotes) = db.get_all_quotes_for_ticker(ticker_id).await {
                quotes.into_iter().map(|q| QuoteView {
                    id: q.id.unwrap_or(0),
                    ticker: q.ticker,
                    price: q.price,
                    time: q.time.to_string(),
                    volume: q.volume,
                }).collect()
            } else {
                Vec::new()
            }
        }
    }
}

#[server(Quotes, "/api")]
pub async fn get_quotes(filter: QuoteFilter) -> Result<RwSignal<Vec<QuoteView>>, ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use log::debug;

    debug!("get quotes called with filter {filter:?}");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let _user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    let db = crate::db::get_db()?;
    Ok(RwSignal::new(get_quotes_ssr(filter.ticker_id, db).await))
}

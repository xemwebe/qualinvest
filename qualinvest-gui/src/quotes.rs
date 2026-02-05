use cfg_if::cfg_if;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::time_range::TimeRange;

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
        use finql::time_series::{TimeSeries, TimeValue};
        use qualinvest_core::plot::make_plot;
        use time::{OffsetDateTime, UtcOffset};
        use crate::time_range::TimeRangePoint;
        use crate::global_settings::GlobalSettings;
        use crate::error::Error;

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

        pub async fn get_quotes_graph_ssr(ticker_id: i32, db: PostgresDB) -> std::result::Result<String, Error> {
            let quotes = db.get_all_quotes_for_ticker(ticker_id).await.map_err(|_| Error::DatabaseAccessFailed)?;

            if quotes.is_empty() {
                return Err(Error::NoQuotesAvailable);
            }

            // Convert quotes to TimeSeries
            let mut items = Vec::new();
            for quote in quotes {
                items.push(TimeValue {
                    time: quote.time,
                    value: quote.price,
                });
            }

            let time_series = TimeSeries {
                title: format!("Ticker {}", ticker_id),
                series: items,
            };

            // Generate the plot
            make_plot("Price History", &[time_series])
                .map_err(|e| Error::PlotGenerationFailed(format!("{}", e)))
        }

        fn get_inception_date() -> Result<OffsetDateTime, Error> {
            use_context::<GlobalSettings>()
                .map(|settings| settings.inception_date)
                .ok_or(Error::MissingGlobalSettings)
        }

        async fn get_start(tr: &TimeRange, db: &PostgresDB, ticker_id: i32) -> Result<OffsetDateTime> {
            Ok(match tr {
                TimeRange::All => get_inception_date()?,
                TimeRange::Latest => if let Some(latest) = db.get_latest_quote_date_for_ticker(ticker_id).await? {
                    latest
                } else {
                    get_inception_date()?
                },
                TimeRange::Custom(range) => match range.start {
                    TimeRangePoint::Inception => get_inception_date()?,
                    TimeRangePoint::Today => OffsetDateTime::now_utc(),
                    TimeRangePoint::Custom(date) => date.midnight().assume_offset(UtcOffset::UTC),
                },
            })
        }

        fn get_end(tr: &TimeRange) -> Result<OffsetDateTime> {
            Ok(match tr {
                TimeRange::All => OffsetDateTime::now_utc(),
                TimeRange::Latest => OffsetDateTime::now_utc(),
                TimeRange::Custom(range) => match range.end {
                    TimeRangePoint::Inception => get_inception_date()?,
                    TimeRangePoint::Today => OffsetDateTime::now_utc(),
                    TimeRangePoint::Custom(date) => date.midnight().assume_offset(UtcOffset::UTC),
                },
            })
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

    // Security Note: Quotes are reference/market data that all authenticated users
    // have read-only access to. This is intentional - users need access to market
    // prices for portfolio valuation and analysis. Authorization is enforced at
    // transaction/account level.

    let db = crate::db::get_db()?;
    Ok(RwSignal::new(get_quotes_ssr(filter.ticker_id, db).await))
}

#[server(QuotesGraph, "/api")]
pub async fn get_quotes_graph(filter: QuoteFilter) -> Result<String, ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use log::debug;

    debug!("get quotes graph called with filter {filter:?}");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let _user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    // Security Note: Quotes are reference/market data that all authenticated users
    // have read-only access to. This is intentional - users need access to market
    // prices for portfolio valuation and analysis. Authorization is enforced at
    // transaction/account level.

    let db = crate::db::get_db()?;
    get_quotes_graph_ssr(filter.ticker_id, db)
        .await
        .map_err(ServerFnError::new)
}

#[server(UpdateQuotes, "/api")]
pub async fn update_quotes(ticker_id: i32, time_range: TimeRange) -> Result<(), ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use log::debug;
    use std::sync::Arc;

    debug!(
        "update quotes called for ticker {} with time range {:?}",
        ticker_id, time_range
    );

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    // Security Note: Only admin users can update quotes
    if !user.is_admin {
        return Err(ServerFnError::new("Forbidden: Admin access required"));
    }

    let db = Arc::new(crate::db::get_db().map_err(ServerFnError::new)?);
    debug!("db created");
    let start = get_start(&time_range, &db, ticker_id)
        .await
        .map_err(ServerFnError::new)?;
    debug!("start is {start:?}");
    let end = get_end(&time_range).map_err(ServerFnError::new)?;
    debug!("end is {end:?}");
    let market = crate::db::get_market().map_err(ServerFnError::new)?;
    market
        .update_quote_history(ticker_id, start, end)
        .await
        .map_err(ServerFnError::new)?;
    debug!("quotes updated");
    Ok(())
}

#[server(DeleteQuotes, "/api")]
pub async fn delete_quotes(ticker_id: i32, time_range: TimeRange) -> Result<(), ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use log::debug;

    debug!(
        "delete quotes called for ticker {} with time range {:?}",
        ticker_id, time_range
    );

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    // Security Note: Only admin users can delete quotes
    if !user.is_admin {
        return Err(ServerFnError::new("Forbidden: Admin access required"));
    }

    let db = crate::db::get_db().map_err(ServerFnError::new)?;
    debug!("db created");
    let start = get_start(&time_range, &db, ticker_id)
        .await
        .map_err(ServerFnError::new)?;
    debug!("start is {start:?}");
    let end = get_end(&time_range).map_err(ServerFnError::new)?;
    debug!("end is {end:?}");
    db.delete_quotes_for_ticker_id_in_range(ticker_id, start, end)
        .await
        .map_err(ServerFnError::new)?;
    debug!("quotes deleted successfully");
    Ok(())
}

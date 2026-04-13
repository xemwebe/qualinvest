use crate::time_range::TimeRange;
use cfg_if::cfg_if;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

// ── serialisable strategy parameters (sent from client to server) ─────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DividendParam {
    /// ISO-8601 date string, e.g. "2023-06-15"
    pub date: String,
    /// Amount per share
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyParams {
    /// Human-readable label shown in the graph legend.
    pub label: String,
    /// "StaticInSingleStock" | "ReInvestInSingleStock"
    pub strategy_type: String,
    pub asset_id: i32,
    /// Only required for ReInvestInSingleStock
    pub ticker_id: Option<i32>,
    /// Initial number of shares
    pub initial_position: f64,
    /// Initial cash in the portfolio
    pub initial_cash: f64,
    /// ISO-4217 currency code, e.g. "EUR"
    pub currency: String,
    // --- transaction costs ---
    pub min_fee: f64,
    /// None means no upper cap on fees
    pub max_fee: Option<f64>,
    /// As a fraction in [0, 1]
    pub proportional_fee: f64,
    /// As a fraction in [0, 1]
    pub tax_rate: f64,
    pub dividends: Vec<DividendParam>,
}

// ── SSR-only implementation ───────────────────────────────────────────────────

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use std::sync::Arc;
        use log::debug;

        use finql::datatypes::{
            CashFlow, CurrencyISOCode, Transaction, TransactionType,
            date_time_helper::date_to_offset_date_time,
        };
        use finql::period_date::PeriodDate;
        use finql::strategy::{
            ReInvestInSingleStock, StaticInSingleStock, StockTransactionCosts, StockTransactionFee,
        };
        use finql::strategy::calc_strategy;
        use finql::time_series::TimeSeries;
        use finql::Market;
        use qualinvest_core::plot::make_plot;
        use time::Date;
        use time::macros::format_description;
        use crate::time_range::{TimeRangePoint, CustomTimeRange};

        fn time_range_to_dates(time_range: TimeRange) -> Result<(PeriodDate, PeriodDate), ServerFnError> {
            let (start, end) = match time_range {
                TimeRange::All     => (PeriodDate::Inception, PeriodDate::Today),
                TimeRange::Latest  => (PeriodDate::Today,     PeriodDate::Today),
                TimeRange::Custom(CustomTimeRange { start, end }) => {
                    let s = match start {
                        TimeRangePoint::Inception     => PeriodDate::Inception,
                        TimeRangePoint::Today         => PeriodDate::Today,
                        TimeRangePoint::Custom(date)  => PeriodDate::FixedDate(date),
                    };
                    let e = match end {
                        TimeRangePoint::Inception     => PeriodDate::Inception,
                        TimeRangePoint::Today         => PeriodDate::Today,
                        TimeRangePoint::Custom(date)  => PeriodDate::FixedDate(date),
                    };
                    (s, e)
                }
            };
            Ok((start, end))
        }

        fn parse_date(s: &str) -> Result<Date, ServerFnError> {
            let fmt = format_description!("[year]-[month]-[day]");
            Date::parse(s, &fmt)
                .map_err(|e| ServerFnError::new(format!("Invalid date '{}': {}", s, e)))
        }

        /// Build the two initial transactions (cash deposit + asset purchase) that
        /// `calc_strategy` expects as its `start_transactions` argument.
        async fn build_start_transactions(
            params: &StrategyParams,
            market: &Market,
            start: Date,
        ) -> Result<Vec<Transaction>, ServerFnError> {
            let currency = market
                .get_currency(
                    CurrencyISOCode::new(&params.currency)
                        .map_err(|e| ServerFnError::new(format!("Bad currency '{}': {}", params.currency, e)))?,
                )
                .await
                .map_err(|e| ServerFnError::new(format!("Failed to resolve currency: {}", e)))?;

            let mut txns = Vec::new();

            // Cash deposit
            txns.push(Transaction {
                id: None,
                transaction_type: TransactionType::Cash,
                cash_flow: CashFlow::new(params.initial_cash, currency, start),
                note: Some("initial cash".to_string()),
            });

            // Initial stock position (valued at purchase price = 0 cost basis here;
            // the position count is what matters for strategy calculations)
            if params.initial_position != 0.0 {
                let start_time = date_to_offset_date_time(&start, 20, None)
                    .map_err(|e| ServerFnError::new(format!("Date conversion error: {}", e)))?;
                debug!("get asset price for asset_id={}, currency={currency}, start_time={start_time:?}", params.asset_id);
                let price = market
                    .get_asset_price(params.asset_id, currency, start_time)
                    .await
                    .map_err(|e| ServerFnError::new(format!("Failed to get asset price: {}", e)))?;

                txns.push(Transaction {
                    id: None,
                    transaction_type: TransactionType::Asset {
                        asset_id: params.asset_id,
                        position: params.initial_position,
                    },
                    cash_flow: CashFlow::new(
                        -params.initial_position * price,
                        currency,
                        start,
                    ),
                    note: Some("initial position".to_string()),
                });
            }

            Ok(txns)
        }

        /// Run a single strategy and return its time series.  Errors are turned into
        /// an empty series with the error message so one bad strategy doesn't abort
        /// the whole run.
        async fn run_one(
            params: StrategyParams,
            market: Market,
            start: Date,
            end: Date,
        ) -> TimeSeries {
            let result: Result<TimeSeries, ServerFnError> = async {
                let currency = market
                    .get_currency(
                        CurrencyISOCode::new(&params.currency)
                            .map_err(|e| ServerFnError::new(format!("Bad currency: {}", e)))?,
                    )
                    .await
                    .map_err(|e| ServerFnError::new(format!("Currency error: {}", e)))?;

                // Parse dividends
                let dividends: Vec<CashFlow> = params
                    .dividends
                    .iter()
                    .filter(|d| !d.date.is_empty() && d.amount != 0.0)
                    .map(|d| {
                        let date = parse_date(&d.date)?;
                        Ok(CashFlow::new(d.amount, currency, date))
                    })
                    .collect::<Result<Vec<_>, ServerFnError>>()?;

                let costs = StockTransactionCosts {
                    fee: StockTransactionFee::new(
                        params.min_fee,
                        params.max_fee,
                        params.proportional_fee,
                    ),
                    tax_rate: params.tax_rate,
                };

                let start_txns =
                    build_start_transactions(&params, &market, start).await?;

                let series = match params.strategy_type.as_str() {
                    "StaticInSingleStock" => {
                        let strategy =
                            StaticInSingleStock::new(params.asset_id, dividends, costs);
                        calc_strategy(currency, &start_txns, &strategy, start, end, market.clone())
                            .await
                    }
                    "ReInvestInSingleStock" => {
                        let ticker_id = params.ticker_id.ok_or_else(|| {
                            ServerFnError::new("ReInvestInSingleStock requires a ticker_id")
                        })?;
                        let strategy = ReInvestInSingleStock::new(
                            params.asset_id,
                            ticker_id,
                            market.clone(),
                            dividends,
                            costs,
                        );
                        calc_strategy(currency, &start_txns, &strategy, start, end, market.clone())
                            .await
                    }
                    other => {
                        return Err(ServerFnError::new(format!(
                            "Unknown strategy type '{}'",
                            other
                        )))
                    }
                };

                Ok(TimeSeries {
                    title: params.label.clone(),
                    series,
                })
            }
            .await;

            debug!("Result: {result:?}");
            result.unwrap_or_else(|e| TimeSeries {
                title: format!("{} (error: {})", params.label, e),
                series: Vec::new(),
            })
        }
    }
}

// ── server function ───────────────────────────────────────────────────────────

#[server(RunStrategies, "/api")]
pub async fn run_strategies(
    strategies: Vec<StrategyParams>,
    time_range: TimeRange,
) -> Result<String, ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use futures::future::join_all;
    use log::debug;

    debug!("run_strategies called with {} strategies", strategies.len());

    let auth: AuthSession<PostgresBackend> = expect_context();
    let _user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    if strategies.is_empty() {
        return Err(ServerFnError::new("No strategies provided"));
    }

    let db = crate::db::get_db()?;

    let (start_pd, end_pd) = time_range_to_dates(time_range)?;

    // Resolve the end date first (no transactions needed for that)
    let end = end_pd
        .date(None)
        .map_err(|e| ServerFnError::new(format!("Failed to resolve end date: {}", e)))?;

    // Use Inception start → resolve from a minimal placeholder date
    // (calc_strategy builds its own position from scratch, so we just need a
    //  sensible start date; PeriodDate::Inception with None falls back to today,
    //  so we use FixedDate when the user picked a real start).
    let start = match start_pd {
        PeriodDate::Inception => {
            // Fall back to the earliest date we can: use end itself if nothing better
            // (the user should pick a Custom range for meaningful simulations)
            PeriodDate::Today
                .date(None)
                .map_err(|e| ServerFnError::new(format!("Failed to resolve start date: {}", e)))?
        }
        other => other
            .date(None)
            .map_err(|e| ServerFnError::new(format!("Failed to resolve start date: {}", e)))?,
    };
    debug!("Time range: start={start:?}, end={end:?}");

    let market = Market::new_with_date_range(Arc::new(db), start, end)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to create market: {}", e)))?;

    debug!("market created");

    debug!("running {} strategies", strategies.len());

    // Run all strategies in parallel
    let futures: Vec<_> = strategies
        .into_iter()
        .map(|p| run_one(p, market.clone(), start, end))
        .collect();

    let all_series: Vec<_> = join_all(futures)
        .await
        .into_iter()
        .filter(|ts| !ts.series.is_empty())
        .collect();

    if all_series.is_empty() {
        return Err(ServerFnError::new(
            "All strategies produced empty time series. \
             Check your parameters and date range.",
        ));
    }

    make_plot("Strategy Simulation", &all_series)
        .map_err(|e| ServerFnError::new(format!("Failed to generate plot: {}", e)))
}

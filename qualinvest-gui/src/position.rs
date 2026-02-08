use cfg_if::cfg_if;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::time_range::TimeRange;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionRow {
    pub name: String,
    pub position: f64,
    pub purchase_value: f64,
    pub trading_pnl: f64,
    pub interest: f64,
    pub dividend: f64,
    pub fees: f64,
    pub tax: f64,
    pub currency: String,
    pub last_quote: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionTotalsView {
    pub value: f64,
    pub trading_pnl: f64,
    pub unrealized_pnl: f64,
    pub dividend: f64,
    pub interest: f64,
    pub tax: f64,
    pub fees: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionData {
    pub cash: PositionRow,
    pub assets: Vec<PositionRow>,
    pub totals: PositionTotalsView,
}

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use std::sync::Arc;
        use finql::datatypes::CurrencyISOCode;
        use finql::period_date::PeriodDate;
        use finql::time_series::TimeSeries;
        use crate::time_range::{TimeRangePoint, CustomTimeRange};

        fn time_range_to_period_dates(time_range: TimeRange) -> (PeriodDate, PeriodDate) {
            match time_range {
                TimeRange::All => (PeriodDate::Inception, PeriodDate::Today),
                TimeRange::Latest => (PeriodDate::Today, PeriodDate::Today),
                TimeRange::Custom(CustomTimeRange { start, end }) => {
                    let start = match start {
                        TimeRangePoint::Inception => PeriodDate::Inception,
                        TimeRangePoint::Today => PeriodDate::Today,
                        TimeRangePoint::Custom(date) => PeriodDate::FixedDate(date),
                    };
                    let end = match end {
                        TimeRangePoint::Inception => PeriodDate::Inception,
                        TimeRangePoint::Today => PeriodDate::Today,
                        TimeRangePoint::Custom(date) => PeriodDate::FixedDate(date),
                    };
                    (start, end)
                }
            }
        }
    }
}

#[server(GetPositions, "/api")]
pub async fn get_positions(
    account_ids: Vec<i32>,
    time_range: TimeRange,
) -> Result<PositionData, ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use log::debug;
    use qualinvest_core::user::UserHandler;

    debug!("get positions called for accounts {account_ids:?}");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    let db = crate::db::get_db()?;

    // Verify user has access to requested accounts
    if !user.is_admin {
        let user_accounts = db
            .get_user_accounts(user.id)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to get user accounts: {}", e)))?;
        let user_account_ids: Vec<i32> = user_accounts.iter().filter_map(|a| a.id).collect();
        for account_id in &account_ids {
            if !user_account_ids.contains(account_id) {
                return Err(ServerFnError::new(format!(
                    "Forbidden: Cannot access account {}",
                    account_id
                )));
            }
        }
    }

    let market = crate::db::get_market()?;
    // todo: read user's base currency from db instead of using "EUR" hard coded
    let currency = market
        .get_currency(CurrencyISOCode::new("EUR")?)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to get currency: {}", e)))?;

    let (start, end) = time_range_to_period_dates(time_range);

    let (portfolio, _totals) =
        qualinvest_core::position::calculate_position_for_period_for_accounts(
            currency,
            &account_ids,
            start,
            end,
            Arc::new(db),
        )
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to calculate positions: {}", e)))?;

    let cash = PositionRow {
        name: "Cash".to_string(),
        position: portfolio.cash.position,
        purchase_value: portfolio.cash.purchase_value,
        trading_pnl: portfolio.cash.trading_pnl,
        interest: portfolio.cash.interest,
        dividend: portfolio.cash.dividend,
        fees: portfolio.cash.fees,
        tax: portfolio.cash.tax,
        currency: portfolio.cash.currency.iso_code.to_string(),
        last_quote: portfolio.cash.last_quote,
    };

    let assets = portfolio
        .assets
        .values()
        .map(|pos| PositionRow {
            name: pos.name.clone(),
            position: pos.position,
            purchase_value: pos.purchase_value,
            trading_pnl: pos.trading_pnl,
            interest: pos.interest,
            dividend: pos.dividend,
            fees: pos.fees,
            tax: pos.tax,
            currency: pos.currency.iso_code.to_string(),
            last_quote: pos.last_quote,
        })
        .collect();

    // Compute totals from portfolio data (PositionTotals has private fields)
    let mut totals_view = PositionTotalsView {
        value: portfolio.cash.position,
        trading_pnl: portfolio.cash.trading_pnl,
        unrealized_pnl: 0.0,
        dividend: portfolio.cash.dividend,
        interest: portfolio.cash.interest,
        tax: portfolio.cash.tax,
        fees: portfolio.cash.fees,
    };
    for pos in portfolio.assets.values() {
        let pos_value = if let Some(quote) = pos.last_quote {
            pos.position * quote
        } else {
            -pos.purchase_value
        };
        totals_view.value += pos_value;
        totals_view.trading_pnl += pos.trading_pnl;
        totals_view.unrealized_pnl += pos_value + pos.purchase_value;
        totals_view.dividend += pos.dividend;
        totals_view.interest += pos.interest;
        totals_view.tax += pos.tax;
        totals_view.fees += pos.fees;
    }

    Ok(PositionData {
        cash,
        assets,
        totals: totals_view,
    })
}

#[server(GetPerformanceGraph, "/api")]
pub async fn get_performance_graph(
    account_ids: Vec<i32>,
    time_range: TimeRange,
) -> Result<String, ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use finql::Market;
    use log::debug;
    use qualinvest_core::accounts::AccountHandler;
    use qualinvest_core::performance::calc_performance;
    use qualinvest_core::plot::make_plot;
    use qualinvest_core::user::UserHandler;

    debug!("get performance graph called for accounts {account_ids:?}");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    let db = crate::db::get_db()?;

    if !user.is_admin {
        let user_accounts = db
            .get_user_accounts(user.id)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to get user accounts: {}", e)))?;
        let user_account_ids: Vec<i32> = user_accounts.iter().filter_map(|a| a.id).collect();
        for account_id in &account_ids {
            if !user_account_ids.contains(account_id) {
                return Err(ServerFnError::new(format!(
                    "Forbidden: Cannot access account {}",
                    account_id
                )));
            }
        }
    }

    let (start_pd, end_pd) = time_range_to_period_dates(time_range);

    let end = end_pd
        .date(None)
        .map_err(|e| ServerFnError::new(format!("Failed to resolve end date: {}", e)))?;
    let transactions = db
        .get_transactions_before_time(&account_ids, end)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to get transactions: {}", e)))?;
    let start = start_pd
        .date_from_trades(&transactions)
        .map_err(|e| ServerFnError::new(format!("Failed to resolve start date: {}", e)))?;

    let market = Market::new_with_date_range(Arc::new(db), start, end)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to create market: {}", e)))?;

    let currency = market
        .get_currency(CurrencyISOCode::new("EUR")?)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to get currency: {}", e)))?;

    let performance = calc_performance(currency, &transactions, start, end, &market, "TARGET")
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to calculate performance: {}", e)))?;

    let time_series = TimeSeries {
        title: "Portfolio Value".to_string(),
        series: performance,
    };

    make_plot("Performance", &[time_series])
        .map_err(|e| ServerFnError::new(format!("Failed to generate plot: {}", e)))
}

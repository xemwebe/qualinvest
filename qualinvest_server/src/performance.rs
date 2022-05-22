/// View performance comparison of different assets / portfolios
use std::str::FromStr;
use super::rocket_uri_macro_login;
use chrono::Local;
use finql::{
    Market,
    postgres::PostgresDB,
    datatypes::{AssetHandler, DataError, date_time_helper::{DateTimeError, naive_date_to_date_time}},
    portfolio::{calc_delta_position, calculate_position_and_pnl},
    time_series::{TimeSeries, TimeValue},
};
use qualinvest_core::accounts::AccountHandler;
use rocket::response::Redirect;
use rocket::State;
use rocket_dyn_templates::Template;
use std::sync::Arc;
use thiserror::Error;

use super::ServerState;
use crate::layout::layout;
use crate::user::UserCookie;

/// Error related to market data object
#[derive(Error, Debug)]
pub enum PerformanceError {
    #[error("List of transactions seems is empty")]
    NoTransactionsFound,
    #[error("Database return error")]
    DatabaseAccessError(#[from] DataError),
    #[error("Market data error")]
    MarketDataError(#[from] finql::market::MarketError),
    #[error("Calculate position failed")]
    CalculatePositionFailed(#[from] finql::portfolio::PositionError),
    #[error("Invalid time period")]
    InvalidTimePeriod(#[from] finql::time_period::TimePeriodError),
    #[error("Date time error")]
    InvalidDateTime(#[from] DateTimeError),
}

#[get("/performance?<asset_ids>&<message>")]
pub async fn performance(
    asset_ids: Option<Vec<i32>>,
    message: Option<String>,
    user_opt: Option<UserCookie>,
    state: &State<ServerState>,
) -> Result<Template, Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(uri!(
            ServerState::base(),
            login(Some("performance"))
        )));
    }
    let user = user_opt.unwrap();

    let mut message = message;
    let mut context = state.default_context();

    let db = state.postgres_db.clone();
    if let Ok(mut asset_list) = db.get_asset_list().await {
        asset_list.sort_by(|a, b| a.name.cmp(&b.name));
        context.insert("assets", &asset_list);
    } else {
        message = Some("No assets found!".to_string());
    }
    let user_accounts = user.get_accounts(db.clone()).await;
    context.insert("accounts", &user_accounts);

    let account_pnl = account_performance(1, "Total portfolio return", db).await;
    if let Ok(pnl_series) = account_pnl {
        context.insert("time_series", &pnl_series);
    } else {
        message = Some(format!(
            "Create total portfolio return graph failed: {:?}",
            account_pnl.err()
        ));
    }

    context.insert("asset_ids", &asset_ids);
    context.insert("user", &user);

    context.insert("err_msg", &message);
    Ok(layout("performance", &context.into_json()))
}

pub async fn account_performance(
    account_id: i32,
    name: &str,
    db: Arc<PostgresDB>,
) -> Result<TimeSeries, PerformanceError> {
    // Calculate total p&l time series
    let market = Market::new(db.clone());
    let transactions = db.get_all_transactions_with_account(account_id).await?;
    let currency = market.get_currency("EUR").await?;
    let calendar = market.get_calendar("TARGET")?;
    let mut current_date = transactions.iter().map(|t| t.cash_flow.date).min().ok_or(PerformanceError::NoTransactionsFound)?;
    let end_date = Local::now().naive_local().date();
    let (mut position, _) =
        calculate_position_and_pnl(currency, &transactions, Some(current_date), db).await?;
    let mut total_pnls = TimeSeries::new(name);
    let period = finql::time_period::TimePeriod::from_str("1B")?;
    while current_date < end_date {
        let next_date = period.add_to(current_date, Some(&calendar));
        calc_delta_position(&mut position, &transactions, Some(current_date), Some(next_date))?;
        let totals = position.calc_totals();
        total_pnls.series.push(TimeValue {
            time: naive_date_to_date_time(&next_date, 0, None)?,
            value: totals.value,
        });
        current_date = next_date
    }
    Ok(total_pnls)
}

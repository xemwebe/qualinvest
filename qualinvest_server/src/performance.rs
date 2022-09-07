/// View performance comparison of different assets / portfolios
use std::str::FromStr;
use finql::{
    Market,
    postgres::PostgresDB,
    datatypes::{DataError, date_time_helper::{DateTimeError, naive_date_to_date_time}},
    portfolio::{calc_delta_position, calculate_position_and_pnl},
    time_series::{TimeSeries, TimeValue},
    period_date::{PeriodDate, PeriodDateError},
};
use qualinvest_core::{
    accounts::AccountHandler,
    user::UserHandler
};
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
    #[error("Invalid period date")]
    InvalidPeriodDate(#[from] PeriodDateError),
}

#[get("/performance?<message>")]
pub async fn performance(
    message: Option<String>,
    user: UserCookie,
    state: &State<ServerState>,
) -> Result<Template, Redirect> {
    let db = state.postgres_db.clone();

    let user_settings = db.get_user_settings(user.userid).await;

    let mut message = message;
    let mut context = state.default_context();

    for account_id in user_settings.account_ids {
        let account_pnl = account_performance(account_id, &format!("performance of {}", account_id),
        user_settings.period_start, user_settings.period_end, db.clone()).await;
        if let Ok(pnl_series) = account_pnl {
            context.insert("time_series", &pnl_series);
        } else {
            message = Some(format!(
                "Create total portfolio return graph failed: {:?}",
                account_pnl.err()
            ));
        }
    }



    context.insert("user", &user);
    context.insert("err_msg", &message);
    Ok(layout("performance", &context.into_json()))
}

pub async fn account_performance(
    account_id: i32,
    name: &str,
    start: PeriodDate,
    end: PeriodDate,
    db: Arc<PostgresDB>,
) -> Result<TimeSeries, PerformanceError> {
    // Calculate total p&l time series
    let transactions = db.get_all_transactions_with_account(account_id).await?;
    let mut current_date = start.date_from_trades(&transactions)?;
    let end_date = end.date(None)?;
    let mut dates = vec![current_date];
    let market = Market::new_with_date_range(db.clone(), current_date, end_date).await?;
    let currency = market.get_currency_from_str("EUR").await?;
    let calendar = market.get_calendar("TARGET")?;
    let period = finql::time_period::TimePeriod::from_str("1B")?;
    while current_date <= end_date {
        let next_date = period.add_to(current_date, Some(calendar));
        dates.push(next_date);
        current_date = next_date;
    }
    let (mut position, _) =
        calculate_position_and_pnl(currency, &transactions, Some(dates[0]), &market).await?;
    let mut time = naive_date_to_date_time(&dates[0], 0, None)?;
    position.add_quote(time, &market).await;
    let mut total_pnls = TimeSeries::new(name);
    for i in 1..dates.len() {
        calc_delta_position(&mut position, &transactions, Some(dates[i-1]), Some(dates[i]), market.clone()).await?;
        time = naive_date_to_date_time(&dates[i], 20, None)?;
        position.add_quote(time, &market).await;
        let totals = position.calc_totals();
        total_pnls.series.push(TimeValue {
            time,
            value: totals.value,
        });    
    }

    Ok(total_pnls)
}

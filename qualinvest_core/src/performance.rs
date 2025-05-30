use chrono::NaiveDate;
use std::cmp::min;

use finql::{
    datatypes::{
        date_time_helper::{from_date, naive_date_to_date_time, to_date},
        Currency, Transaction,
    },
    portfolio::{calc_delta_position, PortfolioPosition},
    time_series::TimeValue,
    Market,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PerformanceError {
    #[error("Failed to calculate position")]
    PositionError(#[from] finql::portfolio::PositionError),
    #[error("Date calculation error")]
    DateError(#[from] finql::datatypes::date_time_helper::DateTimeError),
    #[error("Market error")]
    MarketError(#[from] finql::market::MarketError),
    #[error("Invalid date")]
    InvalidDate(#[from] cal_calc::CalendarError),
}

pub async fn calc_performance(
    currency: Currency,
    transactions: &[Transaction],
    start: NaiveDate,
    end: NaiveDate,
    market: &Market,
    calendar: &str,
) -> Result<Vec<TimeValue>, PerformanceError> {
    let mut current_date = start;
    let mut total_return = Vec::new();
    let cal = market.get_calendar(calendar)?;

    let mut position = PortfolioPosition::new(currency);
    calc_delta_position(
        &mut position,
        transactions,
        Some(start),
        Some(start),
        market.clone(),
    )
    .await?;
    position
        .add_quote(naive_date_to_date_time(&start, 20, None)?, market)
        .await;

    while current_date < end {
        // roll position forward to next day
        let next_date = min(end, from_date(cal.next_bday(to_date(current_date)?)?)?);
        calc_delta_position(
            &mut position,
            transactions,
            Some(current_date),
            Some(next_date),
            market.clone(),
        )
        .await?;

        current_date = next_date;
        let current_time = naive_date_to_date_time(&current_date, 20, None)?;
        position.add_quote(current_time, market).await;
        let totals = position.calc_totals();
        total_return.push(TimeValue {
            value: totals.value,
            time: current_time,
        });
    }
    Ok(total_return)
}

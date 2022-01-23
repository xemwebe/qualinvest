use std::cmp::min;

use chrono::NaiveDate;

use finql::{
    portfolio::{PortfolioPosition, calc_delta_position},
    time_series::TimeValue,
    Market,
};

use finql_data::{
    date_time_helper::{naive_date_to_date_time},
    Currency, Transaction, 
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PerformanceError {
    #[error("Failed to calculate position")]
    PositionError(#[from] finql::portfolio::PositionError),
    #[error("Date calculation error")]
    DateError(#[from] finql_data::date_time_helper::DateTimeError),
    #[error("Market error")]
    MarketError(#[from] finql::market::MarketError),
}

pub async fn calc_performance(
    currency: Currency,
    transactions: &Vec<Transaction>,
    start: NaiveDate,
    end: NaiveDate,
    market: &Market,
    calendar: &str,
) -> Result<Vec<TimeValue>, PerformanceError> {
    let mut current_date = start;
    let mut total_return = Vec::new();
    let cal = market.get_calendar(calendar)?;

    let mut position = PortfolioPosition::new(currency);
    calc_delta_position(&mut position, &transactions, Some(start), Some(start))?;
    position
        .add_quote(naive_date_to_date_time(&start, 20, None)?, &market)
        .await;

    while current_date < end {
        // roll position forward to next day
        let next_date = min(end, cal.next_bday(current_date));
        calc_delta_position(
            &mut position,
            &transactions,
            Some(current_date),
            Some(next_date),
        )?;

        current_date = next_date;
        let current_time = naive_date_to_date_time(&current_date, 20, None)?;
        position.add_quote(current_time, &market).await;
        let totals = position.calc_totals();
        total_return.push(TimeValue {
            value: totals.value,
            time: current_time,
        });
    }
    Ok(total_return)
}


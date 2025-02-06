use std::sync::Arc;

use finql::datatypes::Currency;
use finql::market::Market;
use finql::period_date::PeriodDate;
use finql::portfolio::{
    calculate_position_for_period, PortfolioPosition, PositionError, PositionTotals,
};
use finql::postgres::PostgresDB;

use crate::accounts::AccountHandler;

// Calculate position for a given period for transactions in a set of accounts
pub async fn calculate_position_for_period_for_accounts(
    currency: Currency,
    account_ids: &[i32],
    start: PeriodDate,
    end: PeriodDate,
    db: Arc<PostgresDB>,
) -> Result<(PortfolioPosition, PositionTotals), PositionError> {
    let end = end.date(None)?;
    let transactions = db.get_transactions_before_time(account_ids, end).await?;
    let start = start.date_from_trades(&transactions)?;
    let market = Market::new_with_date_range(db, start, end).await?;
    calculate_position_for_period(currency, &transactions, start, end, &market).await
}

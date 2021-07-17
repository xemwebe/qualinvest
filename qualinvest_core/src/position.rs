use std::sync::Arc;

use chrono::NaiveDate;

use finql_data::Currency;
use finql_postgres::PostgresDB;
use finql::portfolio::{PortfolioPosition, PositionTotals, PositionError, calculate_position_for_period};

use crate::accounts::AccountHandler;

// Calculate position for a given period for transactions in a set of accounts
pub async fn calculate_position_for_period_for_accounts(currency: Currency, account_ids: &[usize], 
    start: NaiveDate, end: NaiveDate, db: Arc<PostgresDB>) 
        -> Result<(PortfolioPosition, PositionTotals), PositionError> {
    let transactions = db.get_transactions_before_time(account_ids, end).await
        .map_err(PositionError::NoTransaction)?;
    calculate_position_for_period(currency, &transactions, start, end, db).await
}

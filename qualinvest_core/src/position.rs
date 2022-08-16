use std::sync::Arc;

use finql::datatypes::{Currency, date_time_helper::naive_date_to_date_time};
use finql::postgres::PostgresDB;
use finql::portfolio::{PortfolioPosition, PositionTotals, PositionError, calculate_position_for_period};
use finql::period_date::PeriodDate;
use finql::market::{Market, CachePolicy, TimeRange};

use crate::accounts::AccountHandler;

// Calculate position for a given period for transactions in a set of accounts
pub async fn calculate_position_for_period_for_accounts(currency: Currency, account_ids: &[i32], 
    start: PeriodDate, end: PeriodDate, db: Arc<PostgresDB>) 
        -> Result<(PortfolioPosition, PositionTotals), PositionError> {
    let end = end.date(None)?;
    let transactions = db.get_transactions_before_time(account_ids, end).await?;
    let start = start.date_from_trades(&transactions)?;
    let mut market = Market::new(db).await;
    market.set_cache_policy(CachePolicy::PredefinedPeriod(TimeRange{start: naive_date_to_date_time(&start, 0, None)?, end: naive_date_to_date_time(&end, 24, None)?}));
    let market = Arc::new(market);
    calculate_position_for_period(currency, &transactions, start, end, market).await
}

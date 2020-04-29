use crate::position::Position;
use chrono::{DateTime, Local, Utc};
use finql::data_handler::{DataError, QuoteHandler};
use finql::fx_rates::get_fx_rate;
use serde::{Deserialize, Serialize};
use std::convert::From;
use std::{error, fmt};

#[derive(Debug, Deserialize, Serialize)]
pub struct PnLPosition {
    position: Position,
    last_quote: f64,
    last_quote_time: DateTime<Utc>,
}

#[derive(Debug)]
pub enum PnLError {
    NoQuote(DataError),
    NoFxRate(DataError),
}

impl fmt::Display for PnLError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Calculation of P&L failed.")
    }
}

impl error::Error for PnLError {
    fn cause(&self) -> Option<&dyn error::Error> {
        match self {
            Self::NoQuote(err) => Some(err),
            Self::NoFxRate(err) => Some(err),
        }
    }
}

impl PnLPosition {
    pub fn from_position(
        position: &Position,
        time: DateTime<Utc>,
        db: &mut dyn QuoteHandler,
    ) -> Result<PnLPosition, PnLError> {
        let (last_quote, last_quote_time) = if let Some(asset_id) = position.asset_id {
            let (quote, currency) = db
                .get_last_quote_before_by_id(asset_id, time)
                .map_err(|e| PnLError::NoQuote(e))?;
            if currency == position.currency {
                (quote.price, quote.time)
            } else {
                let fx_rate = get_fx_rate(currency, position.currency, time, db)
                    .map_err(|e| PnLError::NoFxRate(e))?;
                (quote.price * fx_rate, quote.time)
            }
        } else {
            (1.0, DateTime::<Utc>::from(Local::now()))
        };
        Ok(PnLPosition {
            position: position.clone(),
            last_quote,
            last_quote_time,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finql::asset::Asset;
    use finql::data_handler::asset_handler::AssetHandler;
    use finql::quote::{MarketDataSource, Quote, Ticker};
    use finql::sqlite_handler::SqliteDB;
    use std::str::FromStr;

    #[test]
    fn test_from_position() {
        // Make new database
        let mut db = SqliteDB::create(":memory:").unwrap();
        // first add some assets
        let eur_id = db
            .insert_asset(&Asset {
                id: None,
                name: "EUR Stock".to_string(),
                wkn: None,
                isin: None,
                note: None,
            })
            .unwrap();
        // first add some assets
        let us_id = db
            .insert_asset(&Asset {
                id: None,
                name: "US Stock".to_string(),
                wkn: None,
                isin: None,
                note: None,
            })
            .unwrap();
        let eur = finql::Currency::from_str("EUR").unwrap();
        let usd = finql::Currency::from_str("USD").unwrap();
        // add ticker
        let _eur_ticker_id = db
            .insert_ticker(&Ticker {
                id: None,
                name: "EUR_STOCK.DE".to_string(),
                asset: eur_id,
                priority: 10,
                currency: eur,
                source: MarketDataSource::Manual,
                factor: 1.0,
            })
            .unwrap();
        let _us_ticker_id = db
            .insert_ticker(&Ticker {
                id: None,
                name: "US_STOCK.DE".to_string(),
                asset: us_id,
                priority: 10,
                currency: usd,
                source: MarketDataSource::Manual,
                factor: 1.0,
            })
            .unwrap();
        // add quotes
        let time = finql::helpers::make_time(2019, 12, 30, 10, 0, 0).unwrap();
        let _ = db
            .insert_quote(&Quote {
                id: None,
                ticker: eur_id,
                price: 12.34,
                time,
                volume: None,
            })
            .unwrap();
        let _ = db
            .insert_quote(&Quote {
                id: None,
                ticker: us_id,
                price: 43.21,
                time,
                volume: None,
            })
            .unwrap();
        let eur_position = Position {
            asset_id: Some(eur_id),
            name: "EUR Stock".to_string(),
            position: 1000.0,
            purchase_value: 10.0,
            realized_pnl: 0.,
            interest: 0.,
            dividend: 0.,
            fees: 0.,
            tax: 0.,
            currency: eur,
        };
        let usd_position = Position {
            asset_id: Some(us_id),
            name: "US Stock".to_string(),
            position: 1000.0,
            purchase_value: 10.0,
            realized_pnl: 0.,
            interest: 0.,
            dividend: 0.,
            fees: 0.,
            tax: 0.,
            currency: eur,
        };
        finql::fx_rates::insert_fx_quote(2.0, usd, eur, time, &mut db).unwrap();
        let time = finql::helpers::make_time(2019, 12, 30, 12, 0, 0).unwrap();
        let eur_pnl = PnLPosition::from_position(&eur_position, time, &mut db).unwrap();
        assert_eq!(eur_pnl.last_quote, 12.34);
        assert_eq!(
            eur_pnl.last_quote_time.format("%F %H:%M:%S").to_string(),
            "2019-12-30 09:00:00"
        );
        let usd_pnl = PnLPosition::from_position(&usd_position, time, &mut db).unwrap();
        assert_eq!(usd_pnl.last_quote, 86.42);
        assert_eq!(
            usd_pnl.last_quote_time.format("%F %H:%M:%S").to_string(),
            "2019-12-30 09:00:00"
        );
    }
}

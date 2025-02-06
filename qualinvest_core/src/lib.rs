///! # qualinvest_core
///!
///! This library is part of a set of tools for quantitative investments.
///! For mor information, see [qualinvest on github](https://github.com/xemwebe/qualinvest)
///!
use std::sync::Arc;

use chrono::{Datelike, Local};
use serde::Deserialize;

use cal_calc::{Calendar, Holiday};
use finql::{
    datatypes::QuoteHandler,
    market::{Market, MarketError},
    market_quotes::MarketDataSource,
};

pub mod accounts;
pub mod performance;
pub mod position;
pub mod postgres_user;
pub mod read_pdf;
pub mod sanitization;
pub mod user;

/// Configuration parameters
#[derive(Debug, Deserialize)]
pub struct Config {
    pub db: DbParams,
    pub pdf: PdfParseParams,
    pub market_data: MarketDataProviders,
    pub server: ServerSettings,
    pub debug: bool,
}

/// Database parameters
#[derive(Debug, Deserialize)]
pub struct DbParams {
    pub url: String,
}

/// Parameters for PDF file parsing
#[derive(Debug, Deserialize)]
pub struct PdfParseParams {
    pub doc_path: String,
    pub warn_old: bool,
    pub consistency_check: bool,
    pub rename_asset: bool,
    pub default_account: Option<i32>,
}

/// Market data provider settings
#[derive(Debug, Deserialize)]
pub struct MarketDataProviders {
    pub alpha_vantage_token: Option<String>,
    pub gurufocus_token: Option<String>,
    pub eod_historical_data_token: Option<String>,
}

/// Market data provider settings
#[derive(Debug, Deserialize)]
pub struct ServerSettings {
    pub port: Option<u16>,
    pub relative_path: Option<String>,
    pub secret_key: Option<String>,
}

fn add_provider(market: &finql::Market, token: &Option<String>, source: MarketDataSource) {
    if let Some(token) = token {
        if let Some(provider) = source.get_provider(token.clone()) {
            market.add_provider(source.to_string(), provider)
        }
    }
}

fn set_market_providers(market: &finql::Market, providers: &MarketDataProviders) {
    // yahoo is always present
    let yahoo = MarketDataSource::Yahoo;
    market.add_provider(
        yahoo.to_string(),
        yahoo.get_provider(String::new()).unwrap(),
    );
    add_provider(
        &market,
        &providers.alpha_vantage_token,
        MarketDataSource::AlphaVantage,
    );
    add_provider(
        &market,
        &providers.gurufocus_token,
        MarketDataSource::GuruFocus,
    );
    add_provider(
        &market,
        &providers.eod_historical_data_token,
        MarketDataSource::EodHistData,
    );
    let codi = MarketDataSource::Comdirect;
    market.add_provider(codi.to_string(), codi.get_provider(String::new()).unwrap());
}

pub async fn setup_market(
    db: Arc<dyn QuoteHandler + Send + Sync>,
    market_data: &MarketDataProviders,
) -> Market {
    let mut market = finql::Market::new(db).await;
    set_market_providers(&mut market, market_data);
    market
}

pub async fn fill_quote_gaps(market: &mut Market, min_size: usize) -> Result<(), MarketError> {
    use finql::datatypes::date_time_helper::naive_date_to_date_time;
    use finql::time_series::{TimeSeries, TimeValue};

    let today = Local::now().naive_local().date();

    let tickers = market.db().get_all_ticker().await?;

    let weekends_cal = Calendar::calc_calendar(
        &[
            Holiday::WeekDay(time::Weekday::Saturday),
            Holiday::WeekDay(time::Weekday::Sunday),
        ],
        2000,
        today.year(),
    )?;
    for ticker in tickers {
        if let Some(ticker_id) = ticker.id {
            let quotes = market.db().get_all_quotes_for_ticker(ticker_id).await?;
            let quote_series: Vec<TimeValue> = quotes
                .into_iter()
                .map(|q| TimeValue {
                    time: q.time,
                    value: q.price,
                })
                .collect();

            let ts = TimeSeries {
                series: quote_series,
                title: ticker.name.clone(),
            };
            let cal = if let Some(cal) = ticker.cal {
                market.get_calendar(&cal)?
            } else {
                &weekends_cal
            };
            let gaps = ts.find_gaps(cal, min_size).unwrap_or_default();
            for gap in gaps {
                let _ = market
                    .update_quote_history(
                        ticker_id,
                        naive_date_to_date_time(&gap.0, 0, ticker.tz.clone())?,
                        naive_date_to_date_time(&gap.1, 23, ticker.tz.clone())?,
                    )
                    .await;
            }
        }
    }

    Ok(())
}

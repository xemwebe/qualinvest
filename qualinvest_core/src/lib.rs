///! # qualinvest_core
///! 
///! This library is part of a set of tools for quantitative investments.
///! For mor information, see [qualinvest on github](https://github.com/xemwebe/qualinvest)
///!

use std::sync::Arc;
use std::str::FromStr;
use std::ops::Deref;

use serde::Deserialize;
use chrono::{DateTime, Local, Weekday, Datelike};

use finql_data::QuoteHandler;
use finql::{
    calendar::{Calendar, Holiday},
    market::MarketError,
    market_quotes::MarketDataSource
    };

pub mod accounts;
pub mod position;
pub mod read_pdf;
pub mod user;
pub mod postgres_user;
pub mod sanitization;
pub mod performance;

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
    pub default_account: Option<usize>,
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

fn add_provider(
    market: &mut finql::Market,
    token: &Option<String>,
    source: MarketDataSource,
) {
    if let Some(token) = token {
        if let Some(provider) = source.get_provider(token.clone()) {
            market.add_provider(source.to_string(), provider)
        }
    }
}

fn set_market_providers(market: &mut finql::Market, providers: &MarketDataProviders) {
    // yahoo is always present
    let yahoo = MarketDataSource::Yahoo;
    market.add_provider(
        yahoo.to_string(),
        yahoo.get_provider(String::new()).unwrap(),
    );
    add_provider(
        market,
        &providers.alpha_vantage_token,
        MarketDataSource::AlphaVantage,
    );
    add_provider(
        market,
        &providers.gurufocus_token,
        MarketDataSource::GuruFocus,
    );
    add_provider(
        market,
        &providers.eod_historical_data_token,
        MarketDataSource::EodHistData,
    );
    let codi = MarketDataSource::Comdirect;
    market.add_provider(codi.to_string(), codi.get_provider(String::new()).unwrap());
}

pub async fn update_quote_history(
    ticker_id: usize,
    start: DateTime<Local>,
    end: DateTime<Local>,
    db: Arc<dyn QuoteHandler+Send+Sync>,
    market_data: &MarketDataProviders,
) -> Result<(), MarketError> {
    let mut market = finql::Market::new(db);
    set_market_providers(&mut market, market_data);
    market.update_quote_history(ticker_id, start, end).await
}

pub async fn update_ticker(
    ticker_id: usize,
    db: Arc<dyn QuoteHandler+Send+Sync>,
    market_data: &MarketDataProviders,
) -> Result<(), MarketError> {
    let ticker = db.get_ticker_by_id(ticker_id).await?;
    let ticker_source = MarketDataSource::from_str(&ticker.source)?;
    let token = match ticker_source {
        MarketDataSource::AlphaVantage => market_data.alpha_vantage_token.clone(),
        MarketDataSource::GuruFocus => market_data.gurufocus_token.clone(),
        MarketDataSource::EodHistData => market_data.eod_historical_data_token.clone(),
        _ => Some(String::new()),
    };
    if let Some(token) = token {
        let provider = ticker_source.get_provider(token);
        if let Some(provider) = provider {
            finql::market_quotes::update_ticker(provider.deref(), &ticker, db).await?;
        }
    }
    Ok(())
}

pub async fn update_quotes(
    db: Arc<dyn QuoteHandler+Send+Sync>,
    market_data: &MarketDataProviders,
) -> Result<Vec<usize>, MarketError> {
    let mut market = finql::Market::new(db);
    set_market_providers(&mut market, market_data);
    market.update_quotes().await
}

pub async fn fill_quote_gaps(
    db: Arc<dyn QuoteHandler+Send+Sync>,
    market_data: &MarketDataProviders,
) -> Result<(), MarketError> {
    use finql::time_series::{TimeValue, TimeSeries};

    let today = Local::now().naive_local().date();

    let mut market = finql::Market::new(db.clone());
    set_market_providers(&mut market, market_data);
    let tickers = db.get_all_ticker().await?;

    let cal = Calendar::calc_calendar(&[Holiday::WeekDay(Weekday::Sat), Holiday::WeekDay(Weekday::Sun)], 2000, today.year());
    for ticker in tickers {
        if ticker.name != "BHP.AX" {
            continue;
        }
        if let Some(ticker_id) = ticker.id {
            let quotes = db.get_all_quotes_for_ticker(ticker_id).await?;
            let quote_series: Vec<TimeValue> = quotes.into_iter()
                .map(|q| TimeValue{ time: q.time, value: q.price } )
                .collect();

            let ts = TimeSeries{
                series: quote_series,
                title: ticker.name.clone()
            };
            let gaps = ts.find_gaps(&cal).unwrap_or(Vec::new());
            for gap in gaps {
                print!("ticker: {}, gap: {:?}", ticker.name, gap);
                // let res = market.update_quote_history(ticker_id, naive_date_to_date_time(&gap.0,0), naive_date_to_date_time(&gap.1, 23)).await;
                // if res.is_err() {
                //     println!(" failed :-(");
                // } else {
                //     println!(" succeeded!");
                // }
            }
        }
    }



    Ok(())
}

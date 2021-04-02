///! # qualinvest_core
///! 
///! This library is part of a set of tools for quantitative investments.
///! For mor information, see [qualinvest on github](https://github.com/xemwebe/qualinvest)
///!

use std::sync::Arc;
use std::str::FromStr;
use std::ops::Deref;

use serde::Deserialize;
use chrono::{DateTime, Utc};

use finql_data::QuoteHandler;
use finql::market::MarketError;
use finql::market_quotes::{MarketDataSource,MarketQuoteError};

pub mod accounts;
pub mod position;
pub mod read_pdf;
pub mod user;
pub mod postgres_user;

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
    pub host: String,
    pub name: String,
    pub user: String,
    pub password: String,
}

/// Parameters for PDF file parsing
#[derive(Debug, Deserialize)]
pub struct PdfParseParams {
    pub doc_path: String,
    pub warn_old: bool,
    pub consistency_check: bool,
    pub rename_asset: bool,
    pub default_account: bool,
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
    match token {
        Some(token) => {
            match source.get_provider(token.clone()) {
                Some(provider) => market.add_provider(source.to_string(), provider),
                None => (),
            };
        }
        None => (),
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
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    db: Arc<dyn QuoteHandler+Send+Sync>,
    config: &Config,
) -> Result<(), MarketError> {
    let mut market = finql::Market::new(db);
    set_market_providers(&mut market, &config.market_data);
    market.update_quote_history(ticker_id, start, end).await
}

pub async fn update_ticker(
    ticker_id: usize,
    db: Arc<dyn QuoteHandler+Send+Sync>,
    config: &Config,
) -> Result<(), MarketError> {
    let ticker = db.get_ticker_by_id(ticker_id).await?;
    let ticker_source = MarketDataSource::from_str(&ticker.source)
        .map_err(|_| MarketError::MarketQuoteError(MarketQuoteError::FetchFailed("invalid ticker source".to_string())))?;
    let token = match ticker_source {
        MarketDataSource::AlphaVantage => config.market_data.alpha_vantage_token.clone(),
        MarketDataSource::GuruFocus => config.market_data.gurufocus_token.clone(),
        MarketDataSource::EodHistData => config.market_data.eod_historical_data_token.clone(),
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
    config: &Config,
) -> Result<Vec<usize>, MarketError> {
    let mut market = finql::Market::new(db);
    set_market_providers(&mut market, &config.market_data);
    market.update_quotes().await
}

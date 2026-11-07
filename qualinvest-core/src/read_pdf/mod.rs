//! # Read pdf files and transform into plain text
//! This requires the extern tool `pdftotext`
//! which is part of [XpdfReader](https://www.xpdfreader.com/pdftotext-man.html).
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::{io, num, string};

use thiserror::Error;

use log::{debug, info};
use sanitize_filename::sanitize;

use finql::{
    datatypes::{Asset, CurrencyError, DataError, Transaction},
    Market,
};

type Result<T> = std::result::Result<T, ReadPDFError>;

use super::accounts::{Account, AccountHandler};
use crate::PdfParseParams;

mod comdirect;
mod ing_diba;
pub mod pdf_store;
mod read_account_info;
mod sparkasse;
pub mod utils;
use pdf_oxide::PdfDocument;
pub use pdf_store::sha256_hash;
use read_account_info::{parse_account_info, Bank};

/// Error related to market data object
#[derive(Error, Debug)]
pub enum ReadPDFError {
    #[error("Reading file failed")]
    IoError(#[from] io::Error),
    #[error("UTF8 parse error")]
    ParseError(#[from] string::FromUtf8Error),
    #[error("Error while parsing float")]
    ParseFloat(#[from] num::ParseFloatError),
    #[error("Failed to parse currency")]
    ParseCurrency(#[from] CurrencyError),
    #[error("Database error")]
    DBError(#[from] DataError),
    #[error("Currency mismatch")]
    CurrencyMismatch,
    #[error("Date parsing error")]
    ParseDate,
    #[error("Consistency check failed: {0}")]
    ConsistencyCheckFailed(String),
    #[error("File has already been parsed successfully")]
    AlreadyParsed,
    #[error("Critical keyword '{0}' could not be found")]
    NotFound(&'static str),
    #[error("Unknown document type")]
    UnknownDocumentType,
    #[error("No proper file name has been delivered")]
    MissingFileName,
    #[error("Asset '{0}' not found in database")]
    AssetNotFound(String),
    #[error("Market data error")]
    MarketError(#[from] finql::market::MarketError),
    #[error("Invalid date")]
    InvalidDate,
    #[error("PDF parsing failed")]
    PdfParsingFailed(#[from] pdf_oxide::Error),
    #[error("Invalid file name")]
    InvalidFileName,
    #[error("Unknown cash flow type {0}")]
    UnknownCashFlowType(String),
}

pub fn text_from_pdf(file: &Path) -> Result<String> {
    let output = Command::new("pdftotext")
        .arg("-layout")
        .arg("-q")
        .arg(file)
        .arg("-")
        .output()?;
    Ok(String::from_utf8(output.stdout)?)
}

pub trait ParsedDocument: std::fmt::Debug {
    fn add_account_info(&mut self, bank: Bank, account: &str);
    fn get_account_info(&self) -> Option<(String, String)>;
    fn check_consistency(&self) -> Result<()>;
    fn make_transactions(&self) -> Result<(Vec<Transaction>, Option<Asset>)>;
}

type ParsedInfo = Arc<dyn ParsedDocument>;

pub async fn parse(
    file: &Path,
    market: &Market,
    to_text: bool,
    out_folder: &Path,
) -> Result<ParsedInfo> {
    info!("parsing file {:?}", file.to_str());
    let doc = PdfDocument::open(file)?;
    let options = pdf_oxide::converters::ConversionOptions::default();
    let text = doc.to_plain_text_all(&options)?;
    if to_text {
        let base_file_name = file.file_stem().ok_or(ReadPDFError::InvalidFileName)?;
        std::fs::create_dir_all(&out_folder)?;
        let out_file = out_folder.join(&format!("{}.txt", base_file_name.to_string_lossy()));
        std::fs::write(&out_file, &text)?;
    }

    let account_info = parse_account_info(&text)?;
    debug!("Account: {}:{}", account_info.0, account_info.1);

    // Retrieve all transaction relevant data from pdf
    let mut parsed_document = match account_info.0 {
        Bank::Comdirect => comdirect::parse_document(&text, market).await?,
        Bank::SparkasseFrankfurt => sparkasse::parse_document(&text, market).await?,
        Bank::IngDiba => ing_diba::parse_transactions(&text, market).await?,
    };

    if let Some(doc) = Arc::get_mut(&mut parsed_document) {
        doc.add_account_info(account_info.0, &account_info.1);
    }

    debug!("parsed tranaction info: {parsed_document:#?}");

    Ok(parsed_document)
}

pub async fn store_parsed_pdf(
    hash: &str,
    file_name: &str,
    transaction_info: ParsedInfo,
    db: Arc<dyn AccountHandler + Send + Sync>,
    config: &PdfParseParams,
) -> Result<i32> {
    debug!("store_parsed_pdf called");
    let file_name = sanitize(file_name);
    if let Ok((ids, _path)) = db.lookup_hash(&hash).await {
        if !ids.is_empty() && config.warn_old {
            return Err(ReadPDFError::AlreadyParsed);
        }
    }

    // Find account or insert new one
    let acc_id = if let Some((broker, account_name)) = transaction_info.get_account_info() {
        let account = Account {
            id: None,
            broker: broker.to_string(),
            account_name: account_name.to_string(),
        };
        db.insert_account_if_new(&account)
            .await
            .map_err(ReadPDFError::DBError)?
    } else if let Some(default_account_id) = config.default_account {
        default_account_id
    } else {
        -1
    };
    debug!("Account id is {acc_id}");

    // If not disabled, perform consistency check
    if config.consistency_check {
        transaction_info.check_consistency()?;
    }
    debug!("Consistency check passed successfully");

    // Generate list of transactions
    let (transactions, asset) = transaction_info.make_transactions()?;
    let asset_id = if let Some(asset) = asset {
        Some(db.get_asset_id(&asset).await.ok_or_else(|| {
            ReadPDFError::AssetNotFound(match asset {
                Asset::Stock(stock) => stock.name,
                Asset::Currency(curr) => curr.to_string(),
            })
        })?)
    } else {
        None
    };
    debug!("Asset id is {asset_id:?}");
    let mut trans_ids = Vec::new();
    for trans in transactions {
        let mut trans = trans.clone();
        if let Some(asset_id) = asset_id {
            trans.set_asset_id(asset_id);
        }
        if !trans_ids.is_empty() {
            trans.set_transaction_ref(trans_ids[0]);
        }
        debug!("Try to write transaction {trans:?} to database");
        let trans_id = db.insert_transaction(&trans).await?;
        debug!("ID of newly inserted transaction: {trans_id}");
        trans_ids.push(trans_id);
        db.add_transaction_to_account(acc_id, trans_id).await?;
        debug!("transaction has been added to account");
    }
    let _ = db.insert_doc(&trans_ids, &hash, &file_name).await?;
    debug!("Hash of document has been stored");
    Ok(trans_ids.len() as i32)
}

//! # Read pdf files and transform into plain text
//! This requires the extern tool `pdftotext`
//! which is part of [XpdfReader](https://www.xpdfreader.com/pdftotext-man.html).
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::{io, num, string};

use thiserror::Error;

use log::{debug, error, info, trace};
use sanitize_filename::sanitize;
use time::{macros::format_description, Date};

use finql::{
    datatypes::{
        date_time_helper::make_offset_time, Asset, CashAmount, CashFlow, CurrencyError, DataError,
        Transaction, TransactionType,
    },
    fx_rates::SimpleCurrencyConverter,
    Market,
};

type Result<T> = std::result::Result<T, ReadPDFError>;

use super::accounts::{Account, AccountHandler};
use crate::PdfParseParams;

pub mod pdf_store;
mod read_account_info;
mod read_transactions;
use pdf_oxide::PdfDocument;
pub use pdf_store::sha256_hash;
use read_account_info::parse_account_info;
use read_transactions::parse_transactions;

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
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum DocumentType {
    Buy,
    Sell,
    Dividend,
    Tax,
    Interest,
    BondPayBack,
}

// Collect all parsed data that is required to construct by category distinct cash flow transactions
#[derive(Debug)]
pub struct ParsedTransactionInfo {
    doc_type: DocumentType,
    asset: Asset,
    position: f64,
    valuta: Date,
    fx_rate: Option<f64>,
    main_amount: CashAmount,
    total_amount: CashAmount,
    extra_fees: Vec<CashAmount>,
    extra_taxes: Vec<CashAmount>,
    accruals: Vec<CashAmount>,
    note: Option<String>,
    account_info: Option<(String, String)>,
}

impl ParsedTransactionInfo {
    fn new(
        doc_type: DocumentType,
        asset: Asset,
        main_amount: CashAmount,
        total_amount: CashAmount,
        fx_rate: Option<f64>,
        valuta: Date,
    ) -> ParsedTransactionInfo {
        ParsedTransactionInfo {
            doc_type,
            asset,
            position: 0.0,
            valuta,
            fx_rate,
            main_amount,
            total_amount,
            extra_fees: Vec::new(),
            extra_taxes: Vec::new(),
            accruals: Vec::new(),
            note: None,
            account_info: None,
        }
    }
}

pub fn rounded_equal(x: f64, y: f64, precision: i32) -> bool {
    let factor = 10.0_f64.powi(precision);
    ((x * factor).round() - (y * factor).round()).abs() < 1.0
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

/// Convert a string with German number convention
/// (e.g. '.' as thousands separator and ',' as decimal separator)
pub fn german_string_to_float(num_string: &str) -> Result<f64> {
    let sign_less_string = num_string.replace('-', "");
    let positive = sign_less_string == num_string;
    let result = sign_less_string
        .trim()
        .replace('.', "")
        .replace(',', ".")
        .parse()
        .map_err(ReadPDFError::ParseFloat);
    match result {
        Ok(num) => {
            if positive {
                Ok(num)
            } else {
                Ok(-num)
            }
        }
        Err(err) => Err(err),
    }
}

/// Converts strings in German data convention to Date
pub fn german_string_to_date(date_string: &str) -> Result<Date> {
    let format = format_description!("[day].[month].[year]");
    Date::parse(date_string, &format).map_err(|_| ReadPDFError::ParseDate)
}

pub async fn parse(
    file: &Path,
    market: &Market,
    to_text: bool,
    out_folder: &Path,
) -> Result<ParsedTransactionInfo> {
    info!("parsing file {:?}", file.to_str());
    let doc = PdfDocument::open(file)?;
    let options = pdf_oxide::converters::ConversionOptions::default();
    let text = doc.to_markdown_all(&options)?;
    if to_text {
        let base_file_name = file.file_stem().ok_or(ReadPDFError::InvalidFileName)?;
        std::fs::create_dir_all(&out_folder)?;
        let out_file = out_folder.join(&format!("{}.txt", base_file_name.to_string_lossy()));
        std::fs::write(&out_file, &text)?;
    }

    let account_info = parse_account_info(&text)?;
    debug!("Account: {}:{}", account_info.0, account_info.1);

    // Retrieve all transaction relevant data from pdf
    let mut transaction_info = parse_transactions(&text, market).await?;
    transaction_info.account_info = Some(account_info);

    Ok(transaction_info)
}

pub async fn store_parsed_pdf(
    hash: &str,
    file_name: &str,
    transaction_info: &ParsedTransactionInfo,
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
    let acc_id = if let Some(account_info) = &transaction_info.account_info {
        let (broker, account_name) = account_info;
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

    // If not disabled, perform consistency check
    if config.consistency_check {
        check_consistency(transaction_info).await?;
    }
    // Generate list of transactions
    let (transactions, asset) = make_transactions(transaction_info).await?;
    let asset_id = db.get_asset_id(&asset).await.ok_or_else(|| {
        ReadPDFError::AssetNotFound(match asset {
            Asset::Stock(stock) => stock.name,
            Asset::Currency(curr) => curr.to_string(),
        })
    })?;
    let mut trans_ids = Vec::new();
    for trans in transactions {
        let mut trans = trans.clone();
        trans.set_asset_id(asset_id);
        if !trans_ids.is_empty() {
            trans.set_transaction_ref(trans_ids[0]);
        }
        let trans_id = db.insert_transaction(&trans).await?;
        trans_ids.push(trans_id);
        db.add_transaction_to_account(acc_id, trans_id).await?;
    }
    let _ = db.insert_doc(&trans_ids, &hash, &file_name).await?;
    Ok(trans_ids.len() as i32)
}

// Check if main payment plus all fees and taxes add up to total payment
// Add up all payments separate by currencies, convert into total currency, and check if they add up to zero.
pub async fn check_consistency(tri: &ParsedTransactionInfo) -> Result<()> {
    debug!("check_consistency called");
    let time = make_offset_time(
        tri.valuta.year(),
        tri.valuta.month() as u32,
        tri.valuta.day() as u32,
        18,
        0,
        0,
    )
    .ok_or(ReadPDFError::ParseDate)?;
    trace!("closing time is {time}");

    // temporary storage for fx rates
    // total payment is always in base currency, but main_amount (and maybe fees or taxes) could be in foreign currency.
    let mut fx_converter = SimpleCurrencyConverter::new();
    if let Some(fx_rate) = tri.fx_rate {
        trace!("fx rate to be added to fx converter: {fx_rate}");
        fx_converter.insert_fx_rate(tri.total_amount.currency, tri.main_amount.currency, fx_rate);
    }

    // Add up all payment components and check whether they equal the final payment
    let mut check_sum = -tri.total_amount;
    let mut foreign_check_sum = tri.main_amount;
    for fee in &tri.extra_fees {
        add_by_currency(fee, &mut check_sum, &mut foreign_check_sum);
    }
    for tax in &tri.extra_taxes {
        add_by_currency(tax, &mut check_sum, &mut foreign_check_sum);
    }
    for accrued in &tri.accruals {
        add_by_currency(accrued, &mut check_sum, &mut foreign_check_sum);
    }
    trace!("foreign check sum: {foreign_check_sum}, time: {time}");
    check_sum
        .add(foreign_check_sum, time, &fx_converter, true)
        .await?;
    trace!("check_sum after adding foreign check sum is {check_sum}");
    // Final sum should be nearly zero
    if !rounded_equal(check_sum.amount, 0.0, 4) {
        let warning = format!(
            "Sum of payments does not equal total payments, difference is {}.",
            check_sum.amount
        );
        error!("consistency check failed");
        Err(ReadPDFError::ConsistencyCheckFailed(warning))
    } else {
        debug!("consistency check was successfull");
        Ok(())
    }
}

// Transaction in foreign currency will be converted to currency of total payment amount
pub async fn make_transactions(tri: &ParsedTransactionInfo) -> Result<(Vec<Transaction>, Asset)> {
    debug!("make_transactions called");
    let mut transactions = Vec::new();
    let time = make_offset_time(
        tri.valuta.year(),
        tri.valuta.month() as u32,
        tri.valuta.day() as u32,
        18,
        0,
        0,
    )
    .ok_or(ReadPDFError::InvalidDate)?;

    // temporary storage for fx rates
    // total payment is always in base currency, but main_amount (and maybe fees or taxes) could be in foreign currency.
    let mut fx_converter = SimpleCurrencyConverter::new();
    if tri.fx_rate.is_some() {
        fx_converter.insert_fx_rate(
            tri.total_amount.currency,
            tri.main_amount.currency,
            tri.fx_rate.unwrap(),
        );
    }

    // Construct main transaction
    if tri.main_amount.amount != 0.0 {
        transactions.push(Transaction {
            id: None,
            transaction_type: match tri.doc_type {
                DocumentType::Buy | DocumentType::Sell | DocumentType::BondPayBack => {
                    TransactionType::Asset {
                        asset_id: 0,
                        position: tri.position,
                    }
                }
                DocumentType::Dividend => TransactionType::Dividend { asset_id: 0 },
                DocumentType::Interest => TransactionType::Interest { asset_id: 0 },
                DocumentType::Tax => TransactionType::Tax {
                    transaction_ref: None,
                },
            },
            cash_flow: CashFlow {
                amount: tri.main_amount,
                date: tri.valuta,
            },
            note: tri.note.clone(),
        });
    } else {
        // No main transaction, nothing todo
        return Ok((transactions, tri.asset.clone()));
    }

    let mut total_fee = CashAmount {
        amount: 0.0,
        currency: tri.total_amount.currency,
    };
    for fee in &tri.extra_fees {
        total_fee.add(*fee, time, &fx_converter, true).await?;
    }
    if total_fee.amount != 0.0 {
        transactions.push(Transaction {
            id: None,
            transaction_type: TransactionType::Fee {
                transaction_ref: None,
            },
            cash_flow: CashFlow {
                amount: total_fee,
                date: tri.valuta,
            },
            note: None,
        });
    }

    let mut total_tax = CashAmount {
        amount: 0.0,
        currency: tri.total_amount.currency,
    };
    for tax in &tri.extra_taxes {
        total_tax.add(*tax, time, &fx_converter, true).await?;
    }
    if total_tax.amount != 0.0 {
        transactions.push(Transaction {
            id: None,
            transaction_type: TransactionType::Tax {
                transaction_ref: None,
            },
            cash_flow: CashFlow {
                amount: total_tax,
                date: tri.valuta,
            },
            note: None,
        });
    }

    let mut total_accrued = CashAmount {
        amount: 0.0,
        currency: tri.total_amount.currency,
    };
    for accrued in &tri.accruals {
        total_accrued
            .add(*accrued, time, &fx_converter, true)
            .await
            .map_err(|_| ReadPDFError::CurrencyMismatch)?;
    }
    if total_accrued.amount != 0.0 {
        transactions.push(Transaction {
            id: None,
            transaction_type: TransactionType::Interest { asset_id: 0 },
            cash_flow: CashFlow {
                amount: total_accrued,
                date: tri.valuta,
            },
            note: None,
        });
    }

    // Ensure that sum of payments equal total payments in spite of rounding errors
    transactions[0].cash_flow.amount.amount =
        tri.total_amount.amount - total_accrued.amount - total_tax.amount - total_fee.amount;
    transactions[0].cash_flow.amount.currency = tri.total_amount.currency;

    Ok((transactions, tri.asset.clone()))
}

fn add_by_currency(
    new_amount: &CashAmount,
    base_amount: &mut CashAmount,
    foreign_amount: &mut CashAmount,
) {
    if new_amount.currency == base_amount.currency {
        base_amount.amount += new_amount.amount;
    } else {
        foreign_amount.amount += new_amount.amount;
    }
}

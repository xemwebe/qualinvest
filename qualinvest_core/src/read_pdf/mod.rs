//! # Read pdf files and transform into plain text
//! This requires the extern tool `pdftotext`
//! which is part of [XpdfReader](https://www.xpdfreader.com/pdftotext-man.html).
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::{io, num, string};

use thiserror::Error;

use chrono::{Datelike, Local, NaiveDate, TimeZone};
use sanitize_filename::sanitize;

use finql::{
    datatypes::{
        date_time_helper::{convert_local_result_to_datetime as to_datetime, make_time},
        Asset, CashAmount, CashFlow, CurrencyError, DataError, Transaction, TransactionType,
    },
    fx_rates::SimpleCurrencyConverter,
    Market,
};

use super::accounts::{Account, AccountHandler};
use crate::PdfParseParams;

pub mod pdf_store;
mod read_account_info;
mod read_transactions;
pub use pdf_store::{sha256_hash, store_pdf_as_name};
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
pub struct ParsedTransactionInfo {
    doc_type: DocumentType,
    asset: Asset,
    position: f64,
    valuta: NaiveDate,
    fx_rate: Option<f64>,
    main_amount: CashAmount,
    total_amount: CashAmount,
    extra_fees: Vec<CashAmount>,
    extra_taxes: Vec<CashAmount>,
    accruals: Vec<CashAmount>,
    note: Option<String>,
}

impl ParsedTransactionInfo {
    fn new(
        doc_type: DocumentType,
        asset: Asset,
        main_amount: CashAmount,
        total_amount: CashAmount,
        fx_rate: Option<f64>,
        valuta: NaiveDate,
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
        }
    }
}

pub fn rounded_equal(x: f64, y: f64, precision: i32) -> bool {
    let factor = 10.0_f64.powi(precision);
    ((x * factor).round() - (y * factor).round()).abs() < 1.0
}

pub fn text_from_pdf(file: &Path) -> Result<String, ReadPDFError> {
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
pub fn german_string_to_float(num_string: &str) -> Result<f64, ReadPDFError> {
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

/// Converts strings in German data convention to NaiveDate
pub fn german_string_to_date(date_string: &str) -> Result<NaiveDate, ReadPDFError> {
    NaiveDate::parse_from_str(date_string, "%d.%m.%Y").map_err(|_| ReadPDFError::ParseDate)
}

pub async fn parse_and_store<'a>(
    path: &'a Path,
    file_name: &'a str,
    db: Arc<dyn AccountHandler + Send + Sync>,
    config: &'a PdfParseParams,
    market: &'a Market,
) -> Result<i32, ReadPDFError> {
    let file_name = sanitize(file_name);
    let hash = sha256_hash(path)?;
    if let Ok((ids, _path)) = db.lookup_hash(&hash).await {
        if !ids.is_empty() && config.warn_old {
            return Err(ReadPDFError::AlreadyParsed);
        }
    }

    // Start parsing document
    let text = text_from_pdf(path);
    match text {
        Ok(text) => {
            let account_info = parse_account_info(&text);

            let acc_id = if account_info.is_err() && config.default_account.is_some() {
                config.default_account.unwrap()
            } else {
                let (broker, account_name) = account_info?;
                let account = Account {
                    id: None,
                    broker,
                    account_name,
                };
                db.insert_account_if_new(&account)
                    .await
                    .map_err(ReadPDFError::DBError)?
            };

            // Retrieve all transaction relevant data from pdf
            let tri = parse_transactions(&text, market).await?;
            // If not disabled, perform consistency check
            if config.consistency_check {
                check_consistency(&tri).await?;
            }
            // Generate list of transactions
            let transactions_info = make_transactions(&tri).await;
            match transactions_info {
                Ok((transactions, asset)) => {
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
                    store_pdf_as_name(path, &file_name, &hash, config).await?;
                    let doc_ids = db.insert_doc(&trans_ids, &hash, &file_name).await?;
                    let buffer = std::fs::read(path).unwrap();
                    for id in doc_ids {
                        db.store_pdf(id, &buffer).await.unwrap();
                    }
                    Ok(trans_ids.len() as i32)
                }
                Err(err) => Err(err),
            }
        }
        Err(err) => Err(err),
    }
}

// Check if main payment plus all fees and taxes add up to total payment
// Add up all payments separate by currencies, convert into total currency, and check if they add up to zero.
pub async fn check_consistency(tri: &ParsedTransactionInfo) -> Result<(), ReadPDFError> {
    let time = make_time(
        tri.valuta.year(),
        tri.valuta.month(),
        tri.valuta.day(),
        18,
        0,
        0,
    )
    .ok_or(ReadPDFError::ParseDate)?;

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
    check_sum
        .add(foreign_check_sum, time, &fx_converter, true)
        .await?;

    // Final sum should be nearly zero
    if !rounded_equal(check_sum.amount, 0.0, 4) {
        let warning = format!(
            "Sum of payments does not equal total payments, difference is {}.",
            check_sum.amount
        );
        Err(ReadPDFError::ConsistencyCheckFailed(warning))
    } else {
        Ok(())
    }
}

// Transaction in foreign currency will be converted to currency of total payment amount
pub async fn make_transactions(
    tri: &ParsedTransactionInfo,
) -> Result<(Vec<Transaction>, Asset), ReadPDFError> {
    let mut transactions = Vec::new();
    let time = to_datetime(Local.with_ymd_and_hms(
        tri.valuta.year(),
        tri.valuta.month(),
        tri.valuta.day(),
        18,
        0,
        0,
    ))
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

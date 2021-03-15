///! # Read pdf files and transform into plain text
///! This requires the extern tool `pdftotext`
///! which is part of [XpdfReader](https://www.xpdfreader.com/pdftotext-man.html).
use super::accounts::{Account, AccountHandler};
use std::error::Error;
use std::process::Command;
use std::{fmt, io, num, string};

use chrono::NaiveDate;
use chrono::{Datelike, TimeZone, Utc};
use finql_data::{Asset, Transaction, TransactionType, CashAmount, CashFlow, DataError, CurrencyError};
use finql::fx_rates::SimpleCurrencyConverter;

use crate::PdfParseParams;
use pdf_store::store_pdf;

pub mod pdf_store;
mod read_account_info;
mod read_transactions;
pub use pdf_store::sha256_hash;
use read_account_info::parse_account_info;
use read_transactions::parse_transactions;

#[derive(Debug)]
pub enum ReadPDFError {
    IoError(io::Error),
    ParseError(string::FromUtf8Error),
    ParseFloat(num::ParseFloatError),
    ParseCurrency(CurrencyError),
    DBError(DataError),
    CurrencyMismatch,
    ParseDate,
    ConsistencyCheckFailed(String),
    AlreadyParsed,
    NotFound(&'static str),
    UnknownDocumentType,
}

impl fmt::Display for ReadPDFError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Conversion of pdf to text failed.")
    }
}

impl Error for ReadPDFError {
    fn cause(&self) -> Option<&dyn Error> {
        match self {
            Self::IoError(err) => Some(err),
            Self::ParseError(err) => Some(err),
            Self::ParseFloat(err) => Some(err),
            Self::ParseCurrency(err) => Some(err),
            Self::DBError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::string::FromUtf8Error> for ReadPDFError {
    fn from(error: string::FromUtf8Error) -> Self {
        Self::ParseError(error)
    }
}

impl From<io::Error> for ReadPDFError {
    fn from(error: io::Error) -> Self {
        Self::IoError(error)
    }
}

impl From<DataError> for ReadPDFError {
    fn from(error: DataError) -> Self {
        Self::DBError(error)
    }
}

impl From<CurrencyError> for ReadPDFError {
    fn from(error: CurrencyError) -> Self {
        Self::ParseCurrency(error)
    }
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
    return (x * factor).round() == (y * factor).round();
}

pub fn text_from_pdf(file: &str) -> Result<String, ReadPDFError> {
    let output = Command::new("pdftotext")
        .arg("-layout")
        .arg("-q")
        .arg(&file)
        .arg("-")
        .output()?;
    Ok(String::from_utf8(output.stdout)?)
}

/// Convert a string with German number convention
/// (e.g. '.' as thousands separator and ',' as decimal separator)
pub fn german_string_to_float(num_string: &str) -> Result<f64, ReadPDFError> {
    let sign_less_string = num_string.replace("-", "");
    let positive = if sign_less_string != num_string {
        false
    } else {
        true
    };
    let result = sign_less_string
        .trim()
        .replace(".", "")
        .replace(",", ".")
        .parse()
        .map_err(|err| ReadPDFError::ParseFloat(err));
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

pub async fn parse_and_store<DB: AccountHandler>(
    pdf_file: &str,
    db: &mut DB,
    config: &PdfParseParams,
) -> Result<i32, ReadPDFError> {
    let hash = sha256_hash(pdf_file)?;
    match db.lookup_hash(&hash).await {
        Ok((ids, _path)) => {
            if ids.len() > 0 {
                if config.warn_old {
                    return Err(ReadPDFError::AlreadyParsed);
                }
                return Ok(0);
            }
        }
        Err(_) => {}
    }
    //println!("Start parsing document {}", pdf_file);
    let text = text_from_pdf(pdf_file);
    match text {
        Ok(text) => {
            let account_info = parse_account_info(&text);
            let (broker, account_name) = if account_info.is_err() && config.default_account {
                ("nobroker".to_string(), "unassigned".to_string())
            } else {
                account_info?
            };
            let mut account = Account {
                id: None,
                broker,
                account_name,
            };
            let acc_id = db
                .insert_account_if_new(&account).await
                .map_err(|err| ReadPDFError::DBError(err))?;
            account.id = Some(acc_id);

            // Retrieve all transaction relevant data from pdf
            let tri = parse_transactions(&text)?;
            // If not disable, perform consistency check
            if config.consistency_check {
                check_consistency(&tri).await?;
            }
            // Generate list of transactions
            let transactions = make_transactions(&tri).await;
            let trans_ids = match transactions {
                Ok((transactions, asset)) => {
                    let asset_id = if asset.name == "" {
                        db.get_asset_by_isin(&asset.isin.unwrap()).await
                            .map_err(|_| ReadPDFError::NotFound("could not find ISIN in db"))?
                            .id
                            .unwrap()
                    } else {
                        db.insert_asset_if_new(&asset, config.rename_asset).await
                            .map_err(|err| ReadPDFError::DBError(err))?
                    };
                    let mut trans_ids = Vec::new();
                    for trans in transactions {
                        let mut trans = trans.clone();
                        trans.set_asset_id(asset_id);
                        if trans_ids.len() > 0 {
                            trans.set_transaction_ref(trans_ids[0]);
                        }
                        let trans_id = db
                            .insert_transaction(&trans).await
                            .map_err(|err| ReadPDFError::DBError(err))?;
                        trans_ids.push(trans_id);
                        let _ = db
                            .add_transaction_to_account(acc_id, trans_id).await
                            .map_err(|err| ReadPDFError::DBError(err))?;
                    }
                    Ok(trans_ids)
                }
                Err(err) => Err(err),
            }?;
            let name = store_pdf(pdf_file, &hash, &config)?;
            db.insert_doc(&trans_ids, &hash, &name).await?;
            Ok(trans_ids.len() as i32)
        }
        Err(err) => Err(err),
    }
}

// Check if main payment plus all fees and taxes add up to total payment
// Add up all payments separate by currencies, convert into total currency, and check if the add up to zero.
pub async fn check_consistency(tri: &ParsedTransactionInfo) -> Result<(), ReadPDFError> {
    let time = Utc
        .ymd(tri.valuta.year(), tri.valuta.month(), tri.valuta.day())
        .and_hms_milli(18, 0, 0, 0);

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
    check_sum.add(foreign_check_sum, time, &mut fx_converter, true).await?;

    // Final sum should be nearly zero
    if !rounded_equal(check_sum.amount, 0.0, 4) {
        let warning = format!(
            "Sum of payments does not equal total payments, difference is {}.",
            check_sum.amount
        );
        return Err(ReadPDFError::ConsistencyCheckFailed(warning));
    } else {
        Ok(())
    }
}

// Transaction in foreign currency will be converted to currency of total payment amount
pub async fn make_transactions(
    tri: &ParsedTransactionInfo,
) -> Result<(Vec<Transaction>, Asset), ReadPDFError> {
    let mut transactions = Vec::new();
    let time = Utc
        .ymd(tri.valuta.year(), tri.valuta.month(), tri.valuta.day())
        .and_hms_milli(18, 0, 0, 0);

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
        total_fee.add(*fee, time, &mut fx_converter, true).await?;
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
        total_tax.add(*tax, time, &mut fx_converter, true).await?;
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
        total_accrued.add(*accrued, time, &mut fx_converter, true).await
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

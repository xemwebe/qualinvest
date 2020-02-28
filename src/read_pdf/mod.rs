///! # Read pdf files and transform into plain text
///! This requires the extern tool `pdftotext`
///! which is part of [XpdfReader](https://www.xpdfreader.com/pdftotext-man.html).
use super::accounts::{Account, AccountHandler};
use chrono::NaiveDate;
use finql::currency;
use finql::data_handler::DataError;
use std::error::Error;
use std::fmt;
use std::io;
use std::num;
use std::process::Command;
use std::string;

mod read_account_info;
mod read_transactions;
pub mod pdf_store;
use read_account_info::parse_account_info;
use read_transactions::parse_transactions;

#[derive(Debug)]
pub enum ReadPDFError {
    IoError(io::Error),
    ParseError(string::FromUtf8Error),
    ParseFloat(num::ParseFloatError),
    ParseCurrency(currency::CurrencyError),
    DBError(DataError),
    CurrencyMismatch,
    ParseDate,
    ConsistencyCheckFailed,
    NotFound(&'static str),
}

impl fmt::Display for ReadPDFError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Conversion of pdf to text failed.")
    }
}

impl Error for ReadPDFError {
    fn cause(&self) -> Option<&dyn Error> {
        Some(self)
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
    num_string
        .replace(".", "")
        .replace(",", ".")
        .parse()
        .map_err(|err| ReadPDFError::ParseFloat(err))
}

/// Converts strings in German data convention to NaiveDate
pub fn german_string_to_date(date_string: &str) -> Result<NaiveDate, ReadPDFError> {
    NaiveDate::parse_from_str(date_string, "%d.%m.%Y").map_err(|_| ReadPDFError::ParseDate)
}

pub fn parse_and_store<DB: AccountHandler>(
    pdf_file: &str,
    db: &mut DB,
    debug: bool,
) -> Result<i32, ReadPDFError> {
    let text = text_from_pdf(pdf_file);
    match text {
        Ok(text) => {
            let (broker, account_id) = parse_account_info(&text)?;
            let mut account = Account {
                id: None,
                broker,
                account_id,
            };
            let acc_id = db
                .insert_account_if_new(&account)
                .map_err(|err| ReadPDFError::DBError(err))?;
            account.id = Some(acc_id);
            let transactions = parse_transactions(&text, debug);
            match transactions {
                Ok((transactions, asset)) => {
                    let asset_id = db
                        .insert_asset_if_new(&asset)
                        .map_err(|err| ReadPDFError::DBError(err))?;
                    let mut count = 0;
                    let mut trans_id: usize = 0;
                    for trans in transactions {
                        let mut trans = trans.clone();
                        trans.set_asset_id(asset_id);
                        if trans_id != 0 {
                            trans.set_transaction_ref(trans_id);
                        }
                        trans_id = db
                            .insert_transaction(&trans)
                            .map_err(|err| ReadPDFError::DBError(err))?;
                        let _ = db
                            .add_transaction_to_account(acc_id, trans_id)
                            .map_err(|err| ReadPDFError::DBError(err))?;
                        count += 1;
                    }
                    Ok(count)
                }
                Err(err) => Err(err),
            }
        }

        Err(err) => Err(err),
    }
}

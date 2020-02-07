use super::german_string_to_date;
use super::german_string_to_float;
use super::ReadPDFError;
use finql::asset::Asset;
///! Parse text files and extract asset and transaction data
///! Currently, only the most simple transaction information from comdirect bank are supported
use finql::transaction::{Transaction, TransactionType};
use finql::Amount;
use finql::CashFlow;
use finql::Currency;
use regex::Regex;
use std::str::FromStr;

/// Extract asset information from text file
pub fn parse_asset(text: &str) -> Result<Asset, ReadPDFError> {
    lazy_static! {
        static ref NAME_WKN_ISIN: Regex = Regex::new(
            r"(?m)WPKNR/ISIN\n(.*)\s\s\s*([A-Z0-9]{6})\s*\n\s*(.*)\s\s\s*([A-Z0-9]{12})"
        )
        .unwrap();
    }
    let cap = NAME_WKN_ISIN.captures(text);
    match cap {
        None => Err(ReadPDFError::NotFound),
        Some(cap) => {
            let wkn = Some(cap[2].to_string());
            let isin = Some(cap[4].to_string());
            let name = format!("{} {}", cap[1].trim(), cap[3].trim());
            Ok(Asset {
                id: None,
                name,
                wkn,
                isin,
                note: None,
            })
        }
    }
}

/// Extract transaction information from text files
pub fn parse_transactions(text: &str) -> Result<(Vec<Transaction>, Asset), ReadPDFError> {
    let asset = parse_asset(text)?;
    lazy_static! {
        static ref BUY: Regex = Regex::new(r"Wertpapierkauf").unwrap();
        static ref POSITION: Regex = Regex::new(r"St.\s*([-0-9.,]*)").unwrap();
        static ref PRE_TAX_AMOUNT: Regex = Regex::new(
            r"(?m)Zu Ihren Lasten vor Steuern\s*\n.*\s*([0-9.]{10})\s*([A-Z]{3})\s*([-0-9.,]*)"
        )
        .unwrap();
        static ref AFTER_TAX_AMOUNT: Regex =
            Regex::new(r"Zu Ihren Lasten nach Steuern: *([A-Z]{3}) *([-0-9.,]*)").unwrap();
    }
    let mut transactions = Vec::new();
    if BUY.is_match(text) {
        let position = POSITION.captures(text);
        let position = match position {
            None => Err(ReadPDFError::NotFound),
            Some(position) => german_string_to_float(&position[1]),
        }?;
        let pre_tax = PRE_TAX_AMOUNT.captures(text);
        let (pre_tax, valuta) = match pre_tax {
            None => Err(ReadPDFError::NotFound),
            Some(cap) => {
                // buy cash flows are negative, therefore reverse sign
                let amount = -german_string_to_float(&cap[3])?;
                let currency =
                    Currency::from_str(&cap[2]).map_err(|err| ReadPDFError::ParseCurrency(err))?;
                let valuta = german_string_to_date(&cap[1])?;
                Ok((Amount { amount, currency }, valuta))
            }
        }?;
        let after_tax = AFTER_TAX_AMOUNT.captures(text);
        let _after_tax = match after_tax {
            None => Err(ReadPDFError::NotFound),
            Some(cap) => {
                let amount = german_string_to_float(&cap[2])?;
                let currency =
                    Currency::from_str(&cap[1]).map_err(|err| ReadPDFError::ParseCurrency(err))?;
                Ok(Amount { amount, currency })
            }
        }?;
        transactions.push(Transaction {
            id: None,
            transaction_type: TransactionType::Asset {
                asset_id: 0,
                position,
            },
            cash_flow: CashFlow {
                amount: pre_tax,
                date: valuta,
            },
            note: None,
        })
    }
    Ok((transactions, asset))
}

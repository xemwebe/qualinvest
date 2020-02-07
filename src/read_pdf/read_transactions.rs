///! Parse text files and extract asset and transaction data
///! Currently, only the most simple transaction information from comdirect bank are supported
use super::german_string_to_date;
use super::german_string_to_float;
use super::ReadPDFError;
use finql::asset::Asset;
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
        None => Err(ReadPDFError::NotFound("asset")),
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

fn parse_amount(regex: &Regex, text: &str) -> Result<Option<Amount>, ReadPDFError> {
    match regex.captures(text) {
        None => Ok(None),
        Some(cap) => {
            let amount = german_string_to_float(&cap[2])?;
            let currency =
                Currency::from_str(&cap[1]).map_err(|err| ReadPDFError::ParseCurrency(err))?;
            Ok(Some(Amount { amount, currency }))
        }
    }
}

fn add_opt_amounts(x: Option<Amount>, y: Option<Amount>) -> Result<Option<Amount>, ReadPDFError> {
    match x {
        None => Ok(y),
        Some(x_amount) => match y {
            None => Ok(x),
            Some(y_amount) => {
                if x_amount.currency != y_amount.currency {
                    Err(ReadPDFError::CurrencyMismatch)
                } else {
                    Ok(Some(Amount {
                        amount: x_amount.amount + y_amount.amount,
                        currency: x_amount.currency,
                    }))
                }
            }
        },
    }
}

fn must_have(amount: Option<Amount>, error_message: &'static str) -> Result<Amount, ReadPDFError> {
    match amount {
        None => Err(ReadPDFError::NotFound(error_message)),
        Some(amount) => Ok(amount),
    }
}

/// Extract transaction information from text files
pub fn parse_transactions(text: &str) -> Result<(Vec<Transaction>, Asset), ReadPDFError> {
    let asset = parse_asset(text)?;
    lazy_static! {
        static ref BUY: Regex = Regex::new(r"Wertpapierkauf").unwrap();
        static ref POSITION: Regex = Regex::new(r"St.\s+([0-9.,]+)").unwrap();
        static ref PRE_TAX_AMOUNT: Regex = Regex::new(
            r"(?m)Zu Ihren Lasten vor Steuern\s*\n.*\s*([0-9.]{10})\s*([A-Z]{3})\s*([-0-9.,]*)"
        )
        .unwrap();
        static ref TRADE_VALUE: Regex =
            Regex::new(r"Kurswert\s*:\s+([A-Z]{3})\s+([-0-9,.]*)").unwrap();
        static ref PROVISION: Regex =
            Regex::new(r"Provision\s*:\s+([A-Z]{3})\s+([-0-9,.]*)").unwrap();
        static ref EXCHANGE_FEE: Regex =
            Regex::new(r"Börsenplatzabhäng. Entgelt\s*:\s+([A-Z]{3})\s+([-0-9,.]*)").unwrap();
        static ref AFTER_TAX_AMOUNT: Regex =
            Regex::new(r"Zu Ihren Lasten nach Steuern: *([A-Z]{3}) *([-0-9.,]*)").unwrap();
    }
    let mut transactions = Vec::new();
    if BUY.is_match(text) {
        let position = match POSITION.captures(text) {
            None => Err(ReadPDFError::NotFound("position")),
            Some(position) => german_string_to_float(&position[1]),
        }?;

        let (pre_tax, valuta) = match PRE_TAX_AMOUNT.captures(text) {
            None => Err(ReadPDFError::NotFound("pre-tax amount")),
            Some(cap) => {
                // buy cash flows are negative, therefore reverse sign
                let amount = german_string_to_float(&cap[3])?;
                let currency =
                    Currency::from_str(&cap[2]).map_err(|err| ReadPDFError::ParseCurrency(err))?;
                let valuta = german_string_to_date(&cap[1])?;
                Ok((Amount { amount, currency }, valuta))
            }
        }?;

        let provision = parse_amount(&PROVISION, text)?;
        let exchange_fee = parse_amount(&EXCHANGE_FEE, text)?;

        let total_fee = add_opt_amounts(provision, exchange_fee)?;

        // Do some consistency checks to verify if implicit assumptions are correct
        // These should probably be disabled once parsing is complete
        let after_tax = parse_amount(&AFTER_TAX_AMOUNT, text)?;
        let trade_value = parse_amount(&TRADE_VALUE, text)?;
        let pre_tax_calculated = add_opt_amounts(trade_value, total_fee)?;
        let trade_value = must_have(trade_value, "trade value")?;

        if after_tax.is_some() {
            if after_tax.unwrap().amount != -pre_tax.amount {
                println!("After and pre-tax values differ, paid tax on buy?");
            }
        }

        if pre_tax_calculated.is_some() {
            if pre_tax_calculated.unwrap().amount != pre_tax.amount {
                println!("Calculated pre-tax value differs from reported pre-tax value: missed some fees or taxes?")
            }
        }
        // End of consistency checks

        transactions.push(Transaction {
            id: None,
            transaction_type: TransactionType::Asset {
                asset_id: 0,
                position,
            },
            cash_flow: CashFlow {
                amount: Amount{ amount: -trade_value.amount, currency: trade_value.currency },
                date: valuta,
            },
            note: None,
        });

        if total_fee.is_some() {
            let total_fee = total_fee.unwrap();
            // Add fee transaction
            transactions.push(Transaction {
                id: None,
                transaction_type: TransactionType::Fee {
                    transaction_ref: None,
                },
                cash_flow: CashFlow {
                    amount: Amount {
                        amount: -total_fee.amount,
                        currency: total_fee.currency,
                    },
                    date: valuta,
                },
                note: None,
            });
        }
    }
    Ok((transactions, asset))
}

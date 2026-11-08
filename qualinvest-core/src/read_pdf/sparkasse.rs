//! Parse text files and extract cash flows from Sparkassenkontoauszug
use crate::read_pdf::utils::{german_string_to_date, german_string_to_float};
use std::sync::Arc;
use time::Date;

use super::{Bank, ParsedDocument, ParsedInfo, ReadPDFError};
use finql::{
    datatypes::{Asset, CashAmount, CashFlow, Currency, Transaction, TransactionType},
    Market,
};
use lazy_static::lazy_static;
use log::debug;
use regex::Regex;

type Result<T> = std::result::Result<T, ReadPDFError>;

#[derive(Debug)]
enum TransferType {
    DauerAuftrag,
    DebitCardPayment,
    Lastschrift,
}

impl TransferType {
    fn from_str(s: &str) -> Result<Self> {
        Ok(match s {
            "Dauerauftrag" => TransferType::DauerAuftrag,
            "Debitkartenzahlung" => TransferType::DebitCardPayment,
            "Lastschrift" => TransferType::Lastschrift,
            _ => {
                return Err(ReadPDFError::UnknownCashFlowType(s.to_string()));
            }
        })
    }
}

#[derive(Debug)]
struct CashTransfer {
    date: Date,
    amount: f64,
    transfer_type: TransferType,
    note: String,
}

#[derive(Debug)]
struct SparkassenKontoauszug {
    account_info: Option<(String, String)>,
    start_balance: f64,
    end_balance: f64,
    cash_transfers: Vec<CashTransfer>,
    currency: Currency,
}

impl ParsedDocument for SparkassenKontoauszug {
    fn add_account_info(&mut self, bank: Bank, account: &str) {
        self.account_info = Some((format!("{}", bank), account.to_string()));
    }

    fn get_account_info(&self) -> Option<(String, String)> {
        return self.account_info.clone();
    }
    fn check_consistency(&self) -> Result<()> {
        let mut balance = self.start_balance;
        for cf in &self.cash_transfers {
            balance += cf.amount;
        }
        let balance_diff = self.end_balance - balance;
        if balance_diff.abs() > 1e-4 {
            return Err(ReadPDFError::ConsistencyCheckFailed(format!(
                "Found cash balance difference of {balance_diff} EUR"
            )));
        }
        Ok(())
    }
    fn make_transactions(&self) -> Result<(Vec<Transaction>, Option<Asset>)> {
        let mut transactions = Vec::new();
        for cf in &self.cash_transfers {
            transactions.push(Transaction {
                id: None,
                transaction_type: TransactionType::Cash,
                cash_flow: CashFlow {
                    amount: CashAmount {
                        amount: cf.amount,
                        currency: self.currency.clone(),
                    },
                    date: cf.date,
                },
                note: Some(format!("{:?}: {}", cf.transfer_type, cf.note)),
            })
        }
        Ok((transactions, None))
    }
}

/// Extract transaction information from text files
pub async fn parse_document(text: &str, market: &Market) -> Result<ParsedInfo> {
    lazy_static! {
        static ref KONTOSTAND_BEGIN: Regex =
            Regex::new(r"Kontostand am ([0-9.]{10}), Auszug Nr. [0-9][0-9]? (-?[0-9,.]*)").unwrap();
        static ref CASHFLOW: Regex = Regex::new(
            r"([0-9.]{10}) (Dauerauftrag|Lastschrift|Debitkartenzahlung) (.*)\s+(-?[0-9.]+,[0-9]{2}*)"
        )
        .unwrap();
        static ref KONTOSTAND_END: Regex =
            Regex::new(r"Kontostand am ([0-9.]{10}) um [0-9]{2}:[0-9]{2} Uhr (-?[0-9,.]*)")
                .unwrap();
    }

    debug!("Reading Sparkasse Kontoauszug");
    let (start_date, start_balance) = match KONTOSTAND_BEGIN.captures(text) {
        Some(cap) => Ok((
            german_string_to_date(&cap[1])?,
            german_string_to_float(&cap[2])?,
        )),
        None => Err(ReadPDFError::NotFound("start time and balance")),
    }?;
    debug!("Start balance {start_balance} EUR at {start_date}");
    let (end_date, end_balance) = match KONTOSTAND_END.captures(text) {
        Some(cap) => Ok((
            german_string_to_date(&cap[1])?,
            german_string_to_float(&cap[2])?,
        )),
        None => Err(ReadPDFError::NotFound("end time and balance")),
    }?;
    debug!("End balance {end_balance} EUR at {end_date}");

    let mut cash_transfers = Vec::new();
    for caps in CASHFLOW.captures_iter(text) {
        let transfer = CashTransfer {
            date: german_string_to_date(&caps[1])?,
            amount: german_string_to_float(&caps[4])?,
            transfer_type: TransferType::from_str(&caps[2])?,
            note: caps[3].to_string(),
        };
        debug!("Parsed cash flow {transfer:?}");
        cash_transfers.push(transfer);
    }
    // All cash flows are in EUR
    let currency = market.get_currency_from_str("EUR").await?;
    Ok(Arc::new(SparkassenKontoauszug {
        start_balance,
        end_balance,
        cash_transfers,
        account_info: None,
        currency,
    }))
}

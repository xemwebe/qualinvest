//! Parse text files and extract asset and transaction data
use super::{Bank, ParsedDocument, ParsedInfo, ReadPDFError};
use crate::read_pdf::{
    AccountHandler,
    utils::{german_string_to_date, german_string_to_float},
};
use async_trait::async_trait;
use finql::{
    Market,
    datatypes::{Asset, CashAmount, CashFlow, Currency, Transaction, TransactionType},
};
use lazy_static::lazy_static;
use log::debug;
use regex::Regex;
use std::sync::Arc;
use time::Date;

type Result<T> = std::result::Result<T, ReadPDFError>;

#[derive(Debug)]
enum TransferType {
    DauerAuftrag,
    Lastschrift,
    Gutschrift,
    Entgelt,
}

impl TransferType {
    fn from_str(s: &str) -> Result<Self> {
        Ok(match s {
            "Dauerauftrag/Terminueberw." => TransferType::DauerAuftrag,
            "Lastschrift" => TransferType::Lastschrift,
            "Gutschrift" => TransferType::Gutschrift,
            "Entgelt" => TransferType::Entgelt,
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
    counter_party: String,
    note: String,
}

#[derive(Debug)]
struct INGDibaKontoauszug {
    account_info: Option<(String, String)>,
    start_balance: f64,
    end_balance: f64,
    cash_transfers: Vec<CashTransfer>,
    currency: Currency,
}

#[async_trait]
impl ParsedDocument for INGDibaKontoauszug {
    fn add_account_info(&mut self, bank: Bank, account: &str) {
        self.account_info = Some((format!("{}", bank), account.to_string()));
    }

    fn get_account_info(&self) -> Option<(String, String)> {
        self.account_info.clone()
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

    async fn make_transactions(
        &self,
        _db: Arc<dyn AccountHandler + Send + Sync>,
    ) -> Result<Vec<Transaction>> {
        let mut transactions = Vec::new();
        for cf in &self.cash_transfers {
            transactions.push(Transaction {
                id: None,
                transaction_type: TransactionType::Cash,
                cash_flow: CashFlow {
                    amount: CashAmount {
                        amount: cf.amount,
                        currency: self.currency,
                    },
                    date: cf.date,
                },
                note: Some(format!(
                    "{:?}->{}: {}",
                    cf.transfer_type, cf.counter_party, cf.note
                )),
            })
        }
        Ok(transactions)
    }
}

/// Extract transaction information from text files
pub async fn parse_document(text: &str, market: &Market) -> Result<ParsedInfo> {
    lazy_static! {
        static ref ALTER_SALDO: Regex = Regex::new(r"Alter Saldo ([-0-9.,]*) Euro").unwrap();
        static ref NEUER_SALDO: Regex = Regex::new(r"Neuer Saldo ([-0-9.,]*) Euro").unwrap();
        static ref CASHFLOW: Regex = Regex::new(r"(?m)(?<date>[0-9.]{10}) (?<type>Lastschrift|Gutschrift|Entgelt|Dauerauftrag/Terminueberw.) (?<counterparty>.*) (?<value>-?[0-9.]+,[0-9]{2})\s*([0-9.]{10}) (?<note>.*)$").unwrap();
    }

    debug!("Reading ING Diba Girokonto Kontoauszug");
    let start_balance = match ALTER_SALDO.captures(text) {
        Some(cap) => Ok(german_string_to_float(&cap[1])?),
        None => Err(ReadPDFError::NotFound("start balance")),
    }?;
    debug!("Start balance is {start_balance} EUR");

    let end_balance = match NEUER_SALDO.captures(text) {
        Some(cap) => Ok(german_string_to_float(&cap[1])?),
        None => Err(ReadPDFError::NotFound("start balance")),
    }?;
    debug!("End balance is {end_balance} EUR");

    let mut cash_transfers = Vec::new();
    for caps in CASHFLOW.captures_iter(text) {
        let transfer_type = TransferType::from_str(&caps["type"])?;
        let date = german_string_to_date(&caps["date"])?;
        let note = caps["note"].to_string();
        let counter_party = caps["counterparty"].to_string();
        let amount = german_string_to_float(&caps["value"])?;
        let transfer = CashTransfer {
            date,
            amount,
            transfer_type,
            counter_party,
            note,
        };
        cash_transfers.push(transfer);
    }
    // All cash flows are in EUR
    let currency = market.get_currency_from_str("EUR").await?;
    Ok(Arc::new(INGDibaKontoauszug {
        start_balance,
        end_balance,
        cash_transfers,
        account_info: None,
        currency,
    }))
}

use super::{Bank, ParsedDocument, ParsedInfo, ReadPDFError, Result};
use crate::read_pdf::utils::{german_string_to_date, german_string_to_float};
use time::Date;

use crate::read_pdf::AccountHandler;
use async_trait::async_trait;
use finql::{
    Market,
    datatypes::{Asset, CashAmount, CashFlow, Currency, Transaction, TransactionType},
};
use lazy_static::lazy_static;
use log::debug;
use regex::Regex;
use std::sync::Arc;

#[derive(Debug)]
struct CashTransfer {
    date: Date,
    amount: f64,
    note: String,
}

#[derive(Debug)]
struct Finanzreport {
    account_info: Option<(String, String)>,
    start_balance: f64,
    end_balance: f64,
    cash_transfers: Vec<CashTransfer>,
    currency: Currency,
}

#[async_trait]
impl ParsedDocument for Finanzreport {
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
                note: Some(cf.note.clone()),
            })
        }
        Ok(transactions)
    }
}

pub async fn parse_report(text: &str, market: &Market) -> Result<ParsedInfo> {
    lazy_static! {
        static ref GIRO_SECTION: Regex =
            Regex::new(r"Girokonto Dispositionskredit ([0-9.]*) EUR(\s*.*\s*)*?Alter Saldo ([0-9.]{10}) (?<start>[+-][0-9.,]*)(?<cfs>(\s*.*\s*)*?)Neuer Saldo ([0-9.]{10}) (?<end>[+-][0-9.,]*)").unwrap();
        static ref CASHFLOW: Regex = Regex::new(
            r"(?<booking_date>[0-9.]{10})\s*(?<valuta>[0-9.]{10})(?<note>(\s*.*\s*)*?)\s+(?<amount>[+-][0-9,.]*)"
        ).unwrap();
    }
    debug!("Reading comdirect Finanzreport");
    debug!("captures: {:?}", GIRO_SECTION.captures(text));
    let (start_balance, end_balance, cash_flows) = match GIRO_SECTION.captures(text) {
        Some(cap) => Ok((
            german_string_to_float(&cap["start"])?,
            german_string_to_float(&cap["end"])?,
            cap["cfs"].to_string(),
        )),
        None => Err(ReadPDFError::NotFound(
            "Giro account section in comdirect Finanzreport not found",
        )),
    }?;
    debug!("Reading cashflows with start balance {start_balance} and end balance {end_balance}");
    let mut cash_transfers = Vec::new();
    for caps in CASHFLOW.captures_iter(&cash_flows) {
        let note = caps["note"].to_string();
        let note = note.replace('\n', " ");
        let transfer = CashTransfer {
            date: german_string_to_date(&caps["valuta"])?,
            amount: german_string_to_float(&caps["amount"])?,
            note,
        };
        cash_transfers.push(transfer);
    }
    // All cash flows are in EUR
    let currency = market.get_currency_from_str("EUR").await?;
    Ok(Arc::new(Finanzreport {
        start_balance,
        end_balance,
        cash_transfers,
        account_info: None,
        currency,
    }))
}

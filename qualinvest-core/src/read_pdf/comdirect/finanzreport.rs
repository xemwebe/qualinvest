use super::{Bank, ParsedDocument, ParsedInfo, ReadPDFError, Result};
use crate::read_pdf::utils::{german_string_to_date, german_string_to_float};
use time::Date;

use finql::{
    datatypes::{Asset, CashAmount, CashFlow, Currency, Transaction, TransactionType},
    Market,
};
use lazy_static::lazy_static;
use log::debug;
use regex::Regex;
use std::sync::Arc;

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

impl ParsedDocument for Finanzreport {
    fn add_account_info(&mut self, bank: Bank, account: &str) {
        self.account_info = Some((format!("{}", bank), account.to_string()));
    }

    fn get_account_info(&self) -> Option<(String, String)> {
        return self.account_info.clone();
    }

    fn check_consistency(&self) -> Result<()> {
        Ok(())
    }

    fn make_transactions(&self) -> Result<(Vec<Transaction>, Option<Asset>)> {
        Err(ReadPDFError::NotFound("comdirect Finanzreport parser"))
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

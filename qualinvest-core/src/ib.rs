use ib::read::KontoauszugData;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::{io, num, string};

use thiserror::Error;

use log::{debug, info};
use sanitize_filename::sanitize;

use crate::read_pdf::{ParsedDocument, ReadPDFError, read_account_info::Bank, store_parsed_pdf};
use async_trait::async_trait;
use finql::{
    Market,
    datatypes::{
        Asset, CashAmount, CashFlow, CurrencyError, DataError, Transaction, TransactionType,
    },
};

type Result<T> = std::result::Result<T, ReadPDFError>;

use super::accounts::{Account, AccountHandler};
use crate::PdfParseParams;

#[async_trait]
impl ParsedDocument for KontoauszugData {
    fn add_account_info(&mut self, _bank: Bank, _account: &str) {
        // do nothing
    }

    fn get_account_info(&self) -> Option<(String, String)> {
        self.get_account_info()
    }

    fn check_consistency(&self) -> Result<()> {
        // let mut balance = self.start_balance;
        // for cf in &self.cash_transfers {
        //     balance += cf.amount;
        // }
        // let balance_diff = self.end_balance - balance;
        // if balance_diff.abs() > 1e-4 {
        //     return Err(ReadPDFError::ConsistencyCheckFailed(format!(
        //         "Found cash balance difference of {balance_diff} EUR"
        //     )));
        // }
        Ok(())
    }

    async fn make_transactions(
        &self,
        _db: Arc<dyn AccountHandler + Send + Sync>,
    ) -> Result<Vec<Transaction>> {
        let mut transactions = Vec::new();
        for transaction in &self.transaktionen {
            let position = transaction.menge;
            let symbol = transaction.symbol.clone();
            let (name, isin) = self
                .finanzinstrumente
                .get_isin_and_name_by_symbol(&symbol)?;
            // transactions.push(Transaction {
            //     id: None,
            //     transaction_type: TransactionType::Asset { asset_id, position },
            //     cash_flow: CashFlow {
            //         amount: CashAmount {
            //             amount: cf.amount,
            //             currency: self.currency,
            //         },
            //         date: cf.date,
            //     },
            //     note: Some(format!("{:?}: {}", cf.transfer_type, cf.note)),
            // })
        }
        Ok(transactions)
    }
}

pub async fn store_parsed_csv(
    hash: &str,
    file_name: &str,
    data: KontoauszugData,
    db: Arc<dyn AccountHandler + Send + Sync>,
    config: &PdfParseParams,
) -> Result<i32> {
    let parsed_info = Arc::new(data);
    store_parsed_pdf(hash, file_name, parsed_info, db, config).await
}

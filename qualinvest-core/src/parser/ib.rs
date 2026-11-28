use ib::read::KontoauszugData;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::{io, num, string};

use thiserror::Error;

use log::{debug, info};
use sanitize_filename::sanitize;

use crate::parser::{ParseError, ParsedDocument, read_account_info::Bank, store_parsed_pdf};
use async_trait::async_trait;
use finql::{
    Market,
    datatypes::{
        Asset, CashAmount, CashFlow, CurrencyError, DataError, Stock, Transaction, TransactionType,
    },
};

type Result<T> = std::result::Result<T, ParseError>;

use crate::PdfParseParams;
use crate::parser::{Account, AccountHandler};

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
        db: Arc<dyn AccountHandler + Send + Sync>,
        market: &Market,
    ) -> Result<Vec<Transaction>> {
        let mut transactions = Vec::new();
        for transaction in &self.transaktionen {
            let position = transaction.menge;
            let symbol = transaction.symbol.clone();

            // Determine asse_id
            let (asset_name, isin) = self
                .finanzinstrumente
                .get_isin_and_name_by_symbol(&symbol)?;
            let asset = Asset::Stock(Stock {
                id: None,
                name: asset_name.clone(),
                isin: Some(isin),
                wkn: None,
                note: None,
            });
            let asset_id = db
                .get_asset_id(&asset)
                .await
                .ok_or_else(|| ParseError::AssetNotFound(asset_name))?;
            debug!("Asset id is {asset_id:?}");

            // Get currency
            let currency = market.get_currency_from_str(&transaction.waehrung).await?;

            let amount = transaction.erloese;

            // Date conversion
            let timestamp = ::ib::date::convert_date(&transaction.datum_zeit)?;
            let date = time::UtcDateTime::from_unix_timestamp(timestamp)?.date();

            transactions.push(Transaction {
                id: None,
                transaction_type: TransactionType::Asset { asset_id, position },
                cash_flow: CashFlow {
                    amount: CashAmount { amount, currency },
                    date,
                },
                note: Some(format!("IB symvol {symbol}")),
            });

            let fee = transaction.prov_gebuehr;
            if fee != 0.0 {
                transactions.push(Transaction {
                    id: None,
                    transaction_type: TransactionType::Fee {
                        transaction_ref: Some(-1),
                    },
                    cash_flow: CashFlow {
                        amount: CashAmount {
                            amount: fee,
                            currency,
                        },
                        date,
                    },
                    note: None,
                })
            }
        }
        Ok(transactions)
    }
}

pub async fn store_parsed_csv(
    hash: &str,
    file_name: &str,
    data: KontoauszugData,
    db: Arc<dyn AccountHandler + Send + Sync>,
    market: &Market,
    config: &PdfParseParams,
) -> Result<i32> {
    let parsed_info = Arc::new(data);
    store_parsed_pdf(hash, file_name, parsed_info, db, market, config).await
}

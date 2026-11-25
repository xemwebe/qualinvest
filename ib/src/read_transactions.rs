use std::{cmp::Ordering, collections::HashMap, path::Path};

use crate::date::convert_date;
use crate::error::Error;
use crate::fx::FxRates;

type Result<T> = std::result::Result<T, Error>;

// ============================================================
// Field helpers (same conventions as read.rs)
// ============================================================

/// Parse an optional float from a CSV cell.
/// Returns `None` for empty strings, `"-"` (not applicable), or `"--"`.
/// Handles thousands-separator commas and trailing `%`.
fn opt_f64(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() || s == "-" || s == "--" {
        return None;
    }
    let cleaned: String = s.chars().filter(|&c| c != ',').collect();
    let cleaned = cleaned.trim_end_matches('%');
    cleaned.parse().ok()
}

fn fv(s: &str) -> f64 {
    opt_f64(s).unwrap_or(0.0)
}

/// Return `Some(s)` unless the cell is `"-"` or empty (not applicable).
fn opt_str(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() || s == "-" {
        None
    } else {
        Some(s.to_string())
    }
}

/// Get field `i` from a row slice, returning `""` if out of bounds.
fn c(row: &[String], i: usize) -> &str {
    row.get(i).map(String::as_str).unwrap_or("")
}

// ============================================================
// Table structs
// ============================================================

/// `Statement` – report metadata (key → value pairs).
#[derive(Debug)]
#[allow(dead_code)]
pub struct StatementRow {
    pub feldname: String,
    pub feldwert: String,
}

/// `Summary` – account cash summary (key → value pairs).
#[derive(Debug)]
#[allow(dead_code)]
pub struct SummaryRow {
    pub feldname: String,
    pub feldwert: String,
}

/// `Transaction History` – one row per transaction event.
///
/// Columns: Date, Account, Description, Transaction Type, Symbol,
///          Quantity, Price, Price Currency, Gross Amount, Commission, Net Amount.
///
/// Fields that are not applicable for a given transaction type are
/// represented as `"-"` in the CSV and mapped to `None` here.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TransactionHistoryRow {
    pub date: String,
    pub account: String,
    pub description: String,
    pub transaction_type: String,
    /// `None` when not applicable (e.g. interest or fee rows show `"-"`).
    pub symbol: Option<String>,
    /// `None` when not applicable.
    pub quantity: Option<f64>,
    /// `None` when not applicable.
    pub price: Option<f64>,
    /// `None` when not applicable.
    pub price_currency: Option<String>,
    /// `None` when not applicable.
    pub gross_amount: Option<f64>,
    /// `None` when not applicable.
    pub commission: Option<f64>,
    pub net_amount: f64,
}

// ============================================================
// Top-level data container
// ============================================================

pub struct TransactionHistoryData {
    pub transactions: Vec<TransactionHistoryRow>,
}

// ============================================================
// Raw CSV loading  (same pattern as read.rs)
// ============================================================

/// Raw rows grouped by table name.
/// Each entry: `(row_kind, fields_after_table_name_and_row_kind)`.
type Groups = HashMap<String, Vec<(String, Vec<String>)>>;

fn load_groups(path: &Path) -> Result<Groups> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_path(path)?;

    let mut groups: Groups = HashMap::new();

    for result in rdr.records() {
        let rec = result?;
        if rec.len() < 2 {
            continue;
        }
        let table = rec[0].trim().to_string();
        if table.is_empty() {
            continue;
        }
        let kind = rec[1].trim().to_string();
        let fields: Vec<String> = rec.iter().skip(2).map(str::to_string).collect();
        groups.entry(table).or_default().push((kind, fields));
    }
    Ok(groups)
}

/// Iterate over all `Data`-kind rows for a given table.
fn data_rows<'a>(groups: &'a Groups, table: &str) -> impl Iterator<Item = &'a Vec<String>> {
    groups
        .get(table)
        .into_iter()
        .flatten()
        .filter(|(kind, _)| kind == "Data")
        .map(|(_, fields)| fields)
}

// ============================================================
// Parsing
// ============================================================

pub fn parse_transaction_history(path: &Path) -> Result<TransactionHistoryData> {
    let groups = load_groups(path)?;

    // ── Transaction History ───────────────────────────────────────────────────
    // Columns (0-indexed after table name and row kind are stripped):
    //  0  Date            – YYYY-MM-DD
    //  1  Account         – masked account id
    //  2  Description     – human-readable event description
    //  3  Transaction Type – e.g. Dividend, Buy, Sell, Credit Interest, …
    //  4  Symbol          – ticker or "-" when not applicable
    //  5  Quantity        – number or "-"
    //  6  Price           – number or "-"
    //  7  Price Currency  – 3-letter code or "-"
    //  8  Gross Amount    – number or "-"
    //  9  Commission      – number or "-"
    // 10  Net Amount      – always a number
    let transactions: Vec<TransactionHistoryRow> = data_rows(&groups, "Transaction History")
        .map(|f| TransactionHistoryRow {
            date: c(f, 0).to_string(),
            account: c(f, 1).to_string(),
            description: c(f, 2).to_string(),
            transaction_type: c(f, 3).to_string(),
            symbol: opt_str(c(f, 4)),
            quantity: opt_f64(c(f, 5)),
            price: opt_f64(c(f, 6)),
            price_currency: opt_str(c(f, 7)),
            gross_amount: opt_f64(c(f, 8)),
            commission: opt_f64(c(f, 9)),
            net_amount: fv(c(f, 10)),
        })
        .collect();

    Ok(TransactionHistoryData { transactions })
}

#[derive(Debug, PartialEq, Eq)]
pub enum BuySell {
    Sell,
    Buy,
}

#[derive(Debug, PartialEq)]
pub struct FifoTransaction {
    pub timestamp: i64,
    pub symbol: String,
    pub quantity: f64,
    pub price: f64,
    pub buy_sell: BuySell,
}

impl Eq for FifoTransaction {}

impl std::cmp::PartialOrd for FifoTransaction {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl FifoTransaction {
    pub fn get_timestamp(&self) -> i64 {
        self.timestamp
    }
}

impl std::cmp::Ord for FifoTransaction {
    fn cmp(&self, other: &FifoTransaction) -> Ordering {
        if self.timestamp != other.timestamp {
            return self.timestamp.cmp(&other.timestamp);
        }
        if self.buy_sell == BuySell::Buy && other.buy_sell == BuySell::Sell {
            return Ordering::Less;
        } else if self.buy_sell == BuySell::Sell && other.buy_sell == BuySell::Buy {
            return Ordering::Greater;
        }
        if self.quantity != other.quantity {
            if self.quantity > other.quantity {
                return Ordering::Less;
            } else {
                return Ordering::Greater;
            }
        }
        if self.price != other.price {
            if self.price > other.price {
                return Ordering::Less;
            } else {
                return Ordering::Greater;
            }
        }
        self.symbol.cmp(&other.symbol)
    }
}

impl TransactionHistoryData {
    pub fn extract_purchase_infos(&self, fx_rates: &FxRates) -> Result<Vec<FifoTransaction>> {
        let mut fifo_trans = Vec::new();
        for transaction in self.transactions.iter() {
            match transaction.transaction_type.as_str() {
                "Sell" => {
                    if let Some(symbol) = &transaction.symbol
                        && let Some(quantity) = transaction.quantity
                        && let Some(price) = transaction.price
                        && let Some(curr) = &transaction.price_currency
                    {
                        let timestamp = convert_date(&transaction.date)?;
                        fifo_trans.push(FifoTransaction {
                            timestamp,
                            symbol: symbol.clone(),
                            quantity: -quantity,
                            price: price * fx_rates.get_fx_rate(timestamp, curr)?,
                            buy_sell: BuySell::Sell,
                        });
                    } else {
                        return Err(Error::InvalidSellTransaction);
                    }
                }
                "Buy" => {
                    if let Some(symbol) = &transaction.symbol
                        && let Some(quantity) = transaction.quantity
                        && let Some(price) = transaction.price
                        && let Some(curr) = &transaction.price_currency
                    {
                        let timestamp = convert_date(&transaction.date)?;
                        fifo_trans.push(FifoTransaction {
                            timestamp,
                            symbol: symbol.clone(),
                            quantity,
                            price: price * fx_rates.get_fx_rate(timestamp, curr)?,
                            buy_sell: BuySell::Buy,
                        });
                    } else {
                        return Err(Error::InvalidBuyTransaction);
                    }
                }
                _ => {}
            }
        }

        Ok(fifo_trans)
    }
}

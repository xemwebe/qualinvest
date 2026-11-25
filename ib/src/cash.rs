use crate::date::convert_date;
use crate::error::{Error, Result};
use std::path::Path;

/// `Statement` – broker metadata (key → value pairs).
#[derive(Debug)]
#[allow(dead_code)]
pub struct CashFlow {
    pub account: String,
    pub curr: String,
    pub date: i64,
    pub amount: f64,
    pub total: f64,
}

pub fn read_cash_flows(path: &Path) -> Result<Vec<CashFlow>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)?;

    let mut cash_flows = Vec::new();
    for result in rdr.records() {
        let result = result?;
        let fields: Vec<&str> = result.iter().collect();
        if fields.len() != 5 {
            return Err(Error::CashBerichtUngültig);
        }
        let date = convert_date(fields[2])?;
        let amount = fields[3].parse()?;
        let total = fields[4].parse()?;
        cash_flows.push(CashFlow {
            account: fields[0].to_string(),
            curr: fields[1].to_string(),
            date,
            amount,
            total,
        })
    }
    Ok(cash_flows)
}

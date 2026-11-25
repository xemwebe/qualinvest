use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};

use crate::date::convert_date;
use crate::error::Error;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Default)]
pub struct FxRates {
    tables: HashMap<String, FxTable>,
}

/// Wechselkurse werden als Tage seit UNIX EPOCH und Wert einer Einheit der Fremdwährung in EUR
#[derive(Debug, Default)]
pub struct FxTable {
    table: BTreeMap<i64, f64>,
}

const SECONDS_PER_DATE: i64 = 24 * 60 * 60;

impl FxRates {
    pub fn get_fx_rate(&self, timestamp: i64, währung: &str) -> Result<f64> {
        if währung == "EUR" {
            Ok(1.0)
        } else {
            if let Some(fx) = self
                .tables
                .get(währung)
                .ok_or(Error::CurrencyNotFound(währung.to_string()))?
                .get_fx_rate(timestamp)
            {
                Ok(1.0 / fx)
            } else {
                Err(Error::CurrencyNotFound(währung.to_string()))
            }
        }
    }
}

impl FxTable {
    pub fn get_fx_rate(&self, seconds: i64) -> Option<f64> {
        // Kovertierung von Sekunden in Tage
        let days = seconds / SECONDS_PER_DATE;
        if let Some(fx) = self.table.get(&days) {
            return Some(*fx);
        }
        self.table.range(..days).next_back().map(|(_, f)| *f)
    }
}

pub fn read_fx_rates(fx_path: &Path) -> Result<FxRates> {
    println!("Reading fx rates from '{}'", fx_path.display());
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(fx_path)?;
    let mut fx_rates = FxRates::default();
    let mut header_ref = HashMap::<String, usize>::new();
    let headers = rdr.headers()?;
    for (idx, header) in headers.iter().enumerate() {
        let column_name = header.to_owned();
        if column_name.is_empty() || column_name == "Date" {
            continue;
        }
        header_ref.insert(column_name.clone(), idx);
        fx_rates.tables.insert(column_name, FxTable::default());
    }
    for record in rdr.records() {
        let record = record?;
        let date = record.get(0).to_owned().ok_or(Error::RecordNotFound)?;
        let seconds = convert_date(date)?;
        for header in header_ref.keys() {
            if let Some(fx_rate) = record.get(header_ref[header])
                && fx_rate != "N/A"
                && let Some(fx_table) = fx_rates.tables.get_mut(header)
            {
                fx_table.table.insert(
                    seconds.to_owned() / SECONDS_PER_DATE,
                    fx_rate.parse().map_err(|_| Error::ParsingNumberFailed)?,
                );
            }
        }
    }
    Ok(fx_rates)
}

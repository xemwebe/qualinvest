use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

use crate::asset_events::Kapitalmaßnahme;
use crate::error::Error;
use crate::{fx, read};

type Result<T> = std::result::Result<T, Error>;

/// Store comprehensive transaction history information for FIFO based P&L caculation
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct FifoStore {
    timestamp: i64,
    history: HashMap<String, FifoInfo>,
}

/// FIFO history for single asset
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
struct FifoInfo {
    fifo: VecDeque<PurchaseInfo>,
}

/// Price and position purchased
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PurchaseInfo {
    position: f64,
    price: f64,
}

impl FifoStore {
    pub fn new(timestamp: i64) -> Self {
        Self {
            timestamp,
            history: HashMap::new(),
        }
    }

    pub fn from_open_positions(
        positions: &[read::OffenePositionRow],
        timestamp: i64,
        fx_rates: &fx::FxRates,
    ) -> Result<Self> {
        let mut store = Self::new(timestamp);
        for position in positions {
            if position.data_discriminator == "Summary" {
                let purchase_info = PurchaseInfo {
                    position: position.menge.ok_or(Error::LeereMenge)?,
                    price: fx_rates.get_fx_rate(timestamp, &position.waehrung)?
                        * position.einstands_kurs,
                };
                store.add(&position.symbol, timestamp, purchase_info)?;
            }
        }

        Ok(store)
    }

    pub fn add(&mut self, symbol: &str, timestamp: i64, purchase: PurchaseInfo) -> Result<()> {
        if timestamp < self.timestamp {
            return Err(Error::FifoIstNeuer);
        }
        self.timestamp = timestamp;
        if let Some(fifo_info) = self.history.get_mut(symbol) {
            fifo_info.add(purchase);
        } else {
            self.history
                .insert(symbol.to_owned(), FifoInfo::new(purchase));
        }
        Ok(())
    }

    /// remove the purchase of the first position purchase and return the purchase price
    pub fn reduce(&mut self, symbol: &str, timestamp: i64, position: f64) -> Result<f64> {
        if timestamp < self.timestamp {
            return Err(Error::FifoIstNeuer);
        }
        if let Some(fifo_info) = self.history.get_mut(symbol) {
            let purchase_cost = fifo_info.reduce(position)?;
            self.timestamp = timestamp;
            Ok(purchase_cost)
        } else {
            let date = crate::date::convert_timestamp_to_date_string(timestamp)?;
            eprintln!("Verkauf von {position} {symbol} am {date} schlägt fehl: Leerverkauf!");
            Err(Error::KeineLeerverkäufe)
        }
    }

    pub fn get_timestamp(&self) -> i64 {
        self.timestamp
    }

    pub fn contains(&self, symbol: &str) -> bool {
        if let Some(v) = self.history.get(symbol) {
            !v.fifo.is_empty()
        } else {
            false
        }
    }

    pub fn exchange_assets(&mut self, k: &Kapitalmaßnahme, timestamp: i64) -> Result<()> {
        let kosten = self.reduce(&k.altes_symbol, timestamp, k.alte_menge)?;
        let neuer_preis = kosten / k.neue_menge;
        self.add(
            &k.neues_symbol,
            timestamp,
            PurchaseInfo::new(k.neue_menge, neuer_preis),
        )?;
        Ok(())
    }
}

impl FifoInfo {
    fn new(purchase: PurchaseInfo) -> Self {
        let mut fifo = VecDeque::new();
        fifo.push_back(purchase);
        Self { fifo }
    }

    fn add(&mut self, purchase: PurchaseInfo) {
        self.fifo.push_back(purchase);
    }

    fn reduce(&mut self, position: f64) -> Result<f64> {
        let mut purchase_amount = 0.0;
        let mut position = position;
        while position > 0.0 {
            let mut delete_first = false;
            if let Some(first) = self.fifo.front_mut() {
                if position < first.position {
                    purchase_amount += position * first.price;
                    first.position -= position;
                    position = 0.0;
                } else {
                    purchase_amount += first.position * first.price;
                    position -= first.position;
                    first.position = 0.0;
                    delete_first = true;
                }
            } else {
                if position < 1.0 {
                    break;
                }
                eprintln!(
                    "Verkauf von {position} assets schlägt fehl: keine offene Position vorhanden"
                );
                return Err(Error::KeineLeerverkäufe);
            }
            if delete_first {
                let _ = self.fifo.pop_front();
            }
        }
        Ok(purchase_amount)
    }
}

impl PurchaseInfo {
    pub fn new(position: f64, price: f64) -> Self {
        Self { position, price }
    }
}

use finql::Currency;
use finql::transaction::{Transaction, TransactionType};
use std::collections::BTreeMap;

/// Errors related to position calculation
pub enum PositionError{
    ForeignCurrency,
}

/// Calculate the total position as of a given date by applying a specified set of filters
pub struct Position {
    pub asset_id: Option<usize>,
    pub position: f64,
    pub purchase_value: f64,
    pub interest: f64,
    pub dividend: f64,
    pub fees: f64,
    pub tax: f64,
    pub currency: Currency,
}

impl Position {
    fn new(asset_id: Option<usize>, currency: Currency) -> Position {
        Position{
            asset_id,
            position: 0.0,
            purchase_value: 0.0,
            currency,
            interest: 0.0,
            dividend: 0.0,
            fees: 0.0,
            tax: 0.0,
        }
    }
}

pub struct PortfolioPosition {
    cash: Position,
    assets: BTreeMap<usize, Position>,
}


impl PortfolioPosition {
    pub fn new(base_currency: Currency) -> PortfolioPosition {
        PortfolioPosition { 
            cash: Position {
                asset_id: None,
                position: 0.0,
                purchase_value: 0.0,
                currency: base_currency,
                interest: 0.0,
                dividend: 0.0,
                fees: 0.0,
                tax: 0.0,
            },
            assets: BTreeMap::new(),
        }
    }
}

/// Search for transaction referred to by transaction_ref and return associated asset_id
fn get_asset_id(transactions: &Vec<Transaction>, trans_ref: Option<usize>) -> Option<usize> {
    if trans_ref.is_none() { return None; }
    for trans in transactions {
        if trans.id == trans_ref {
            return match trans.transaction_type {
                TransactionType::Asset{asset_id, position: _} => Some(asset_id),
                TransactionType::Dividend{asset_id} => Some(asset_id),
                TransactionType::Interest{asset_id} => Some(asset_id),
                _ => None,
            }
        }
    }
    None
}

/// Calculation of position since inception
pub fn calc_position(base_currency: Currency, transactions: &Vec<Transaction>) -> Result<PortfolioPosition, PositionError> {
    let mut positions = PortfolioPosition::new(base_currency);
    calc_delta_position(&mut positions, transactions)?;
    Ok(positions)
}

pub fn calc_delta_position(positions: &mut PortfolioPosition, transactions: &Vec<Transaction>) -> Result<(),PositionError> {
    let base_currency = positions.cash.currency;
    for trans in transactions {
        // ignore zero cash flows
        if trans.cash_flow.amount.amount == 0.0 { continue; }
        // currently, we assume that all cash flows are in one account have the same currency
        if trans.cash_flow.amount.currency != base_currency {
            return Err(PositionError::ForeignCurrency);
        }
        match trans.transaction_type {
            TransactionType::Cash => {
                positions.cash.position += trans.cash_flow.amount.amount;
            },
            TransactionType::Asset{asset_id, position} => {
                match positions.assets.get_mut(&asset_id) {
                    None => {
                        let mut new_pos = Position::new(Some(asset_id), base_currency);
                        new_pos.position = position;
                        new_pos.purchase_value = trans.cash_flow.amount.amount;
                        positions.assets.insert(asset_id, new_pos);
                    },
                    Some(pos) => {
                        pos.position += position;
                        pos.purchase_value += trans.cash_flow.amount.amount;
                    },
                };
            },
            TransactionType::Interest{asset_id} => {
                match positions.assets.get_mut(&asset_id) {
                    None => {
                        let mut new_pos = Position::new(Some(asset_id), base_currency);
                        new_pos.interest = trans.cash_flow.amount.amount;
                        positions.assets.insert(asset_id, new_pos);
                    },
                    Some(pos) => {
                        pos.interest += trans.cash_flow.amount.amount;
                    },
                };
            },
            TransactionType::Dividend{asset_id} => {
                match positions.assets.get_mut(&asset_id) {
                    None => {
                        let mut new_pos = Position::new(Some(asset_id), base_currency);
                        new_pos.dividend = trans.cash_flow.amount.amount;
                        positions.assets.insert(asset_id, new_pos);
                    },
                    Some(pos) => {
                        pos.dividend += trans.cash_flow.amount.amount;                      
                    },
                };
            }
            TransactionType::Fee{transaction_ref} => {
                let asset_id = get_asset_id(transactions, transaction_ref);
                if asset_id.is_some() {
                    let asset_id = asset_id.unwrap();
                    match positions.assets.get_mut(&asset_id) {
                        None => {
                            let mut new_pos = Position::new(Some(asset_id), base_currency);
                            new_pos.fees = trans.cash_flow.amount.amount;
                            positions.assets.insert(asset_id, new_pos);
                        },
                        Some(pos) => {
                            pos.fees += trans.cash_flow.amount.amount;
                        },
                    };   
                } else {
                    positions.cash.fees += trans.cash_flow.amount.amount;
                }
            }
            TransactionType::Tax{transaction_ref} => {
                let asset_id = get_asset_id(transactions, transaction_ref);
                if asset_id.is_some() {
                    let asset_id = asset_id.unwrap();
                    match positions.assets.get_mut(&asset_id) {
                        None => {
                            let mut new_pos = Position::new(Some(asset_id), base_currency);
                            new_pos.tax = trans.cash_flow.amount.amount;
                            positions.assets.insert(asset_id, new_pos);
                        },
                        Some(pos) => {
                            pos.tax += trans.cash_flow.amount.amount;
                        },
                    };   
                } else {
                    positions.cash.tax += trans.cash_flow.amount.amount;
                }            
            }
        }
    }
    Ok(())
}

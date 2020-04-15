use finql::transaction::{Transaction, TransactionType};
use finql::Currency;
use finql::data_handler::{DataError,AssetHandler};

use std::collections::BTreeMap;
use serde::{Deserialize,Serialize};

/// Errors related to position calculation
#[derive(Debug, Clone, PartialEq)]
pub enum PositionError {
    ForeignCurrency,
}

/// Calculate the total position as of a given date by applying a specified set of filters
#[derive(Debug,Serialize,Deserialize)]
pub struct Position {
    pub asset_id: Option<usize>,
    pub name: String,
    pub position: f64,
    pub purchase_value: f64,
    // realized p&l from buying/selling assets
    pub realized_pnl: f64,
    pub interest: f64,
    pub dividend: f64,
    pub fees: f64,
    pub tax: f64,
    pub currency: Currency,
}

impl Position {
    fn new(asset_id: Option<usize>, currency: Currency) -> Position {
        Position {
            asset_id,
            name: String::new(),
            position: 0.0,
            purchase_value: 0.0,
            realized_pnl: 0.0,
            currency,
            interest: 0.0,
            dividend: 0.0,
            fees: 0.0,
            tax: 0.0,
        }
    }
}

#[derive(Debug,Serialize,Deserialize)]
pub struct PortfolioPosition {
    pub cash: Position,
    pub assets: BTreeMap<usize, Position>,
}

impl PortfolioPosition {
    pub fn new(base_currency: Currency) -> PortfolioPosition {
        PortfolioPosition {
            cash: Position::new(None, base_currency),
            assets: BTreeMap::new(),
        }
    }

    pub fn get_asset_names(&mut self, db: &mut dyn AssetHandler) -> Result<(),DataError> {
        for (id, mut pos) in &mut self.assets {
            let asset = db.get_asset_by_id(*id)?;
            pos.name = asset.name;
        }
        Ok(())
    }
}

/// Search for transaction referred to by transaction_ref and return associated asset_id
fn get_asset_id(transactions: &Vec<Transaction>, trans_ref: Option<usize>) -> Option<usize> {
    if trans_ref.is_none() {
        return None;
    }
    for trans in transactions {
        if trans.id == trans_ref {
            return match trans.transaction_type {
                TransactionType::Asset {
                    asset_id,
                    position: _,
                } => Some(asset_id),
                TransactionType::Dividend { asset_id } => Some(asset_id),
                TransactionType::Interest { asset_id } => Some(asset_id),
                _ => None,
            };
        }
    }
    None
}

/// Calculation of position since inception
pub fn calc_position(
    base_currency: Currency,
    transactions: &Vec<Transaction>,
) -> Result<PortfolioPosition, PositionError> {
    let mut positions = PortfolioPosition::new(base_currency);
    calc_delta_position(&mut positions, transactions)?;
    Ok(positions)
}

pub fn calc_delta_position(
    positions: &mut PortfolioPosition,
    transactions: &Vec<Transaction>,
) -> Result<(), PositionError> {
    let base_currency = positions.cash.currency;
    for trans in transactions {
        // ignore zero cash flows
        if trans.cash_flow.amount.amount == 0.0 {
            continue;
        }
        // currently, we assume that all cash flows are in one account have the same currency
        if trans.cash_flow.amount.currency != base_currency {
            return Err(PositionError::ForeignCurrency);
        }
        // adjust cash balance
        positions.cash.position += trans.cash_flow.amount.amount;

        match trans.transaction_type {
            TransactionType::Cash => {
                // Do nothing, cash position has already been updated
            }
            TransactionType::Asset { asset_id, position } => {
                match positions.assets.get_mut(&asset_id) {
                    None => {
                        let mut new_pos = Position::new(Some(asset_id), base_currency);
                        new_pos.position = position;
                        new_pos.purchase_value = trans.cash_flow.amount.amount;
                        positions.assets.insert(asset_id, new_pos);
                    }
                    Some(pos) => {
                        let amount = trans.cash_flow.amount.amount;
                        if pos.position * position >= 0.0 {
                            // Increase position
                            pos.position += position;
                            pos.purchase_value += amount;
                        } else {
                            // Reduce position, calculate realized p&l part
                            let eff_price = -pos.purchase_value / pos.position;
                            let sell_price = -amount / position;
                            let pnl = -position * (sell_price - eff_price);
                            pos.realized_pnl += pnl;
                            pos.position += position;
                            pos.purchase_value += amount - pnl;
                        }
                    }
                };
            }
            TransactionType::Interest { asset_id } => {
                match positions.assets.get_mut(&asset_id) {
                    None => {
                        let mut new_pos = Position::new(Some(asset_id), base_currency);
                        new_pos.interest = trans.cash_flow.amount.amount;
                        positions.assets.insert(asset_id, new_pos);
                    }
                    Some(pos) => {
                        pos.interest += trans.cash_flow.amount.amount;
                    }
                };
            }
            TransactionType::Dividend { asset_id } => {
                match positions.assets.get_mut(&asset_id) {
                    None => {
                        let mut new_pos = Position::new(Some(asset_id), base_currency);
                        new_pos.dividend = trans.cash_flow.amount.amount;
                        positions.assets.insert(asset_id, new_pos);
                    }
                    Some(pos) => {
                        pos.dividend += trans.cash_flow.amount.amount;
                    }
                };
            }
            TransactionType::Fee { transaction_ref } => {
                let asset_id = get_asset_id(transactions, transaction_ref);
                if asset_id.is_some() {
                    let asset_id = asset_id.unwrap();
                    match positions.assets.get_mut(&asset_id) {
                        None => {
                            let mut new_pos = Position::new(Some(asset_id), base_currency);
                            new_pos.fees = trans.cash_flow.amount.amount;
                            positions.assets.insert(asset_id, new_pos);
                        }
                        Some(pos) => {
                            pos.fees += trans.cash_flow.amount.amount;
                        }
                    };
                } else {
                    positions.cash.fees += trans.cash_flow.amount.amount;
                }
            }
            TransactionType::Tax { transaction_ref } => {
                let asset_id = get_asset_id(transactions, transaction_ref);
                if asset_id.is_some() {
                    let asset_id = asset_id.unwrap();
                    match positions.assets.get_mut(&asset_id) {
                        None => {
                            let mut new_pos = Position::new(Some(asset_id), base_currency);
                            new_pos.tax = trans.cash_flow.amount.amount;
                            positions.assets.insert(asset_id, new_pos);
                        }
                        Some(pos) => {
                            pos.tax += trans.cash_flow.amount.amount;
                        }
                    };
                } else {
                    positions.cash.tax += trans.cash_flow.amount.amount;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use finql::assert_fuzzy_eq;
    use finql::{CashAmount, CashFlow};
    use std::str::FromStr;

    #[test]
    fn test_portfolio_position() {
        let tol = 1e-4;
        let eur = Currency::from_str("EUR").unwrap();

        let mut transactions = Vec::new();
        let positions = calc_position(eur, &transactions).unwrap();
        assert_fuzzy_eq!(positions.cash.position, 0.0, tol);

        transactions.push(Transaction {
            id: Some(1),
            transaction_type: TransactionType::Cash,
            cash_flow: CashFlow {
                amount: CashAmount {
                    amount: 10000.0,
                    currency: eur,
                },
                date: NaiveDate::from_ymd(2020, 1, 1),
            },
            note: None,
        });
        let positions = calc_position(eur, &transactions).unwrap();
        assert_fuzzy_eq!(positions.cash.position, 10000.0, tol);
        assert_eq!(positions.assets.len(), 0);

        transactions.push(Transaction {
            id: Some(2),
            transaction_type: TransactionType::Asset {
                asset_id: 1,
                position: 100.0,
            },
            cash_flow: CashFlow {
                amount: CashAmount {
                    amount: -104.0,
                    currency: eur,
                },
                date: NaiveDate::from_ymd(2020, 1, 2),
            },
            note: None,
        });
        transactions.push(Transaction {
            id: Some(3),
            transaction_type: TransactionType::Fee {
                transaction_ref: Some(2),
            },
            cash_flow: CashFlow {
                amount: CashAmount {
                    amount: -5.0,
                    currency: eur,
                },
                date: NaiveDate::from_ymd(2020, 1, 2),
            },
            note: None,
        });
        let positions = calc_position(eur, &transactions).unwrap();
        assert_fuzzy_eq!(positions.cash.position, 10000.0 - 104.0 - 5.0, tol);
        assert_eq!(positions.assets.len(), 1);
        let asset_pos_1 = positions.assets.get(&1).unwrap();
        assert_fuzzy_eq!(asset_pos_1.purchase_value, -104.0, tol);
        assert_fuzzy_eq!(asset_pos_1.position, 100.0, tol);
        assert_fuzzy_eq!(asset_pos_1.fees, -5.0, tol);
        assert_eq!(asset_pos_1.currency, eur);

        transactions.push(Transaction {
            id: Some(4),
            transaction_type: TransactionType::Asset {
                asset_id: 1,
                position: -50.0,
            },
            cash_flow: CashFlow {
                amount: CashAmount {
                    amount: 60.0,
                    currency: eur,
                },
                date: NaiveDate::from_ymd(2020, 1, 31),
            },
            note: None,
        });
        transactions.push(Transaction {
            id: Some(5),
            transaction_type: TransactionType::Fee {
                transaction_ref: Some(4),
            },
            cash_flow: CashFlow {
                amount: CashAmount {
                    amount: -3.0,
                    currency: eur,
                },
                date: NaiveDate::from_ymd(2020, 1, 31),
            },
            note: None,
        });
        transactions.push(Transaction {
            id: Some(6),
            transaction_type: TransactionType::Tax {
                transaction_ref: Some(4),
            },
            cash_flow: CashFlow {
                amount: CashAmount {
                    amount: -2.0,
                    currency: eur,
                },
                date: NaiveDate::from_ymd(2020, 1, 31),
            },
            note: None,
        });
        let positions = calc_position(eur, &transactions).unwrap();
        assert_fuzzy_eq!(
            positions.cash.position,
            10000.0 - 104.0 - 5.0 + 60.0 - 2.0 - 3.0,
            tol
        );
        assert_eq!(positions.assets.len(), 1);
        let asset_pos_1 = positions.assets.get(&1).unwrap();
        assert_fuzzy_eq!(asset_pos_1.purchase_value, -52.0, tol);
        assert_fuzzy_eq!(asset_pos_1.position, 50.0, tol);
        assert_fuzzy_eq!(asset_pos_1.fees, -8.0, tol);
        assert_fuzzy_eq!(asset_pos_1.realized_pnl, 8.0, tol);
        assert_eq!(asset_pos_1.currency, eur);

        transactions.push(Transaction {
            id: Some(7),
            transaction_type: TransactionType::Asset {
                asset_id: 1,
                position: 150.0,
            },
            cash_flow: CashFlow {
                amount: CashAmount {
                    amount: -140.0,
                    currency: eur,
                },
                date: NaiveDate::from_ymd(2020, 2, 15),
            },
            note: None,
        });
        transactions.push(Transaction {
            id: Some(8),
            transaction_type: TransactionType::Fee {
                transaction_ref: None,
            },
            cash_flow: CashFlow {
                amount: CashAmount {
                    amount: -7.0,
                    currency: eur,
                },
                date: NaiveDate::from_ymd(2020, 2, 25),
            },
            note: None,
        });
        transactions.push(Transaction {
            id: Some(9),
            transaction_type: TransactionType::Tax {
                transaction_ref: None,
            },
            cash_flow: CashFlow {
                amount: CashAmount {
                    amount: -4.5,
                    currency: eur,
                },
                date: NaiveDate::from_ymd(2020, 2, 26),
            },
            note: None,
        });
        transactions.push(Transaction {
            id: Some(10),
            transaction_type: TransactionType::Dividend { asset_id: 2 },
            cash_flow: CashFlow {
                amount: CashAmount {
                    amount: 13.0,
                    currency: eur,
                },
                date: NaiveDate::from_ymd(2020, 2, 27),
            },
            note: None,
        });
        transactions.push(Transaction {
            id: Some(11),
            transaction_type: TransactionType::Interest { asset_id: 3 },
            cash_flow: CashFlow {
                amount: CashAmount {
                    amount: 6.6,
                    currency: eur,
                },
                date: NaiveDate::from_ymd(2020, 2, 28),
            },
            note: None,
        });
        let positions = calc_position(eur, &transactions).unwrap();
        assert_fuzzy_eq!(
            positions.cash.position,
            10000.0 - 104.0 - 5.0 + 60.0 - 2.0 - 3.0 - 140.0 - 7.0 - 4.5 + 13.0 + 6.6,
            tol
        );
        assert_eq!(positions.assets.len(), 3);
        let asset_pos_1 = positions.assets.get(&1).unwrap();
        assert_fuzzy_eq!(asset_pos_1.purchase_value, -192.0, tol);
        assert_fuzzy_eq!(asset_pos_1.position, 200.0, tol);
        assert_fuzzy_eq!(asset_pos_1.fees, -8.0, tol);
        assert_fuzzy_eq!(asset_pos_1.realized_pnl, 8.0, tol);

        // fees and taxes not associated to any transaction
        assert_fuzzy_eq!(positions.cash.fees, -7.0, tol);
        assert_fuzzy_eq!(positions.cash.tax, -4.5, tol);

        // standalone dividends/interest
        let asset_pos_2 = positions.assets.get(&2).unwrap();
        assert_fuzzy_eq!(asset_pos_2.dividend, 13.0, tol);
        let asset_pos_3 = positions.assets.get(&3).unwrap();
        assert_fuzzy_eq!(asset_pos_3.interest, 6.6, tol);
    }
}

use cfg_if::cfg_if;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionFilter {
    pub user_id: u32,
    pub account_id: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransactionView {
    pub id: i32,
    pub group_id: Option<i32>,
    pub asset_id: Option<i32>,
    pub asset_name: Option<String>,
    pub position: Option<f64>,
    pub trans_type: String,
    pub cash_amount: f64,
    pub cash_currency: String,
    pub cash_date: String,
    pub note: Option<String>,
    pub account_id: i32,
    pub state: TransactionDisplay,
}

impl TransactionView {
    pub fn new(id: i32) -> Self {
        Self {
            id,
            group_id: None,
            asset_id: None,
            asset_name: None,
            position: None,
            trans_type: String::new(),
            cash_amount: 0.0,
            cash_currency: "EUR".to_string(),
            cash_date: "2025-01-01".to_string(),
            note: None,
            account_id: 0,
            state: TransactionDisplay::Edit,
        }
    }
    pub fn trans_type_getter(&self) -> String {
        match self.trans_type.as_str() {
            "a" => "Asset".to_string(),
            "c" => "Cash".to_string(),
            _ => "unknown".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TransactionDisplay {
    View,
    Edit,
}

impl TransactionDisplay {
    pub fn get_icon(&self, index: usize) -> &'static str {
        match index {
            1 => match self {
                TransactionDisplay::View => "locked.svg",
                TransactionDisplay::Edit => "check.svg",
            },
            _ => match self {
                TransactionDisplay::View => "cross.svg",
                TransactionDisplay::Edit => "unlocked.svg",
            },
        }
    }
}

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use finql::postgres::PostgresDB;
        use qualinvest_core::{
            accounts::AccountHandler,
            user::UserHandler,
        };

        pub async fn get_transactions_ssr(account_id: i32, db: PostgresDB) -> Vec<TransactionView> {
            let accounts_to_query = vec![account_id];

            if let Ok(transactions) = db
                .get_transaction_view_for_accounts(&accounts_to_query)
                .await
            {
                transactions.into_iter().map(|t| TransactionView {
                    id: t.id,
                    group_id: t.group_id,
                    asset_id: t.asset_id,
                    asset_name: t.asset_name,
                    position: t.position,
                    trans_type: t.trans_type,
                    cash_amount: t.cash_amount,
                    cash_currency: t.cash_currency,
                    cash_date: t.cash_date,
                    note: t.note,
                    account_id: t.account_id,
                    state: TransactionDisplay::View,
                }).collect()
            } else {
                Vec::new()
            }
        }
    }
}

#[server(Transactions, "/api")]
pub async fn get_transactions(
    filter: TransactionFilter,
) -> Result<RwSignal<Vec<TransactionView>>, ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use log::debug;

    debug!("get transactions called with filter {filter:?}");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    // Verify the authenticated user matches the requested user_id or is an admin
    let db = crate::db::get_db()?;
    if !user.is_admin {
        if user.id != filter.user_id as i32 {
            return Err(ServerFnError::new(
                "Forbidden: Cannot access other user's transactions",
            ));
        }
        let user_settings = db.get_user_settings(user.id).await;
        if !user_settings.account_ids.contains(&filter.account_id) {
            debug!(
                "User {} does not have access to account {}",
                user.id, filter.account_id
            );
            debug!("User settings: {:?}", user_settings);
            return Err(ServerFnError::new("Forbidden: Cannot access account"));
        }
    }

    Ok(RwSignal::new(
        get_transactions_ssr(filter.account_id, db).await,
    ))
}

#[server(InsertTransaction, "/api")]
pub async fn insert_transaction(
    transaction: TransactionView,
    user_id: i32,
) -> Result<i32, ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use finql::datatypes::{
        Asset, AssetHandler, CashAmount, CashFlow, CurrencyISOCode, Stock, Transaction,
        TransactionHandler, TransactionType,
    };
    use log::debug;
    use qualinvest_core::accounts::AccountHandler;
    use time::Date;

    debug!("insert transaction called with transaction {transaction:?}");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    // Verify the authenticated user matches the requested user_id or is an admin
    if !user.is_admin && user.id != user_id {
        return Err(ServerFnError::new(
            "Forbidden: Cannot insert transactions for other users",
        ));
    }

    let db = crate::db::get_db()?;

    // Parse date
    let date = Date::parse(
        &transaction.cash_date,
        &time::format_description::well_known::Iso8601::DEFAULT,
    )
    .map_err(|e| ServerFnError::new(format!("Invalid date format: {}", e)))?;

    let market = crate::db::get_market()?;
    let currency = market
        .get_currency(CurrencyISOCode::new(&transaction.cash_currency)?)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to get currency: {}", e)))?;
    debug!("Currency: {currency:?}");

    // Determine transaction type
    let transaction_type = match transaction.trans_type.as_str() {
        "a" => {
            let asset_id = if let Some(asset_id) = transaction.asset_id {
                asset_id
            } else {
                db.get_asset_id(&Asset::Stock(Stock {
                    id: None,
                    name: transaction
                        .asset_name
                        .ok_or_else(|| {
                            ServerFnError::new("Asset transaction requires existing asset")
                        })?
                        .clone(),
                    wkn: None,
                    isin: None,
                    note: None,
                }))
                .await
                .ok_or_else(|| ServerFnError::new("Asset transaction requires existing asset"))?
            };
            let position = transaction
                .position
                .ok_or_else(|| ServerFnError::new("Asset transaction requires position"))?;
            TransactionType::Asset { asset_id, position }
        }
        "c" => TransactionType::Cash,
        _ => return Err(ServerFnError::new("Invalid transaction type")),
    };
    debug!("Transaction Type: {transaction_type:?}");

    // Create transaction
    let new_transaction = Transaction {
        id: None,
        transaction_type,
        cash_flow: CashFlow {
            amount: CashAmount {
                amount: transaction.cash_amount,
                currency,
            },
            date,
        },
        note: transaction.note,
    };
    debug!("New Transaction: {new_transaction:?}");

    // Insert transaction
    let transaction_id = db
        .insert_transaction(&new_transaction)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to insert transaction: {}", e)))?;
    debug!("Transaction ID: {transaction_id:?}");

    // Link transaction to account
    debug!(
        "Try linking transaction {transaction_id} to account {}",
        transaction.account_id
    );
    db.add_transaction_to_account(transaction.account_id, transaction_id)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to link transaction to account: {}", e)))?;
    debug!(
        "Transaction {transaction_id} linked to account {}",
        transaction.account_id
    );

    Ok(transaction_id)
}

#[server(UpdateTransaction, "/api")]
pub async fn update_transaction(
    transaction: TransactionView,
    user_id: i32,
) -> Result<(), ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use finql::datatypes::{
        CashAmount, CashFlow, CurrencyISOCode, Transaction, TransactionHandler, TransactionType,
    };
    use log::debug;
    use qualinvest_core::accounts::AccountHandler;
    use time::Date;

    debug!("update transaction called with transaction {transaction:?}");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    // Verify the authenticated user matches the requested user_id or is an admin
    if !user.is_admin && user.id != user_id {
        return Err(ServerFnError::new(
            "Forbidden: Cannot update transactions for other users",
        ));
    }

    let db = crate::db::get_db()?;

    // Verify the transaction belongs to one of the user's accounts
    let transaction_account_id = db
        .get_transactions_account_id(transaction.id)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to get transaction account: {}", e)))?;
    if transaction_account_id != transaction.account_id {
        return Err(ServerFnError::new(format!("Account ID missmatch. Existing account ID {transaction_account_id} differs from new account ID {}", transaction.account_id)));
    }
    debug!(
        "Verified transaction account ID: {}",
        transaction_account_id
    );
    if !user.is_admin {
        let user_settings = db.get_user_settings(user_id).await;
        if !user_settings.account_ids.contains(&transaction_account_id) {
            return Err(ServerFnError::new(
                "Forbidden: Transaction does not belong to your accounts",
            ));
        }
    }

    // Parse date
    let date = Date::parse(
        &transaction.cash_date,
        &time::format_description::well_known::Iso8601::DEFAULT,
    )
    .map_err(|e| ServerFnError::new(format!("Invalid date format: {}", e)))?;

    let market = crate::db::get_market()?;
    let currency = market
        .get_currency(CurrencyISOCode::new(&transaction.cash_currency)?)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to get currency: {}", e)))?;

    // Determine transaction type
    let transaction_type = match transaction.trans_type.as_str() {
        "a" => {
            let asset_id = transaction
                .asset_id
                .ok_or_else(|| ServerFnError::new("Asset transaction requires asset_id"))?;
            let position = transaction
                .position
                .ok_or_else(|| ServerFnError::new("Asset transaction requires position"))?;
            TransactionType::Asset { asset_id, position }
        }
        "c" => TransactionType::Cash,
        _ => return Err(ServerFnError::new("Invalid transaction type")),
    };

    // Create updated transaction
    let updated_transaction = Transaction {
        id: Some(transaction.id),
        transaction_type,
        cash_flow: CashFlow {
            amount: CashAmount {
                amount: transaction.cash_amount,
                currency,
            },
            date,
        },
        note: transaction.note,
    };

    // Update transaction
    db.update_transaction(&updated_transaction)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to update transaction: {}", e)))
}

#[server(DeleteTransaction, "/api")]
pub async fn delete_transaction(transaction_id: i32, user_id: i32) -> Result<(), ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    use finql::datatypes::TransactionHandler;
    use log::debug;
    use qualinvest_core::accounts::AccountHandler;

    debug!("delete transaction called with id {transaction_id}");

    let auth: AuthSession<PostgresBackend> = expect_context();
    let user = auth
        .user
        .ok_or_else(|| ServerFnError::new("Unauthorized"))?;

    // Verify the authenticated user matches the requested user_id or is an admin
    if !user.is_admin && user.id != user_id {
        return Err(ServerFnError::new(
            "Forbidden: Cannot delete transactions for other users",
        ));
    }

    let db = crate::db::get_db()?;

    // Verify the transaction belongs to one of the user's accounts
    let transaction_account_id = db
        .get_transactions_account_id(transaction_id)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to get transaction account: {}", e)))?;

    let user_settings = db.get_user_settings(user_id).await;
    if !user_settings.account_ids.contains(&transaction_account_id) {
        return Err(ServerFnError::new(
            "Forbidden: Transaction does not belong to your accounts",
        ));
    }

    // Delete transaction
    db.delete_transaction(transaction_id)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to delete transaction: {}", e)))
}

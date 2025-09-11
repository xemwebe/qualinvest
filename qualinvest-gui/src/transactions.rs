use cfg_if::cfg_if;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionFilter {
    pub user_id: u32,
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
    pub doc_path: Option<String>,
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
            doc_path: None,
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

        pub fn get_db() -> Result<PostgresDB, ServerFnError> {
            use_context::<PostgresDB>()
                .ok_or_else(|| ServerFnError::ServerError("Database is missing.".into()))
        }

        pub async fn get_transactions_ssr(user_id: u32, db: PostgresDB) -> Vec<TransactionView> {
            let user_settings = db.get_user_settings(user_id as i32).await;
            if let Ok(transactions) = db
                .get_transaction_view_for_accounts(&user_settings.account_ids)
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
                    doc_path: t.doc_path,
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
    use log::debug;

    debug!("get transactions called with filter {filter:?}");
    let db = get_db()?;
    Ok(RwSignal::new(
        get_transactions_ssr(filter.user_id, db).await,
    ))
}

///! Implementation of Accounts and an according PostgreSQL handler
use async_trait::async_trait;
use serde::{Serialize,Deserialize};
use chrono::{NaiveDate};
use sqlx::Row;

use finql_data::{DataError, TransactionHandler, Transaction};
use finql_postgres::{PostgresDB, transaction_handler::RawTransaction};

#[derive(Debug,Serialize,Deserialize)]
pub struct Account {
    pub id: Option<usize>,
    pub broker: String,
    pub account_name: String,
}

#[derive(Debug,Serialize,Deserialize)]
pub struct TransactionView {
    pub id: usize,
    pub group_id: usize,
    pub asset_name: Option<String>,
    pub asset_id: Option<usize>,
    pub position: Option<f64>,
    pub trans_type: String,
    pub cash_amount: f64,
    pub cash_currency: String,
    pub cash_date: String,
    pub note: Option<String>,
    pub doc_path: Option<String>, 
    pub account_id: usize,
}

/// Handler for asset depot accounts
#[async_trait]
pub trait AccountHandler: TransactionHandler {
    /// Clean database by dropping all tables and than run init
    async fn clean_accounts(&self) -> Result<(), sqlx::Error>;

    /// Set up new table for account management
    async fn init_accounts(&self) -> Result<(), sqlx::Error>;

    /// Insert new account info in database, if it not yet exist
    async fn insert_account_if_new(&self, account: &Account) -> Result<usize, DataError>;

    /// Get account id for given account
    async fn get_account_id(&self, account: &Account) -> Result<usize, DataError>;

    /// Get all accounts form db
    async fn get_all_account_ids(&self) -> Result<Vec<usize>, DataError>;

    /// Get list of all accounts
    async fn get_all_accounts(&self) -> Result<Vec<Account>, DataError>;

    /// Add a transaction to the account
    async fn add_transaction_to_account(
        &self,
        account: usize,
        transaction: usize,
    ) -> Result<(), DataError>;

    /// Check if we have already parsed a given document by look-up its hash
    /// If successful, return the transaction ids and the path of the document
    async fn lookup_hash(&self, hash: &str) -> Result<(Vec<usize>, String), DataError>;
    /// Insert document information for successfully parsed documents
    async fn insert_doc(
        &self,
        transaction_ids: &[usize],
        hash: &str,
        path: &str,
    ) -> Result<(), DataError>;

    /// Get transactions filtered by account id
    async fn get_all_transactions_with_account(
        &self,
        account_id: usize,
    ) -> Result<Vec<Transaction>, DataError>;

    /// Get transactions filtered by account id before time
    async fn get_all_transactions_with_account_before(
        &self,
        account_id: usize,
        time: NaiveDate,
    ) -> Result<Vec<Transaction>, DataError>;

    /// Get transactions filtered by account id in time range
    async fn get_all_transactions_with_account_in_range (
        &self,
        account_id: usize,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Transaction>, DataError>;

    /// Get transactions filtered by a list of account ids
    async fn get_all_transactions_with_accounts(
        &self,
        accounts: &[usize],
    ) -> Result<Vec<Transaction>, DataError>;

    /// Get transactions filtered by a list of account ids and cash dates before time
    async fn get_transactions_before_time(
        &self,
        accounts: &[usize],
        time: NaiveDate,
    ) -> Result<Vec<Transaction>, DataError>;

    /// Get transactions filtered by a list of account ids and cash dates in time range
    async fn get_transactions_in_range(
        &self,
        accounts: &[usize],
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Transaction>, DataError>;

    /// Get transactions view for list of account ids that a related to a given asset
    async fn get_transaction_view_for_accounts_and_asset(&self, accounts: &[usize], asset_id: usize) -> Result<Vec<TransactionView>, DataError>;

    /// Get transactions view by accounts
    async fn get_transaction_view_for_accounts(
        &self,
        accounts: &[usize],
    ) -> Result<Vec<TransactionView>, DataError>;

    /// Change the account a transaction identified by id belongs to
    async fn change_transaction_account(&self, transaction_id: usize, 
        old_account_id: usize, new_account_id: usize) -> Result<(), DataError>;
}

#[async_trait]
impl AccountHandler for PostgresDB {
    /// Clean database by dropping all tables and than run init
    async fn clean_accounts(&self) -> Result<(), sqlx::Error> {
        sqlx::query!("DROP TABLE IF EXISTS account_transactions").execute(&self.pool).await?;
        sqlx::query!("DROP TABLE IF EXISTS accounts").execute(&self.pool).await?;
        sqlx::query!("DROP TABLE IF EXISTS documents").execute(&self.pool).await?;
        sqlx::query!("DROP TABLE IF EXISTS users").execute(&self.pool).await?;
        self.init_accounts().await?;
        Ok(())
    }

    /// Set up new table for account management
    async fn init_accounts(&self) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "CREATE TABLE IF NOT EXISTS users (
                id SERIAL PRIMARY KEY,
                name TEXT NOT NULL,
                display TEXT NOT NULL,
                salt_hash TEXT NOT NULL,
                is_admin BOOLEAN NOT NULL DEFAULT False,
                UNIQUE (name))")
            .execute(&self.pool).await?;
        sqlx::query!(
            "CREATE TABLE IF NOT EXISTS accounts (
                id SERIAL PRIMARY KEY,
                broker TEXT NOT NULL,
                account_name TEXT NOT NULL,
                UNIQUE (broker, account_name))")
            .execute(&self.pool).await?;
        sqlx::query!(
            "CREATE TABLE IF NOT EXISTS account_transactions (
                id SERIAL PRIMARY KEY,
                account_id INTEGER NOT NULL,
                transaction_id INTEGER NOT NULL,
                FOREIGN KEY(account_id) REFERENCES accounts(id),
                FOREIGN KEY(transaction_id) REFERENCES transactions(id))")
            .execute(&self.pool).await?;
        sqlx::query!(
            "CREATE TABLE IF NOT EXISTS documents (
                id SERIAL PRIMARY KEY,
                transaction_id INTEGER NOT NULL,
                hash TEXT NOT NULL,
                path TEXT NOT NULL,
                FOREIGN KEY(transaction_id) REFERENCES transactions(id))")
            .execute(&self.pool).await?;
        Ok(())
    }

    /// Insert new account info in database
    async fn insert_account_if_new(&self, account: &Account) -> Result<usize, DataError> {
        let id = self.get_account_id(account).await;
        match id {
            Ok(id) => Ok(id),
            _ => {
                let row = sqlx::query!(
                        "INSERT INTO accounts (broker, account_name) VALUES ($1, $2) RETURNING id",
                        account.broker, account.account_name).fetch_one(&self.pool).await
                    .map_err(|e| DataError::InsertFailed(e.to_string()))?;
                let id: i32 = row.id;
                Ok(id as usize)
            }
        }
    }

    /// Get ID of given account
    async fn get_account_id(&self, account: &Account) -> Result<usize, DataError> {
        let row = sqlx::query!(
                "SELECT id FROM accounts where broker=$1 AND account_name=$2",
                &account.broker, &account.account_name,
            ).fetch_one(&self.pool).await
            .map_err(|e| DataError::NotFound(e.to_string()))?;
        let id: i32 = row.id;
        Ok(id as usize)
    }

    async fn get_all_account_ids(&self) -> Result<Vec<usize>, DataError> {
        let rows = sqlx::query!(
                "SELECT id FROM accounts").fetch_all(&self.pool).await
            .map_err(|e| DataError::NotFound(e.to_string()))?;
        let mut ids = Vec::new();
        for row in rows {
            let id: i32 = row.id;
            ids.push(id as usize);
        }
        Ok(ids)
    }

    async fn get_all_accounts(&self) -> Result<Vec<Account>, DataError> {
        let rows = sqlx::query!(
                "SELECT id, broker, account_name FROM accounts"
            ).fetch_all(&self.pool).await
            .map_err(|e| DataError::NotFound(e.to_string()))?;
        let mut accounts = Vec::new();
        for row in rows {
            let id: i32 = row.id;
            accounts.push(Account{
                id: Some(id as usize),
                broker: row.broker,
                account_name: row.account_name,
            });
        }
        Ok(accounts)
    }

    /// Insert transaction to account relation
    async fn add_transaction_to_account(
        &self,
        account: usize,
        transaction: usize,
    ) -> Result<(), DataError> {
        sqlx::query!(
                "INSERT INTO account_transactions (account_id, transaction_id) VALUES ($1, $2)",
                (account as i32), &(transaction as i32),
            ).execute(&self.pool).await
            .map_err(|e| DataError::InsertFailed(e.to_string()))?;
        Ok(())
    }

    /// Insert document information for successfully parsed documents
    async fn lookup_hash(&self, hash: &str) -> Result<(Vec<usize>, String), DataError> {
        let mut trans_ids = Vec::new();
        let mut path = "".to_string();
        for row in sqlx::query!(
                "SELECT transaction_id, path FROM documents WHERE hash=$1",
                hash).fetch_all(&self.pool).await
            .map_err(|e| DataError::NotFound(e.to_string()))?
        {
            let trans: i32 = row.transaction_id;
            trans_ids.push(trans as usize);
            path = row.path;
        }
        Ok((trans_ids, path))
    }

    /// Insert document information for successfully parsed documents
    async fn insert_doc(
        &self,
        transaction_ids: &[usize],
        hash: &str,
        path: &str,
    ) -> Result<(), DataError> {
        for trans_id in transaction_ids {
            sqlx::query!(
                    "INSERT INTO documents (transaction_id, hash, path) VALUES ($1, $2, $3)",
                    (*trans_id as i32), hash, path
                ).execute(&self.pool).await
                .map_err(|e| DataError::InsertFailed(e.to_string()))?;
        }
        Ok(())
    }

    /// Get transactions filtered by account id
    async fn get_all_transactions_with_account(
        &self,
        account_id: usize,
    ) -> Result<Vec<Transaction>, DataError> {
        let mut transactions = Vec::new();
        for row in sqlx::query!(
                "SELECT t.id, t.trans_type, t.asset_id, 
        t.cash_amount, t.cash_currency, t.cash_date, t.related_trans, t.position, t.note 
        FROM transactions t, account_transactions a WHERE a.account_id = $1 and a.transaction_id = t.id",
                (account_id as i32),        
            ).fetch_all(&self.pool).await
            .map_err(|e| DataError::NotFound(e.to_string()))?
        {
            let transaction = RawTransaction {
                id: Some(row.id),
                trans_type: row.trans_type,
                asset: row.asset_id,
                cash_amount: row.cash_amount,
                cash_currency: row.cash_currency,
                cash_date: row.cash_date,
                related_trans: row.related_trans,
                position: row.position,
                note: row.note,
            };
            transactions.push(transaction.to_transaction()?);
        }
        Ok(transactions)
    }

    /// Get transactions filtered by account id before time
    async fn get_all_transactions_with_account_before(
        &self,
        account_id: usize,
        time: NaiveDate,
    ) -> Result<Vec<Transaction>, DataError> {
        println!("Get all transactions for account id {} before time {}", account_id, time);

        let mut transactions = Vec::new();
        for row in sqlx::query!(
                "SELECT t.id, t.trans_type, t.asset_id, 
        t.cash_amount, t.cash_currency, t.cash_date, t.related_trans, t.position, t.note 
        FROM transactions t, account_transactions a WHERE a.account_id = $1 AND a.transaction_id = t.id AND t.cash_date < $2",
                (account_id as i32), time,
            ).fetch_all(&self.pool).await
            .map_err(|e| DataError::NotFound(e.to_string()))?
        {
            let transaction = RawTransaction {
                id: Some(row.id),
                trans_type: row.trans_type,
                asset: row.asset_id,
                cash_amount: row.cash_amount,
                cash_currency: row.cash_currency,
                cash_date: row.cash_date,
                related_trans: row.related_trans,
                position: row.position,
                note: row.note,
            };
            transactions.push(transaction.to_transaction()?);
        }
        Ok(transactions)
    }

    /// Get transactions filtered by account id in time range
    async fn get_all_transactions_with_account_in_range(
        &self,
        account_id: usize,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Transaction>, DataError> {
        let mut transactions = Vec::new();
        for row in sqlx::query!(
                "SELECT t.id, t.trans_type, t.asset_id, 
        t.cash_amount, t.cash_currency, t.cash_date, t.related_trans, t.position, t.note 
        FROM transactions t, account_transactions a WHERE a.account_id = $1 AND a.transaction_id = t.id AND t.cash_date BETWEEN $2 AND $3",
                (account_id as i32), start, end,
            ).fetch_all(&self.pool).await
            .map_err(|e| DataError::NotFound(e.to_string()))?
        {
            let transaction = RawTransaction {
                id: Some(row.id),
                trans_type: row.trans_type,
                asset: row.asset_id,
                cash_amount: row.cash_amount,
                cash_currency: row.cash_currency,
                cash_date: row.cash_date,
                related_trans: row.related_trans,
                position: row.position,
                note: row.note,
            };
            transactions.push(transaction.to_transaction()?);
        }
        Ok(transactions)
    }

    /// Get transactions filtered by a list of account ids
    async fn get_all_transactions_with_accounts(
        &self,
        accounts: &[usize],
    ) -> Result<Vec<Transaction>, DataError> {
        let mut transactions = Vec::new();
        for i in accounts {
            transactions.extend(self.get_all_transactions_with_account(*i).await?);
        }
        Ok(transactions)
    }

    /// Get transactions filtered by a list of account ids and cash dates before time
    async fn get_transactions_before_time(
        &self,
        accounts: &[usize],
        time: NaiveDate,
    ) -> Result<Vec<Transaction>, DataError> {
        let mut transactions = Vec::new();
        for i in accounts {
            transactions.extend(self.get_all_transactions_with_account_before(*i, time).await?);
        }
        Ok(transactions)
    }

    /// Get transactions filtered by a list of account ids and cash dates in time range
    async fn get_transactions_in_range(
        &self,
        accounts: &[usize],
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Transaction>, DataError> {
        let mut transactions = Vec::new();
        for i in accounts {
            transactions.extend(self.get_all_transactions_with_account_in_range(*i, start, end).await?);
        }
        Ok(transactions)
    }


    /// Get transactions view by accounts
    async fn get_transaction_view_for_accounts(
        &self,
        accounts: &[usize],
    ) -> Result<Vec<TransactionView>, DataError> {
        if accounts.is_empty(){
            return Err(DataError::DataAccessFailure("transaction view requires account list".to_string()));
        }
        let mut query_string = r#"SELECT
        t.id
        ,(CASE WHEN t.related_trans IS null THEN t.id 
         ELSE t.related_trans
         END) AS group_id
        , a.name
        , a.id AS asset_id
        , t.position
        , t.trans_type
        , t.cash_amount
        , t.cash_currency
        , t.cash_date
        , t.note
        , d.path
        , at.account_id
    FROM transactions t
    LEFT JOIN assets a ON a.id = t.asset_id
    LEFT JOIN documents d ON d.transaction_id = t.id
    JOIN account_transactions at ON at.transaction_id = t.id
    WHERE at.account_id IN ("#.to_string();
        query_string = format!("{}{}",query_string, accounts[0]);
        for id in &accounts[1..] {
            query_string = format!("{},{}",query_string, *id);
        }
        query_string = format!("{}{}", query_string,
        r#")
    ORDER BY t.cash_date desc, group_id, t.id;
        "#);
        let mut transactions = Vec::new();
        for row in sqlx::query(&query_string)
            .fetch_all(&self.pool).await
            .map_err(|e| DataError::NotFound(e.to_string()))?
        {
            {
            let id: i32 = row.try_get("id").map_err(|e| DataError::NotFound(e.to_string()))?;
            let group_id: i32 = row.try_get("group_id").map_err(|e| DataError::NotFound(e.to_string()))?;
            let asset_id: Option<i32> = row.try_get("asset_id").map_err(|e| DataError::NotFound(e.to_string()))?;
            let asset_id = asset_id.map(|id| id as usize);
            let account_id: i32 = row.try_get("account_id").map_err(|e| DataError::NotFound(e.to_string()))?;
            let date: chrono::NaiveDate = row.try_get("cash_date").map_err(|e| DataError::NotFound(e.to_string()))?;
            let cash_date = date.format("%Y-%m-%d").to_string();          
            transactions.push( TransactionView {
                id: id as usize,
                group_id: group_id as usize,
                asset_name: row.try_get("name").map_err(|e| DataError::NotFound(e.to_string()))?,
                asset_id,
                position: row.try_get("position").map_err(|e| DataError::NotFound(e.to_string()))?,
                trans_type: row.try_get("trans_type").map_err(|e| DataError::NotFound(e.to_string()))?,
                cash_amount: row.try_get("cash_amount").map_err(|e| DataError::NotFound(e.to_string()))?,
                cash_currency: row.try_get("cash_currency").map_err(|e| DataError::NotFound(e.to_string()))?,
                cash_date, 
                note: row.try_get("note").map_err(|e| DataError::NotFound(e.to_string()))?,
                doc_path: row.try_get("path").map_err(|e| DataError::NotFound(e.to_string()))?, 
                account_id: account_id as usize,
            });
        }
        }
        Ok(transactions)
    }


    /// Get transactions view for list of account ids that are related to a given asset
    async fn get_transaction_view_for_accounts_and_asset(&self, accounts: &[usize], asset_id: usize) -> Result<Vec<TransactionView>, DataError> {
        if accounts.is_empty() {
            return Err(DataError::DataAccessFailure("transaction view requires account list".to_string()));
        }
        let mut query_string = r#"SELECT
        t.id
        ,(CASE WHEN t.related_trans IS null THEN t.id 
         ELSE t.related_trans
         END) AS group_id
        , a.name
        , a.id AS asset_id
        , t.position
        , t.trans_type
        , t.cash_amount
        , t.cash_currency
        , t.cash_date
        , t.note
        , d.path
        , at.account_id
    FROM transactions t
    LEFT JOIN assets a ON a.id = t.asset_id
    LEFT JOIN documents d ON d.transaction_id = t.id
    JOIN account_transactions at ON at.transaction_id = t.id
    WHERE "#.to_string();
        query_string = format!("{} a.id = {} AND at.account_id IN ({}",query_string, asset_id, accounts[0]);
        for id in &accounts[1..] {
            query_string = format!("{},{}",query_string, *id);
        }
        query_string = format!("{}{}", query_string,
        r#")
    ORDER BY t.cash_date desc, group_id, t.id;
        "#);
        let mut transactions = Vec::new();
        for row in sqlx::query(query_string.as_str())
            .fetch_all(&self.pool).await
            .map_err(|e| DataError::NotFound(e.to_string()))?
        {
            let id: i32 = row.try_get("id").map_err(|e| DataError::NotFound(e.to_string()))?;
            let group_id: i32 = row.try_get("group_id").map_err(|e| DataError::NotFound(e.to_string()))?;
            let asset_id: Option<i32> = row.try_get("asset_id").map_err(|e| DataError::NotFound(e.to_string()))?;
            let asset_id = asset_id.map(|id| id as usize);
            let account_id: i32 = row.try_get("account_id").map_err(|e| DataError::NotFound(e.to_string()))?;
            let date: chrono::NaiveDate = row.try_get("cash_date").map_err(|e| DataError::NotFound(e.to_string()))?;
            let cash_date = date.format("%Y-%m-%d").to_string();          
            transactions.push( TransactionView {
                id: id as usize,
                group_id: group_id as usize,
                asset_name: row.try_get("name").map_err(|e| DataError::NotFound(e.to_string()))?,
                asset_id,
                position: row.try_get("position").map_err(|e| DataError::NotFound(e.to_string()))?,
                trans_type: row.try_get("trans_type").map_err(|e| DataError::NotFound(e.to_string()))?,
                cash_amount: row.try_get("cash_amount").map_err(|e| DataError::NotFound(e.to_string()))?,
                cash_currency: row.try_get("cash_currency").map_err(|e| DataError::NotFound(e.to_string()))?,
                cash_date, 
                note: row.try_get("note").map_err(|e| DataError::NotFound(e.to_string()))?,
                doc_path: row.try_get("path").map_err(|e| DataError::NotFound(e.to_string()))?, 
                account_id: account_id as usize,
            });
        }
        Ok(transactions)
    }

    /// Change the account a transaction identified by id belongs to
    async fn change_transaction_account(&self, transaction_id: usize, old_account_id: usize, new_account_id: usize) -> Result<(), DataError> {
        sqlx::query!(
            "UPDATE account_transactions SET account_id=$3 WHERE transaction_id=$1 AND account_id=$2",
            (transaction_id as i32), (old_account_id as i32), (new_account_id as i32))
            .execute(&self.pool).await
            .map_err(|e| DataError::UpdateFailed(e.to_string()))?;
    Ok(())
    }

}

///! Implementation of Accounts and an according PostgreSQL handler
use async_trait::async_trait;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use finql::{
    datatypes::{Currency, CurrencyISOCode, DataError, Transaction, TransactionHandler},
    postgres::transaction_handler::RawTransaction,
    postgres::PostgresDB,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: Option<i32>,
    pub broker: String,
    pub account_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionView {
    pub id: i32,
    pub group_id: Option<i32>,
    pub asset_id: Option<i32>,
    pub position: Option<f64>,
    pub trans_type: String,
    pub cash_amount: f64,
    pub cash_currency: String,
    pub cash_date: String,
    pub note: Option<String>,
    pub doc_path: Option<String>,
    pub account_id: i32,
}

/// Handler for asset depot accounts
#[async_trait]
pub trait AccountHandler: TransactionHandler {
    /// Clean database by dropping all tables and than run init
    async fn clean_accounts(&self) -> Result<(), sqlx::Error>;

    /// Set up new table for account management
    async fn init_accounts(&self) -> Result<(), sqlx::Error>;

    /// Insert new account info in database, if it not yet exist
    async fn insert_account_if_new(&self, account: &Account) -> Result<i32, DataError>;

    /// Update an existing accounts name and/or broker
    async fn update_account(&self, account: &Account) -> Result<(), DataError>;

    /// Remove account from database
    /// Fails if it does not exist or is referenced by other tables.
    async fn delete_account(&self, account_id: i32) -> Result<(), DataError>;

    /// Get account id for given account
    async fn get_account_id(&self, account: &Account) -> Result<i32, DataError>;

    /// Get all accounts form db
    async fn get_all_account_ids(&self) -> Result<Vec<i32>, DataError>;

    /// Get list of all accounts
    async fn get_all_accounts(&self) -> Vec<Account>;

    /// Add a transaction to the account
    async fn add_transaction_to_account(
        &self,
        account: i32,
        transaction: i32,
    ) -> Result<(), DataError>;

    /// Check if we have already parsed a given document by look-up its hash
    /// If successful, return the transaction ids and the path of the document
    async fn lookup_hash(&self, hash: &str) -> Result<(Vec<i32>, String), DataError>;
    /// Insert document information for successfully parsed documents
    async fn insert_doc(
        &self,
        transaction_ids: &[i32],
        hash: &str,
        path: &str,
    ) -> Result<(), DataError>;

    /// Get document path for given transaction
    async fn get_doc_path(&self, transaction_id: i32) -> Result<String, DataError>;
    /// Get id of account a transaction belongs to
    async fn get_transactions_account_id(&self, transaction_id: i32) -> Result<i32, DataError>;

    /// Get transactions filtered by account id
    async fn get_all_transactions_with_account(
        &self,
        account_id: i32,
    ) -> Result<Vec<Transaction>, DataError>;

    /// Get transactions filtered by account id before time
    async fn get_all_transactions_with_account_before(
        &self,
        account_id: i32,
        time: NaiveDate,
    ) -> Result<Vec<Transaction>, DataError>;

    /// Get transactions filtered by account id in time range
    async fn get_all_transactions_with_account_in_range(
        &self,
        account_id: i32,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Transaction>, DataError>;

    /// Get transactions filtered by a list of account ids
    async fn get_all_transactions_with_accounts(
        &self,
        accounts: &[i32],
    ) -> Result<Vec<Transaction>, DataError>;

    /// Get transactions filtered by a list of account ids and cash dates before time
    async fn get_transactions_before_time(
        &self,
        accounts: &[i32],
        time: NaiveDate,
    ) -> Result<Vec<Transaction>, DataError>;

    /// Get transactions filtered by a list of account ids and cash dates in time range
    async fn get_transactions_in_range(
        &self,
        accounts: &[i32],
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Transaction>, DataError>;

    /// Get transactions view for list of account ids that a related to a given asset
    async fn get_transaction_view_for_accounts_and_asset(
        &self,
        accounts: &[i32],
        asset_id: i32,
    ) -> Result<Vec<TransactionView>, DataError>;

    /// Get transactions view by accounts
    async fn get_transaction_view_for_accounts(
        &self,
        accounts: &[i32],
    ) -> Result<Vec<TransactionView>, DataError>;

    /// Change the account a transaction identified by id belongs to
    async fn change_transaction_account(
        &self,
        transaction_id: i32,
        old_account_id: i32,
        new_account_id: i32,
    ) -> Result<(), DataError>;
}

#[async_trait]
impl AccountHandler for PostgresDB {
    /// Clean database by dropping all tables and than run init
    async fn clean_accounts(&self) -> Result<(), sqlx::Error> {
        sqlx::query!("DROP TABLE IF EXISTS account_transactions")
            .execute(&self.pool)
            .await?;
        sqlx::query!("DROP TABLE IF EXISTS accounts")
            .execute(&self.pool)
            .await?;
        sqlx::query!("DROP TABLE IF EXISTS documents")
            .execute(&self.pool)
            .await?;
        sqlx::query!("DROP TABLE IF EXISTS users")
            .execute(&self.pool)
            .await?;
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
                UNIQUE (name))"
        )
        .execute(&self.pool)
        .await?;
        sqlx::query!(
            "CREATE TABLE IF NOT EXISTS accounts (
                id SERIAL PRIMARY KEY,
                broker TEXT NOT NULL,
                account_name TEXT NOT NULL,
                UNIQUE (broker, account_name))"
        )
        .execute(&self.pool)
        .await?;
        sqlx::query!(
            "CREATE TABLE IF NOT EXISTS account_transactions (
                id SERIAL PRIMARY KEY,
                account_id INTEGER NOT NULL,
                transaction_id INTEGER NOT NULL,
                FOREIGN KEY(account_id) REFERENCES accounts(id),
                FOREIGN KEY(transaction_id) REFERENCES transactions(id))"
        )
        .execute(&self.pool)
        .await?;
        sqlx::query!(
            "CREATE TABLE IF NOT EXISTS documents (
                id SERIAL PRIMARY KEY,
                transaction_id INTEGER NOT NULL,
                hash TEXT NOT NULL,
                path TEXT NOT NULL,
                FOREIGN KEY(transaction_id) REFERENCES transactions(id))"
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Insert new account info in database
    async fn insert_account_if_new(&self, account: &Account) -> Result<i32, DataError> {
        let id = self.get_account_id(account).await;
        match id {
            Ok(id) => Ok(id),
            _ => {
                let row = sqlx::query!(
                    "INSERT INTO accounts (broker, account_name) VALUES ($1, $2) RETURNING id",
                    account.broker,
                    account.account_name
                )
                .fetch_one(&self.pool)
                .await?;
                Ok(row.id)
            }
        }
    }

    /// Update an existing accounts name and/or broker
    async fn update_account(&self, account: &Account) -> Result<(), DataError> {
        if let Some(id) = account.id {
            sqlx::query!(
                "UPDATE accounts SET 
                    account_name=$1, 
                    broker=$2
                WHERE id=($3)",
                account.account_name,
                account.broker,
                id
            )
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Remove account from database
    /// Fails if account is referenced by other tables.
    async fn delete_account(&self, account_id: i32) -> Result<(), DataError> {
        sqlx::query!("DELETE FROM accounts WHERE id=($1)", account_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get ID of given account
    async fn get_account_id(&self, account: &Account) -> Result<i32, DataError> {
        let row = sqlx::query!(
            "SELECT id FROM accounts where broker=$1 AND account_name=$2",
            &account.broker,
            &account.account_name,
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row.id)
    }

    async fn get_all_account_ids(&self) -> Result<Vec<i32>, DataError> {
        let rows = sqlx::query!("SELECT id FROM accounts")
            .fetch_all(&self.pool)
            .await?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(row.id);
        }
        Ok(ids)
    }

    async fn get_all_accounts(&self) -> Vec<Account> {
        let rows = sqlx::query!("SELECT id, broker, account_name FROM accounts")
            .fetch_all(&self.pool)
            .await;
        let mut accounts = Vec::new();
        if let Ok(rows) = rows {
            for row in rows {
                let id: i32 = row.id;
                accounts.push(Account {
                    id: Some(id),
                    broker: row.broker,
                    account_name: row.account_name,
                });
            }
        }
        accounts
    }

    /// Insert transaction to account relation
    async fn add_transaction_to_account(
        &self,
        account: i32,
        transaction: i32,
    ) -> Result<(), DataError> {
        sqlx::query!(
            "INSERT INTO account_transactions (account_id, transaction_id) VALUES ($1, $2)",
            account,
            transaction,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Insert document information for successfully parsed documents
    async fn lookup_hash(&self, hash: &str) -> Result<(Vec<i32>, String), DataError> {
        let mut trans_ids = Vec::new();
        let mut path = "".to_string();
        for row in sqlx::query!(
            "SELECT transaction_id, path FROM documents WHERE hash=$1",
            hash
        )
        .fetch_all(&self.pool)
        .await?
        {
            trans_ids.push(row.transaction_id);
            path = row.path;
        }
        Ok((trans_ids, path))
    }

    /// Insert document information for successfully parsed documents
    async fn insert_doc(
        &self,
        transaction_ids: &[i32],
        hash: &str,
        path: &str,
    ) -> Result<(), DataError> {
        for trans_id in transaction_ids {
            sqlx::query!(
                "INSERT INTO documents (transaction_id, hash, path) VALUES ($1, $2, $3)",
                *trans_id,
                hash,
                path
            )
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Get document path for given transaction
    async fn get_doc_path(&self, transaction_id: i32) -> Result<String, DataError> {
        let row = sqlx::query!(
            "SELECT path FROM documents WHERE transaction_id=$1",
            transaction_id
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row.path.to_string())
    }

    /// Get id of account a transaction belongs to
    async fn get_transactions_account_id(&self, transaction_id: i32) -> Result<i32, DataError> {
        let row = sqlx::query!(
            "SELECT account_id FROM account_transactions WHERE transaction_id = $1",
            transaction_id
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row.account_id)
    }

    /// Get transactions filtered by account id
    async fn get_all_transactions_with_account(
        &self,
        account_id: i32,
    ) -> Result<Vec<Transaction>, DataError> {
        let mut transactions = Vec::new();
        for row in sqlx::query!(
            "SELECT 
                    t.id, 
                    t.trans_type, 
                    t.asset_id, 
                    t.cash_amount, 
                    t.cash_currency_id, 
                    t.cash_date, 
                    t.related_trans, 
                    t.position, 
                    t.note,
                    c.iso_code,
                    c.rounding_digits
                FROM 
                    transactions t, 
                    account_transactions a,
                    currencies c
                WHERE 
                    a.account_id = $1 
                    AND a.transaction_id = t.id
                    AND c.id = t.cash_currency_id",
            account_id,
        )
        .fetch_all(&self.pool)
        .await?
        {
            let currency_isocode = CurrencyISOCode::new(&row.iso_code)?;
            let transaction = RawTransaction {
                id: Some(row.id),
                trans_type: row.trans_type,
                asset: row.asset_id,
                cash_amount: row.cash_amount,
                cash_currency: Currency::new(
                    Some(row.cash_currency_id),
                    currency_isocode,
                    Some(row.rounding_digits),
                ),
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
        account_id: i32,
        time: NaiveDate,
    ) -> Result<Vec<Transaction>, DataError> {
        println!(
            "Get all transactions for account id {} before time {}",
            account_id, time
        );

        let mut transactions = Vec::new();
        for row in sqlx::query!(
            "SELECT 
                    t.id, 
                    t.trans_type, 
                    t.asset_id, 
                    t.cash_amount, 
                    t.cash_currency_id, 
                    t.cash_date, 
                    t.related_trans, 
                    t.position, 
                    t.note,
                    c.iso_code,
                    c.rounding_digits
                FROM 
                    transactions t, 
                    account_transactions a,
                    currencies c
                WHERE 
                    a.account_id = $1 
                    AND a.transaction_id = t.id 
                    AND t.cash_date < $2
                    AND c.id = t.cash_currency_id",
            account_id,
            time,
        )
        .fetch_all(&self.pool)
        .await?
        {
            let currency_isocode = CurrencyISOCode::new(&row.iso_code)?;
            let transaction = RawTransaction {
                id: Some(row.id),
                trans_type: row.trans_type,
                asset: row.asset_id,
                cash_amount: row.cash_amount,
                cash_currency: Currency::new(
                    Some(row.cash_currency_id),
                    currency_isocode,
                    Some(row.rounding_digits),
                ),
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
        account_id: i32,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Transaction>, DataError> {
        let mut transactions = Vec::new();
        for row in sqlx::query!(
            "SELECT 
                    t.id, 
                    t.trans_type, 
                    t.asset_id, 
                    t.cash_amount, 
                    t.cash_currency_id, 
                    t.cash_date, 
                    t.related_trans, 
                    t.position, 
                    t.note,
                    c.iso_code,
                    c.rounding_digits 
                FROM 
                    transactions t, 
                    account_transactions a,
                    currencies c
                WHERE 
                    a.account_id = $1 
                    AND a.transaction_id = t.id 
                    AND t.cash_date BETWEEN $2 AND $3
                    AND c.id = t.cash_currency_id",
            account_id,
            start,
            end,
        )
        .fetch_all(&self.pool)
        .await?
        {
            let currency_isocode = CurrencyISOCode::new(&row.iso_code)?;
            let transaction = RawTransaction {
                id: Some(row.id),
                trans_type: row.trans_type,
                asset: row.asset_id,
                cash_amount: row.cash_amount,
                cash_currency: Currency::new(
                    Some(row.cash_currency_id),
                    currency_isocode,
                    Some(row.rounding_digits),
                ),
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
        accounts: &[i32],
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
        accounts: &[i32],
        time: NaiveDate,
    ) -> Result<Vec<Transaction>, DataError> {
        let mut transactions = Vec::new();
        for i in accounts {
            transactions.extend(
                self.get_all_transactions_with_account_before(*i, time)
                    .await?,
            );
        }
        Ok(transactions)
    }

    /// Get transactions filtered by a list of account ids and cash dates in time range
    async fn get_transactions_in_range(
        &self,
        accounts: &[i32],
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Transaction>, DataError> {
        let mut transactions = Vec::new();
        for i in accounts {
            transactions.extend(
                self.get_all_transactions_with_account_in_range(*i, start, end)
                    .await?,
            );
        }
        Ok(transactions)
    }

    /// Get transactions view by accounts
    async fn get_transaction_view_for_accounts(
        &self,
        accounts: &[i32],
    ) -> Result<Vec<TransactionView>, DataError> {
        if accounts.is_empty() {
            return Err(DataError::DataAccessFailure(
                "transaction view requires account list".to_string(),
            ));
        }
        let mut transactions = Vec::new();
        for row in sqlx::query!(
            r#"SELECT
            t.id
            ,(CASE WHEN t.related_trans IS null THEN t.id 
                ELSE t.related_trans
                END) AS group_id
            , a.id AS "asset_id?"
            , t.position
            , t.trans_type
            , t.cash_amount
            , t.cash_currency_id
            , t.cash_date
            , t.note
            , d.path AS "path?"
            , at.account_id
            , c.iso_code
            , c.rounding_digits
        FROM
            currencies c,
            transactions t
            LEFT JOIN assets a ON a.id = t.asset_id
            LEFT JOIN documents d ON d.transaction_id = t.id
            JOIN account_transactions at ON at.transaction_id = t.id
        WHERE 
            c.id = t.cash_currency_id
            AND at.account_id = ANY($1)
            ORDER BY t.cash_date DESC, group_id, t.id"#,
            accounts
        )
        .fetch_all(&self.pool)
        .await?
        {
            {
                let date: chrono::NaiveDate = row.cash_date;
                let cash_date = date.format("%Y-%m-%d").to_string();
                let currency_isocode = CurrencyISOCode::new(&row.iso_code)?;
                transactions.push(TransactionView {
                    id: row.id,
                    group_id: row.group_id,
                    asset_id: row.asset_id,
                    position: row.position,
                    trans_type: row.trans_type,
                    cash_amount: row.cash_amount,
                    cash_currency: currency_isocode.to_string(),
                    cash_date,
                    note: row.note,
                    doc_path: row.path,
                    account_id: row.account_id,
                });
            }
        }
        Ok(transactions)
    }

    /// Get transactions view for list of account ids that are related to a given asset
    async fn get_transaction_view_for_accounts_and_asset(
        &self,
        accounts: &[i32],
        asset_id: i32,
    ) -> Result<Vec<TransactionView>, DataError> {
        if accounts.is_empty() {
            return Err(DataError::DataAccessFailure(
                "transaction view requires account list".to_string(),
            ));
        }
        let mut transactions = Vec::new();
        for row in sqlx::query!(
            r#"SELECT
            t.id
            ,(CASE WHEN t.related_trans IS null THEN t.id 
                ELSE t.related_trans
                END) AS group_id
            , a.id AS "asset_id?"
            , t.position
            , t.trans_type
            , t.cash_amount
            , t.cash_currency_id
            , t.cash_date
            , t.note
            , d.path as "path?"
            , at.account_id
            , c.iso_code
            , c.rounding_digits
        FROM
            currencies c,
            transactions t
            LEFT JOIN assets a ON a.id = t.asset_id
            LEFT JOIN documents d ON d.transaction_id = t.id
            JOIN account_transactions at ON at.transaction_id = t.id
        WHERE 
            a.id = $1
            AND c.id = t.cash_currency_id
            AND at.account_id = ANY($2)
        ORDER BY t.cash_date desc, group_id, t.id"#,
            asset_id,
            accounts
        )
        .fetch_all(&self.pool)
        .await?
        {
            let date: chrono::NaiveDate = row.cash_date;
            let cash_date = date.format("%Y-%m-%d").to_string();
            let currency_isocode = CurrencyISOCode::new(&row.iso_code)?;
            transactions.push(TransactionView {
                id: row.id,
                group_id: row.group_id,
                asset_id: row.asset_id,
                position: row.position,
                trans_type: row.trans_type,
                cash_amount: row.cash_amount,
                cash_currency: currency_isocode.to_string(),
                cash_date: cash_date,
                note: row.note,
                doc_path: row.path,
                account_id: row.account_id,
            });
        }
        Ok(transactions)
    }

    /// Change the account a transaction identified by id belongs to
    async fn change_transaction_account(
        &self,
        transaction_id: i32,
        old_account_id: i32,
        new_account_id: i32,
    ) -> Result<(), DataError> {
        sqlx::query!(
            "UPDATE account_transactions SET account_id=$3 WHERE transaction_id=$1 AND account_id=$2",
            transaction_id, old_account_id , new_account_id)
            .execute(&self.pool).await?;
        Ok(())
    }
}

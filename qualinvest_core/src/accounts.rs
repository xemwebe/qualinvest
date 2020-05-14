///! Implementation of Accounts and an according PostgreSQL handler
use finql::data_handler::{DataError, TransactionHandler};
use finql::postgres_handler::{PostgresDB, RawTransaction};
use finql::transaction::Transaction;
use tokio_postgres::error::Error;
use serde::{Serialize,Deserialize};

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
pub trait AccountHandler: TransactionHandler {
    /// Clean database by dropping all tables and than run init
    fn clean_accounts(&mut self) -> Result<(), Error>;

    /// Set up new table for account management
    fn init_accounts(&mut self) -> Result<(), Error>;

    /// Insert new account info in database, if it not yet exist
    fn insert_account_if_new(&mut self, account: &Account) -> Result<usize, DataError>;

    /// Get account id for given account
    fn get_account_id(&mut self, account: &Account) -> Result<usize, DataError>;

    /// Get all accounts form db
    fn get_all_account_ids(&mut self) -> Result<Vec<usize>, DataError>;

    /// Get list of all accounts
    fn get_all_accounts(&mut self) -> Result<Vec<Account>, DataError>;

    /// Add a transaction to the account
    fn add_transaction_to_account(
        &mut self,
        account: usize,
        transaction: usize,
    ) -> Result<(), DataError>;

    /// Check if we have already parsed a given document by look-up its hash
    /// If successful, return the transaction ids and the path of the document
    fn lookup_hash(&mut self, hash: &str) -> Result<(Vec<usize>, String), DataError>;
    /// Insert document information for successfully parsed documents
    fn insert_doc(
        &mut self,
        transaction_ids: &Vec<usize>,
        hash: &str,
        path: &str,
    ) -> Result<(), DataError>;

    /// Get transactions filtered by account id
    fn get_all_transactions_with_account(
        &mut self,
        account_id: usize,
    ) -> Result<Vec<Transaction>, DataError>;

    /// Get transactions filtered by a list of account ids
    fn get_all_transactions_with_accounts(
        &mut self,
        accounts: &Vec<usize>,
    ) -> Result<Vec<Transaction>, DataError> {
        let mut transactions = Vec::new();
        for i in accounts {
            transactions.extend(self.get_all_transactions_with_account(*i)?);
        }
        Ok(transactions)
    }

    /// Get transactions view for list of account ids
    fn get_transaction_view_for_accounts(&mut self, accounts: &Vec<usize>) -> Result<Vec<TransactionView>, DataError>;

    /// Change the account a transaction identified by id belongs to
    fn change_transaction_account(&mut self, transaction_id: usize, old_account_id: usize, new_account_id: usize) -> Result<(), DataError>;
}

impl AccountHandler for PostgresDB<'_> {
    /// Clean database by dropping all tables and than run init
    fn clean_accounts(&mut self) -> Result<(), Error> {
        self.conn
            .execute("DROP TABLE IF EXISTS account_transactions", &[])?;
        self.conn.execute("DROP TABLE IF EXISTS accounts", &[])?;
        self.conn.execute("DROP TABLE IF EXISTS documents", &[])?;
        self.conn.execute("DROP TABLE IF EXISTS users", &[])?;
        self.init_accounts()?;
        Ok(())
    }

    /// Set up new table for account management
    fn init_accounts(&mut self) -> Result<(), Error> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS users (
                id SERIAL PRIMARY KEY,
                name TEXT NOT NULL,
                display TEXT NOT NULL,
                salt_hash TEXT NOT NULL,
                is_admin BOOLEAN NOT NULL DEFAULT False,
                UNIQUE (name))",
            &[],
        )?;
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS accounts (
                id SERIAL PRIMARY KEY,
                broker TEXT NOT NULL,
                account_name TEXT NOT NULL,
                UNIQUE (broker, account_name))",
            &[],
        )?;
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS account_transactions (
                id SERIAL PRIMARY KEY,
                account_id INTEGER NOT NULL,
                transaction_id INTEGER NOT NULL,
                FOREIGN KEY(account_id) REFERENCES accounts(id),
                FOREIGN KEY(transaction_id) REFERENCES transactions(id))",
            &[],
        )?;
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS documents (
                id SERIAL PRIMARY KEY,
                transaction_id INTEGER NOT NULL,
                hash TEXT NOT NULL,
                path TEXT NOT NULL,
                FOREIGN KEY(transaction_id) REFERENCES transactions(id))",
            &[],
        )?;
        Ok(())
    }

    /// Insert new account info in database
    fn insert_account_if_new(&mut self, account: &Account) -> Result<usize, DataError> {
        let id = self.get_account_id(account);
        match id {
            Ok(id) => Ok(id),
            _ => {
                let row = self
                    .conn
                    .query_one(
                        "INSERT INTO accounts (broker, account_name) VALUES ($1, $2) RETURNING id",
                        &[&account.broker, &account.account_name],
                    )
                    .map_err(|e| DataError::InsertFailed(e.to_string()))?;
                let id: i32 = row.get(0);
                Ok(id as usize)
            }
        }
    }

    /// Get ID of given account
    fn get_account_id(&mut self, account: &Account) -> Result<usize, DataError> {
        let row = self
            .conn
            .query_one(
                "SELECT id FROM accounts where broker=$1 AND account_name=$2",
                &[&account.broker, &account.account_name],
            )
            .map_err(|e| DataError::NotFound(e.to_string()))?;
        let id: i32 = row.get(0);
        Ok(id as usize)
    }

    fn get_all_account_ids(&mut self) -> Result<Vec<usize>, DataError> {
        let rows = self
            .conn
            .query(
                "SELECT id FROM accounts",
                &[],
            )
            .map_err(|e| DataError::NotFound(e.to_string()))?;
        let mut ids = Vec::new();
        for row in rows {
            let id: i32 = row.get(0);
            ids.push(id as usize);
        }
        Ok(ids)
    }

    fn get_all_accounts(&mut self) -> Result<Vec<Account>, DataError> {
        let rows = self
            .conn
            .query(
                "SELECT id, broker, account_name FROM accounts",
                &[],
            )
            .map_err(|e| DataError::NotFound(e.to_string()))?;
        let mut accounts = Vec::new();
        for row in rows {
            let id: i32 = row.get(0);
            accounts.push(Account{
                id: Some(id as usize),
                broker: row.get(1),
                account_name: row.get(2),
            });
        }
        Ok(accounts)
    }

    /// Insert transaction to account relation
    fn add_transaction_to_account(
        &mut self,
        account: usize,
        transaction: usize,
    ) -> Result<(), DataError> {
        self.conn
            .execute(
                "INSERT INTO account_transactions (account_id, transaction_id) VALUES ($1, $2)",
                &[&(account as i32), &(transaction as i32)],
            )
            .map_err(|e| DataError::InsertFailed(e.to_string()))?;
        Ok(())
    }

    /// Insert document information for successfully parsed documents
    fn lookup_hash(&mut self, hash: &str) -> Result<(Vec<usize>, String), DataError> {
        let mut trans_ids = Vec::new();
        let mut path = "".to_string();
        for row in self
            .conn
            .query(
                "SELECT transaction_id, path FROM documents WHERE hash=$1",
                &[&hash],
            )
            .map_err(|e| DataError::NotFound(e.to_string()))?
        {
            let trans: i32 = row.get(0);
            trans_ids.push(trans as usize);
            path = row.get(1);
        }
        Ok((trans_ids, path))
    }

    /// Insert document information for successfully parsed documents
    fn insert_doc(
        &mut self,
        transaction_ids: &Vec<usize>,
        hash: &str,
        path: &str,
    ) -> Result<(), DataError> {
        for trans_id in transaction_ids {
            self.conn
                .execute(
                    "INSERT INTO documents (transaction_id, hash, path) VALUES ($1, $2, $3)",
                    &[&(*trans_id as i32), &hash, &path],
                )
                .map_err(|e| DataError::InsertFailed(e.to_string()))?;
        }
        Ok(())
    }

    /// Get transactions filtered by account id
    fn get_all_transactions_with_account(
        &mut self,
        account_id: usize,
    ) -> Result<Vec<Transaction>, DataError> {
        let mut transactions = Vec::new();
        for row in self
            .conn
            .query(
                "SELECT t.id, t.trans_type, t.asset_id, 
        t.cash_amount, t.cash_currency, t.cash_date, t.related_trans, t.position, t.note 
        FROM transactions t, account_transactions a WHERE a.account_id = $1 and a.transaction_id = t.id",
                &[&(account_id as i32)],
            )
            .map_err(|e| DataError::NotFound(e.to_string()))?
        {
            let transaction = RawTransaction {
                id: row.get(0),
                trans_type: row.get(1),
                asset: row.get(2),
                cash_amount: row.get(3),
                cash_currency: row.get(4),
                cash_date: row.get(5),
                related_trans: row.get(6),
                position: row.get(7),
                note: row.get(8),
            };
            transactions.push(transaction.to_transaction()?);
        }
        Ok(transactions)
    }

    /// Get transactions view by accounts
    fn get_transaction_view_for_accounts(
        &mut self,
        accounts: &Vec<usize>,
    ) -> Result<Vec<TransactionView>, DataError> {
        if accounts.len() == 0 {
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
        for row in self.conn.query(query_string.as_str(),&[])
            .map_err(|e| DataError::NotFound(e.to_string()))?
        {
            let id: i32 = row.get(0);
            let group_id: i32 = row.get(1);
            let asset_id: Option<i32> = row.get(3);
            let asset_id = match asset_id {
                Some(id) => Some(id as usize),
                None => None,
            };
            let account_id: i32 = row.get(11);
            let date: chrono::NaiveDate = row.get(8);
            let cash_date = date.format("%Y-%m-%d").to_string();          
            transactions.push( TransactionView {
                id: id as usize,
                group_id: group_id as usize,
                asset_name: row.get(2),
                asset_id,
                position: row.get(4),
                trans_type: row.get(5),
                cash_amount: row.get(6),
                cash_currency: row.get(7),
                cash_date, 
                note: row.get(9),
                doc_path: row.get(10), 
                account_id: account_id as usize,
            });
        }
        Ok(transactions)
    }

    /// Change the account a transaction identified by id belongs to
    fn change_transaction_account(&mut self, transaction_id: usize, old_account_id: usize, new_account_id: usize) -> Result<(), DataError> {
        self.conn
        .execute(
            "UPDATE account_transactions SET account_id=$3 WHERE transaction_id=$1 AND account_id=$2",
            &[&(transaction_id as i32), &(old_account_id as i32), &(new_account_id as i32)])
            .map_err(|e| DataError::UpdateFailed(e.to_string()))?;
    Ok(())
    }

}

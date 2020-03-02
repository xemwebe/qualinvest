///! Implementation of Accounts and an according PostgreSQL handler
use finql::data_handler::{DataError, TransactionHandler};
use finql::postgres_handler::PostgresDB;
use tokio_postgres::error::Error;

pub struct Account {
    pub id: Option<usize>,
    pub broker: String,
    pub account_id: String,
}

/// Handler for asset depot accounts
pub trait AccountHandler: TransactionHandler {
    /// Clean database by dropping all tables and than run init
    fn clean_accounts(&mut self) -> Result<(), Error>;

    /// Set up new table for account management
    fn init_accounts(&mut self) -> Result<(), Error>;

    /// Insert new account info in database, if it not yet exist
    fn insert_account_if_new(&mut self, account: &Account) -> Result<usize, DataError>;

    /// Insert new account info in database
    fn get_account_id(&mut self, account: &Account) -> Result<usize, DataError>;
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
}

impl AccountHandler for PostgresDB {
    /// Clean database by dropping all tables and than run init
    fn clean_accounts(&mut self) -> Result<(), Error> {
        self.conn
            .execute("DROP TABLE IF EXISTS account_transactions", &[])?;
        self.conn.execute("DROP TABLE IF EXISTS accounts", &[])?;
        self.conn.execute("DROP TABLE IF EXISTS documents", &[])?;
        Ok(())
    }

    /// Set up new table for account management
    fn init_accounts(&mut self) -> Result<(), Error> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS accounts (
                id SERIAL PRIMARY KEY,
                broker TEXT NOT NULL,
                account_id TEXT NOT NULL,
                UNIQUE (broker, account_id))",
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
                        "INSERT INTO accounts (broker, account_id) VALUES ($1, $2) RETURNING id",
                        &[&account.broker, &account.account_id],
                    )
                    .map_err(|e| DataError::InsertFailed(e.to_string()))?;
                let id: i32 = row.get(0);
                Ok(id as usize)
            }
        }
    }

    /// Insert new account info in database
    fn get_account_id(&mut self, account: &Account) -> Result<usize, DataError> {
        let row = self
            .conn
            .query_one(
                "SELECT id FROM accounts where broker=$1 AND account_id=$2",
                &[&account.broker, &account.account_id],
            )
            .map_err(|e| DataError::InsertFailed(e.to_string()))?;
        let id: i32 = row.get(0);
        Ok(id as usize)
    }

    /// Insert new account info in database
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
            .map_err(|e| DataError::InsertFailed(e.to_string()))?
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
}

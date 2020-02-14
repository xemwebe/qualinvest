use finql::data_handler::DataError;
///! Implementation of Accounts and an according PostgreSQL handler
use postgres::{Client, NoTls};
use tokio_postgres::error::Error;

pub struct Account {
    pub id: Option<usize>,
    pub broker: String,
    pub account_id: String,
}

pub struct AccountHandler {
    conn: Client,
}

impl AccountHandler {
    pub fn connect(conn_str: &str) -> Result<AccountHandler, Error> {
        let conn = Client::connect(conn_str, NoTls)?;
        Ok(AccountHandler { conn })
    }

    /// Clean database by dropping all tables and than run init
    pub fn clean(&mut self) -> Result<(), Error> {
        self.conn
            .execute("DROP TABLE IF EXISTS account_transactions", &[])?;
        self.conn.execute("DROP TABLE IF EXISTS accounts", &[])?;
        Ok(())
    }

    /// Set up new table for account management
    pub fn init(&mut self) -> Result<(), Error> {
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
        Ok(())
    }

    /// Insert new account info in database
    pub fn insert_account_if_new(&mut self, account: &Account) -> Result<usize, DataError> {
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
    pub fn get_account_id(&mut self, account: &Account) -> Result<usize, DataError> {
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
    pub fn add_transaction_to_account(
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
}

use finql::postgres_handler::PostgresDB;
use finql::data_handler::DataError;
use crate::user::{User, UserHandler};
use crate::accounts::Account;

fn get_salt_hash(db: &mut PostgresDB, user_id: usize) -> Option<String> {
    match db.conn.query_one(
        "SELECT salt_hash FROM users WHERE id = $1",
            &[&(user_id as i32)],
        ) {
            Err(_) => None,
            Ok(row) => {
                let salt_hash: String = row.get(0);
                Some(salt_hash)
            }

        }
}

impl UserHandler for PostgresDB<'_> {
    /// Clean database by dropping all tables related to user management and run init_users
    fn clean_users(&mut self) -> Result<(), DataError> {
        self.conn.execute("DROP TABLE IF EXISTS account_rights", &[])
        .map_err(|e| DataError::DataAccessFailure(e.to_string()))?;
        self.init_users()?;
        self.conn.execute("DROP TABLE IF EXISTS users", &[])
           .map_err(|e| DataError::DataAccessFailure(e.to_string()))?;
        self.init_users()?;
        Ok(())
    }

    /// Set up new tables for user management
    fn init_users(&mut self) -> Result<(), DataError> {
        self.conn.execute(
        "CREATE TABLE IF NOT EXISTS users (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            display TEXT,
            salt_hash TEXT NOT NULL,
            is_admin BOOLEAN NOT NULL DEFAULT False,
            UNIQUE (name))",
        &[],
        )
        .map_err(|e| DataError::DataAccessFailure(e.to_string()))?;
    self.conn.execute(
        "CREATE TABLE IF NOT EXISTS account_rights (
            id SERIAL PRIMARY KEY,
            user_id INTEGER NOT NULL,
            account_id INTEGER NOT NULL,
            FOREIGN KEY(user_id) REFERENCES users(id),
            FOREIGN KEY(account_id) REFERENCES accounts(id))",
        &[],
        )
            .map_err(|e| DataError::DataAccessFailure(e.to_string()))?;
        Ok(())
    }

    /// Insert new account info in database, if it not yet exist
    fn insert_user(&mut self, user: &mut User, password: &str) -> Result<usize, DataError> {
        let row = self.conn
            .query_one(
                "INSERT INTO users (name, display, salt_hash, is_admin) VALUES ($1, $2, crypt($3,get_salt('bf',8)), $4) RETURNING id",
                &[&user.name, &user.display, &password, &user.is_admin],
            )
            .map_err(|e| DataError::InsertFailed(e.to_string()))?;
        let id: i32 = row.get(0);
        user.salt_hash = get_salt_hash(self, id as usize).ok_or(DataError::InsertFailed("reading hash of just inserted user failed".to_string()))?;
        Ok(id as usize)
    }
    
    /// Get full user information if user name and password are valid
    fn get_user_by_credentials(&mut self, name: &str, password: &str) -> Option<User> {
        match self.conn.query_one(
                "SELECT id, name, display, salt_hash, is_admin FROM users WHERE name = $1 AND 
                salt_hash = crypt($2, salt_hash)",
                &[&name, &password],
            ) {
                Err(_) => None,
                Ok(row) => {
                    let id: i32 = row.get(0);
                    Some(User{
                        id: Some(id as usize),
                        name: row.get(1),
                        display: row.get(2),
                        salt_hash: row.get(3),
                        is_admin: row.get(4),
                    })
                }

            }
    }

     /// Get full user information for given user id
    fn get_user_by_id(&mut self, user_id: usize) -> Option<User> {
        match self.conn.query_one(
                "SELECT id, name, display, salt_hash, is_admin FROM users WHERE id = $1",
                &[&(user_id as i32)],
            ) {
                Err(_) => None,
                Ok(row) => {
                    let id: i32 = row.get(0);
                    Some(User{
                        id: Some(id as usize),
                        name: row.get(1),
                        display: row.get(2),
                        salt_hash: row.get(3),
                        is_admin: row.get(4),
                    })
                }

            }
    }

    /// Get user id for given name if it exists
    fn get_user_id(&mut self, name: &str) -> Option<usize> {
        match self.conn.query_one(
            "SELECT id FROM users WHERE name = $1",
            &[&name],
        ) {
            Err(_) => None,
            Ok(row) => {
                let id: i32 = row.get(0);
                Some(id as usize)
            },
        }
    }

    /// Get user id for given name if user exists and is admin
    fn get_admin_id(&mut self, name: &str) -> Option<usize> {
        match self.conn.query_one(
            "SELECT id FROM users WHERE name = $1 AND is_admin",
                &[&name],
            ) {
                Err(_) => None,
                Ok(row) => {
                    let id: i32 = row.get(0);
                    Some(id as usize)
                }

            }
    }

    /// Get user id if user name and password are valid
    fn get_user_id_by_credentials(&mut self, name: &str, password: &str) -> Option<usize> {
        match self.conn.query_one(
            "SELECT id FROM users WHERE name = $1 AND 
                salt_hash = crypt($2, salt_hash)",
                &[&name, &password],
            ) {
                Err(_) => None,
                Ok(row) => {
                    let id: i32 = row.get(0);
                    Some(id as usize)
                }

            }
    }

    /// Update user 
    fn update_user(&mut self, user: &User) -> Result<(), DataError> {
        if user.id.is_none() {
            return Err(DataError::NotFound(
                "not yet stored to database".to_string(),
            ));
        }
        let id = user.id.unwrap();
        self.conn
            .execute(
                "UPDATE users SET name=$2, display=$3, is_admin=$4 WHERE id=$1",
                &[
                    &(id as i32),
                    &user.name,
                    &user.display,
                    &user.is_admin,
                ],
            )
            .map_err(|e| DataError::InsertFailed(e.to_string()))?;
        Ok(())
    }

    /// Update user password
    fn update_password(&mut self, user: &mut User, new_password: &str) -> Result<(), DataError> {
        if user.id.is_none() {
            return Err(DataError::NotFound(
                "not yet stored to database".to_string(),
            ));
        }
        let id = user.id.unwrap();
        self.conn
            .execute(
                "UPDATE users SET salt_hash=crypt($3, get_salt('bf',8)) WHERE id=$1 AND 
                salt_hash = crypt($2, salt_hash)",
                &[
                    &(id as i32),
                    &user.salt_hash,
                    &new_password,
                ],
            )
            .map_err(|e| DataError::InsertFailed(e.to_string()))?;
        user.salt_hash = get_salt_hash(self, id).ok_or(DataError::InsertFailed("reading hash of just inserted user failed".to_string()))?;
        Ok(())
    }

    /// Remove all user information form data base
    fn delete_user(&mut self, user_id: usize) -> Result<(), DataError> {
        self.conn
            .execute("DELETE FROM users WHERE id=$1;", &[&(user_id as i32)])
            .map_err(|e| DataError::DataAccessFailure(e.to_string()))?;
        Ok(())
    }

    /// Give user identified by user_id the right to access account given by account_id
    fn add_account_right(&mut self, user_id: usize, account_id: usize) -> Result<usize, DataError> {
        let rows = self.conn
            .query("SELECT id FROM account_rights WHERE user_id=$1 AND account_id=$2"
                , &[&(user_id as i32), &(account_id as i32)])
                .map_err(|e| DataError::DataAccessFailure(e.to_string()))?;
        if rows.len() > 0 {
            let id: i32 = rows[0].get(0);
            Ok(id as usize)
        } else {
            let row = self.conn.query_one(
                "INSERT INTO account_rights (user_id, account_id) VALUES ($1, $2) RETURNING id",
                &[&(user_id as i32), &(account_id as i32)])
                .map_err(|e| DataError::InsertFailed(e.to_string()))?;
            let id: i32 = row.get(0);
            Ok(id as usize)
        }
        
    }

    /// Remove right to access account given by account_id from user with id user_id
    fn remove_account_right(&mut self, user_id: usize, account_id: usize) -> Result<(), DataError> {
        self.conn
            .execute("DELETE FROM account_rights WHERE user_id=$1 AND account_id=$2", 
                &[&(user_id as i32), &(account_id as i32)])
                .map_err(|e| DataError::DataAccessFailure(e.to_string()))?;
        Ok(())
    }


    /// Get list of account ids a user as access to
    fn get_user_accounts(&mut self, user_id: usize) -> Result<Vec<Account>, DataError> {
        let rows= self.conn
            .query("SELECT a.id, a.broker, a.account_name FROM accounts a, account_rights r WHERE r.account_id = a.id AND r.user_id=$1", 
                &[&(user_id as i32)])
                .map_err(|e| DataError::DataAccessFailure(e.to_string()))?;
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
}

use crate::accounts::{Account,AccountHandler};
use finql::data_handler::DataError;

/// User information as stored in database
#[derive(Debug)]
pub struct User {
    pub id: Option<usize>,
    pub name: String,
    pub display: Option<String>,
    pub salt_hash: String,
    pub is_admin: bool,
}

pub trait UserHandler: AccountHandler {
    /// Clean database by dropping all tables related to user management and run init_users
    fn clean_users(&mut self) -> Result<(), DataError>;

    /// Set up new tables for user management
    fn init_users(&mut self) -> Result<(), DataError>;

    /// Insert new account info in database, if it not yet exist
    fn insert_user(&mut self, user: &mut User, password: &str) -> Result<usize, DataError>;

    /// Get full user information if user name and password are valid
    fn get_user_by_credentials(&mut self, name: &str, password: &str) -> Option<User>;
    /// Get full user information for given user id
    fn get_user_by_id(&mut self, user_id: usize) -> Option<User>;
    /// Get user id for given name if it exists
    fn get_user_id(&mut self, name: &str) -> Option<usize>;
    /// Get user id for given name if user exists and is admin
    fn get_admin_id(&mut self, name: &str) -> Option<usize>;
    /// Get user id if user name and password are valid
    fn get_user_id_by_credentials(&mut self, name: &str, password: &str) -> Option<usize>; 

    /// Update user, but let password unchanged
    fn update_user(&mut self, user: &User) -> Result<(), DataError>;

    /// Update user password 
    fn update_password(&mut self, user: &mut User, password: &str) -> Result<(), DataError>;

    /// Remove all user information form data base
    fn delete_user(&mut self, user_id: usize) -> Result<(), DataError>;

    /// Give user identified by user_id the right to access account given by account_id
    fn add_account_right(&mut self, user_id: usize, account_id: usize) -> Result<usize, DataError>;

    /// Remove right to access account given by account_id from user with id user_id
    fn remove_account_right(&mut self, user_id: usize, account_id: usize) -> Result<(), DataError>;
    
    /// Get list of account ids a user given by user_id as access to
    fn get_user_accounts(&mut self, user_id: usize) -> Result<Vec<Account>, DataError>;

    /// Remove all ids form ids the user has no access to
    fn valid_accounts(&mut self, user_id: usize, ids: &Vec<usize>) -> Result<Vec<usize>, DataError> {
        let user_accounts = self.get_user_accounts(user_id)?;
        let mut valid_ids = Vec::new();
        for id in ids {
            for account in &user_accounts {
                if account.id.unwrap() == *id {
                    valid_ids.push(*id);
                    continue;
                }
            };
        }
        Ok(valid_ids)
    }
}


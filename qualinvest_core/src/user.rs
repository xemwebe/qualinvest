use async_trait::async_trait;
use std::default::Default;
use serde::{Serialize,Deserialize};
use crate::accounts::{Account,AccountHandler};
use finql_data::DataError;
use chrono::NaiveDate;

/// User information as stored in database
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct User {
    pub id: Option<usize>,
    pub name: String,
    pub display: Option<String>,
    pub salt_hash: String,
    pub is_admin: bool,
}

/// User settings stored in database
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct UserSettings {
    pub period_start: Option<NaiveDate>,
    pub period_end: Option<NaiveDate>,
    // User accounts selected to be used for portfolio analysis
    pub account_ids: Vec<usize>,
}

#[async_trait]
pub trait UserHandler: AccountHandler {
    /// Clean database by dropping all tables related to user management and run init_users
    async fn clean_users(&self) -> Result<(), DataError>;

    /// Set up new tables for user management
    async fn init_users(&self) -> Result<(), DataError>;

    /// Insert new account info in database, if it not yet exist
    async fn insert_user(&self, user: &mut User, password: &str) -> Result<usize, DataError>;

    /// Get full user information if user name and password are valid
    async fn get_user_by_credentials(& self, name: &str, password: &str) -> Option<User>;
    /// Get full user information for given user id
    async fn get_user_by_id(&self, user_id: usize) -> Option<User>;
    /// Get user id for given name if it exists
    async fn get_user_id(&self, name: &str) -> Option<usize>;
    /// Get user id for given name if user exists and is admin
    async fn get_admin_id(&self, name: &str) -> Option<usize>;
    /// Get user id if user name and password are valid
    async fn get_user_id_by_credentials(&self, name: &str, password: &str) -> Option<usize>; 
    /// Get list of all users
    async fn get_all_users(&self) -> Vec<User>; 

    /// Update user, but let password unchanged
    async fn update_user(&self, user: &User) -> Result<(), DataError>;

    /// Update user password 
    async fn update_password(&self, user: &mut User, password: &str) -> Result<(), DataError>;

    /// Remove all user information form data base
    async fn delete_user(&self, user_id: usize) -> Result<(), DataError>;

    /// Give user identified by user_id the right to access account given by account_id
    async fn add_account_right(&self, user_id: usize, account_id: usize) -> Result<usize, DataError>;

    /// Remove right to access account given by account_id from user with id user_id
    async fn remove_account_right(&self, user_id: usize, account_id: usize) -> Result<(), DataError>;
    
    /// Get list of account ids a user given by user_id as access to
    async fn get_user_accounts(&self, user_id: usize) -> Result<Vec<Account>, DataError>;

    /// Remove all ids form ids the user has no access to
    async fn valid_accounts(&self, user_id: usize, ids: &[usize]) -> Result<Vec<usize>, DataError>;
   
    /// Get the account the transaction given by id belongs to, 
    /// if the user given by user_id as the right to access this account
    async fn get_transaction_account_if_valid(&self, trans_id: usize, user_id: usize) -> Result<Account, DataError>;


    /// Remove this transaction and all its dependencies, if it belongs to an account the user has
    /// access rights for.
    async fn remove_transaction(&self, trans_id: usize, user_id: usize)-> Result<(), DataError>;

    /// Get user settings
    async fn get_user_settings(&self, user_id: usize) -> UserSettings;

    /// Set user settings
    async fn set_user_settings(&self, user_id: usize, settings: &UserSettings) -> Result<(), DataError>;
}


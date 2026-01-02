use crate::accounts::{Account, AccountHandler};
use async_trait::async_trait;
use finql::datatypes::DataError;
use finql::period_date::PeriodDate;
use serde::{Deserialize, Serialize};
use std::default::Default;

/// User settings stored in database
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct UserSettings {
    pub period_start: PeriodDate,
    pub period_end: PeriodDate,
    // User accounts selected to be used for portfolio analysis
    pub account_ids: Vec<i32>,
}

/// User information as stored in database
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct User {
    pub id: Option<i32>,
    pub name: String,
    pub display: Option<String>,
    pub is_admin: bool,
}

#[async_trait]
pub trait UserHandler: AccountHandler {
    /// Clean database by dropping all tables related to user management and run init_users
    async fn clean_users(&self) -> Result<(), DataError>;

    /// Set up new tables for user management
    async fn init_users(&self) -> Result<(), DataError>;

    /// Insert new account info in database, if it not yet exist
    async fn insert_user(&self, user: &User, password: &str) -> Result<i32, DataError>;

    /// Get full user information if user name and password are valid
    async fn get_user_by_credentials(&self, name: &str, password: &str) -> Option<User>;
    /// Get full user information for given user id
    async fn get_user_by_id(&self, user_id: i32) -> Option<User>;
    /// Get user id for given name if it exists
    async fn get_user_id(&self, name: &str) -> Option<i32>;
    /// Get user id for given name if user exists and is admin
    async fn get_admin_id(&self, name: &str) -> Option<i32>;
    /// Get user id if user name and password are valid
    async fn get_user_id_by_credentials(&self, name: &str, password: &str) -> Option<i32>;
    /// Get list of all users
    async fn get_all_users(&self) -> Vec<User>;

    /// Update user, but let password unchanged
    async fn update_user(&self, user: &User) -> Result<(), DataError>;

    /// Update user password
    async fn update_password(&self, user_id: i32, password: &str) -> Result<(), DataError>;

    /// Remove all user information form data base
    async fn delete_user(&self, user_id: i32) -> Result<(), DataError>;

    /// Give user identified by user_id the right to access account given by account_id
    async fn add_account_right(&self, user_id: i32, account_id: i32) -> Result<i32, DataError>;

    /// Remove right to access account given by account_id from user with id user_id
    async fn remove_account_right(&self, user_id: i32, account_id: i32) -> Result<(), DataError>;

    /// Get list of account ids a user given by user_id has access to
    async fn get_user_accounts(&self, user_id: i32) -> Result<Vec<Account>, DataError>;

    /// Remove all account ids form ids the user has no access to
    async fn valid_accounts(&self, user_id: i32, ids: &[i32]) -> Result<Vec<i32>, DataError>;

    /// Get the account the transaction given by id belongs to,
    /// if the user given by user_id as the right to access this account
    async fn get_transaction_account_if_valid(
        &self,
        trans_id: i32,
        user_id: i32,
    ) -> Result<Account, DataError>;

    /// Remove this transaction and all its dependencies, if it belongs to an account the user has
    /// access rights for.
    async fn remove_transaction(&self, trans_id: i32, user_id: i32) -> Result<(), DataError>;

    /// Get user settings
    async fn get_user_settings(&self, user_id: i32) -> UserSettings;

    /// Set user settings
    async fn set_user_settings(
        &self,
        user_id: i32,
        settings: &UserSettings,
    ) -> Result<(), DataError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use time::macros::date;

    #[test]
    fn test_user_settings_serialize_to_value() {
        let settings = UserSettings {
            period_start: PeriodDate::FixedDate(date!(2025 - 02 - 04)),
            period_end: PeriodDate::Today,
            account_ids: vec![1],
        };
        let serialized_settings = serde_json::to_value(&settings).unwrap();
        assert_eq!(
            serialized_settings,
            json!(
                {"period_start":{"FixedDate":[2025,35]},"period_end":"Today","account_ids":[1]}
            )
        )
    }

    #[test]
    fn test_user_settings_serialize() {
        let settings = UserSettings {
            period_start: PeriodDate::FixedDate(date!(2025 - 02 - 04)),
            period_end: PeriodDate::Today,
            account_ids: vec![1],
        };
        let serialized_settings = serde_json::to_string(&settings).unwrap();
        assert_eq!(
            serialized_settings,
            r#"{"period_start":{"FixedDate":[2025,35]},"period_end":"Today","account_ids":[1]}"#
        );
    }

    #[test]
    fn test_user_settings_deserialize() {
        let settings = r#"{"period_end": "Today", "account_ids": [1], "period_start": {"FixedDate": [2025,35]}}"#;
        let deserialized_settings: UserSettings = serde_json::from_str(&settings).unwrap();
        assert_eq!(
            format!("{:?}", deserialized_settings.period_end),
            format!("{:?}", PeriodDate::Today)
        );
        assert_eq!(deserialized_settings.account_ids, vec![1]);
        assert_eq!(
            format!("{:?}", deserialized_settings.period_start),
            format!("{:?}", PeriodDate::FixedDate(date!(2025 - 02 - 04)))
        );
    }

    #[test]
    fn test_user_settings_deserialize_value() {
        let settings = json!({"period_end": "Today", "account_ids": [1], "period_start": {"FixedDate": [2025,35]}});
        let deserialized_settings: UserSettings = serde_json::from_value(settings).unwrap();
        assert_eq!(
            format!("{:?}", deserialized_settings.period_end),
            format!("{:?}", PeriodDate::Today)
        );
        assert_eq!(deserialized_settings.account_ids, vec![1]);
        assert_eq!(
            format!("{:?}", deserialized_settings.period_start),
            format!("{:?}", PeriodDate::FixedDate(date!(2025 - 02 - 04)))
        );
    }
}

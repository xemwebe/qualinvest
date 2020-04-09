use serde::Deserialize;

pub mod read_pdf;
pub mod accounts;


/// Configuration parameters
#[derive(Debug, Deserialize)]
pub struct Config {
    pub db_host: String,
    pub db_name: String,
    pub db_user: String,
    pub db_password: String,
    pub debug: bool,
    pub doc_path: String,
    pub warn_old: bool,
    pub consistency_check: bool,
    pub rename_asset: bool,
    pub default_account: bool,
}


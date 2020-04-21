use serde::Deserialize;

pub mod accounts;
pub mod position;
pub mod read_pdf;

/// Configuration parameters
#[derive(Debug, Deserialize)]
pub struct Config {
    pub db: DbParams,
    pub pdf: PdfParseParams,
    pub debug: bool,

}

/// Database parameters
#[derive(Debug, Deserialize)]
pub struct DbParams {
    pub host: String,
    pub name: String,
    pub user: String,
    pub password: String,
}

/// Parameters for PDF file parsing
#[derive(Debug, Deserialize)]
pub struct PdfParseParams {
    pub doc_path: String,
    pub warn_old: bool,
    pub consistency_check: bool,
    pub rename_asset: bool,
    pub default_account: bool,
}

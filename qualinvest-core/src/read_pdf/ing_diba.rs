//! Parse text files and extract asset and transaction data
use super::{ParsedInfo, ReadPDFError};
use finql::Market;

type Result<T> = std::result::Result<T, ReadPDFError>;

/// Extract transaction information from text files
pub async fn parse_transactions(_text: &str, _market: &Market) -> Result<ParsedInfo> {
    Err(ReadPDFError::NotFound("ING-DiBa parser"))
}

//! Parse text files and extract asset and transaction data
use super::{ParsedTransactionInfo, ReadPDFError};
use finql::Market;

/// Extract transaction information from text files
pub async fn parse_transactions(
    _text: &str,
    _market: &Market,
) -> Result<ParsedTransactionInfo, ReadPDFError> {
    Err(ReadPDFError::NotFound("Sparkasse parser"))
}

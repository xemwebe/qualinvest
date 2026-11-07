use super::{Bank, ParsedDocument, ParsedInfo, ReadPDFError, Result};
use finql::datatypes::{Asset, Transaction};

#[derive(Debug)]
struct Finanzreport {
    account_info: Option<(String, String)>,
}

impl ParsedDocument for Finanzreport {
    fn add_account_info(&mut self, bank: Bank, account: &str) {
        self.account_info = Some((format!("{}", bank), account.to_string()));
    }

    fn get_account_info(&self) -> Option<(String, String)> {
        return self.account_info.clone();
    }

    fn check_consistency(&self) -> Result<()> {
        Ok(())
    }

    fn make_transactions(&self) -> Result<(Vec<Transaction>, Option<Asset>)> {
        Err(ReadPDFError::NotFound("comdirect Finanzreport parser"))
    }
}

pub fn parse_report(_text: &str) -> Result<ParsedInfo> {
    Err(ReadPDFError::NotFound("comdirect Finanzreport parser"))
}

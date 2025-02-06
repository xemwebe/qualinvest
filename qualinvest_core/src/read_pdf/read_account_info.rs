//! Parse broker or bank and account information form pdf file
use super::ReadPDFError;
use lazy_static::lazy_static;
use regex::Regex;

fn trim_account(account: &str) -> String {
    account.trim().replace(' ', "")
}

pub fn parse_account_info(text: &str) -> Result<(String, String), ReadPDFError> {
    lazy_static! {
        static ref BROKER: Regex = Regex::new(r"(comdirect|Baader Bank)").unwrap();
        static ref ACCOUNT: Regex = Regex::new(r"Depotnummer\s*:\s*([0-9]*\s[0-9]*)").unwrap();
        static ref ACCOUNT_ABBREV: Regex = Regex::new(r"Depotnr.:\s*([0-9]*\s[0-9]*)").unwrap();
    }
    let broker = BROKER.captures(text);
    match broker {
        None => Err(ReadPDFError::NotFound("broker")),
        Some(broker) => match &broker[1] {
            "comdirect" => {
                let account_id = ACCOUNT.captures(text);
                match account_id {
                    None => {
                        let account_id = ACCOUNT_ABBREV.captures(text);
                        match account_id {
                            None => Err(ReadPDFError::NotFound("comdirect account")),
                            Some(account_id) => {
                                Ok(("comdirect".to_string(), trim_account(&account_id[1])))
                            }
                        }
                    }
                    Some(account_id) => Ok(("comdirect".to_string(), trim_account(&account_id[1]))),
                }
            }
            _ => Err(ReadPDFError::NotFound("broker")),
        },
    }
}

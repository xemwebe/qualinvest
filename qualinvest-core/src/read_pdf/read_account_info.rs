//! Parse broker or bank and account information form pdf file
use super::ReadPDFError;
use regex::Regex;
use std::{fmt::Display, sync::LazyLock};

fn trim_account(account: &str) -> String {
    account.trim().replace(' ', "")
}

pub enum Bank {
    Comdirect,
    IngDiba,
    SparkasseFrankfurt,
}

impl Display for Bank {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Bank::Comdirect => write!(f, "comdirect"),
            Bank::IngDiba => write!(f, "ING-DiBa"),
            Bank::SparkasseFrankfurt => write!(f, "Sparkasse Frankfurt"),
        }
    }
}

pub fn parse_account_info(text: &str) -> Result<(Bank, String), ReadPDFError> {
    static BROKER: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(comdirect|Frankfurter Sparkasse|ING-DiBa)").unwrap());
    static ACCOUNT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"Depotnummer:\s*([0-9]*\s*[0-9]*)").unwrap());
    static ACCOUNT2: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"Konto-Nr.\s*([0-9]*\s*[0-9]*)").unwrap());
    static ACCOUNT_ABBREV: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"Depotnr.:\s*([0-9]*\s*[0-9]*)").unwrap());
    static GIRO_ACCOUNT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?:Girokonto|IBAN)\s+([A-Z]{2}[0-9]{2} [0-9]{4} [0-9]{4} [0-9]{4} [0-9]{4} [0-9]{2})",
        )
        .unwrap()
    });

    let broker = BROKER.captures(text);
    match broker {
        None => Err(ReadPDFError::NotFound("broker")),
        Some(broker) => match &broker[1] {
            "comdirect" => {
                let mut account_id = None;
                for acc_re in [&ACCOUNT, &ACCOUNT_ABBREV, &GIRO_ACCOUNT] {
                    account_id = acc_re.captures(text);
                    if account_id.is_some() {
                        break;
                    }
                }
                if let Some(account_id) = account_id {
                    Ok((Bank::Comdirect, trim_account(&account_id[1])))
                } else {
                    Err(ReadPDFError::NotFound("comdirect account"))
                }
            }
            "Frankfurter Sparkasse" => {
                let account_id = ACCOUNT2.captures(text);
                if let Some(account_id) = account_id {
                    Ok((Bank::SparkasseFrankfurt, trim_account(&account_id[1])))
                } else {
                    Err(ReadPDFError::NotFound("Frankfurter Sparkasse account"))
                }
            }
            "ING-DiBa" => {
                let account_id = GIRO_ACCOUNT.captures(text);
                if let Some(account_id) = account_id {
                    Ok((Bank::IngDiba, trim_account(&account_id[1])))
                } else {
                    Err(ReadPDFError::NotFound("ING-DiBa account"))
                }
            }
            _ => Err(ReadPDFError::NotFound("broker")),
        },
    }
}

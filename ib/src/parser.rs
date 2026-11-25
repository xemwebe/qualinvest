use crate::error::{Error, Result};

pub fn parse_asset_ids(beschreibung: &str) -> Result<(String, String)> {
    let re = regex::Regex::new(r"^([A-Za-z0-9.]*) *\(([A-Z0-9]*)\)").unwrap();
    if let Some(caps) = re.captures(beschreibung) {
        Ok((caps[1].to_string(), caps[2].to_string()))
    } else {
        eprintln!("Kann Symbole nicht aus Beschreibung '{beschreibung}' extrahieren");
        Err(Error::FailedToParseSymbolsFromDescription)
    }
}

pub fn parse_jurisdiction(beschreibung: &str) -> Result<String> {
    let re_german = regex::Regex::new(r"- (.{2}) Steuer$").unwrap();
    if let Some(caps) = re_german.captures(beschreibung) {
        return Ok(caps[1].to_string());
    }
    let re_english = regex::Regex::new(r"- (.{2}) TAX$").unwrap();
    if let Some(caps) = re_english.captures(beschreibung) {
        return Ok(caps[1].to_string());
    }
    Err(Error::FailedToParseJurisdiction(beschreibung.to_string()))
}

pub fn parse_symbols_von_kapitalmaßnahme(beschreibung: &str) -> Result<(String, String)> {
    let re = regex::Regex::new(r"^([A-Za-z0-9. ]*)\(.*\(([A-Za-z0-9. ]*), [^)]*\)$").unwrap();
    if let Some(caps) = re.captures(beschreibung) {
        return Ok((caps[1].to_string(), caps[2].to_string()));
    }
    Err(Error::FailedToParseKapitalmaßnahme(
        beschreibung.to_string(),
    ))
}

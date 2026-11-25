use super::{Groups, c, fv};
use crate::error::Error;

const TABLE: &str = "Informationen zum Finanzinstrument";

type Result<T> = std::result::Result<T, Error>;

/// `Informationen zum Finanzinstrument` – instrument master data.
#[derive(Debug)]
#[allow(dead_code)]
pub struct FinanzinstrumentRow {
    pub vermoegenswert_kategorie: String,
    pub symbol: String,
    pub beschreibung: String,
    pub conid: String,
    pub wertpapier_id: String,
    pub basiswert: String,
    pub boerse: String,
    pub multiplikator: f64,
    pub typ: String,
    pub code: String,
}

/// FinazinstrumentInfos
#[derive(Debug)]
pub struct FinanzInstrumentInfos {
    pub infos: Vec<FinanzinstrumentRow>,
}

impl FinanzInstrumentInfos {
    pub(super) fn parse_from_groups(groups: &Groups) -> Result<FinanzInstrumentInfos> {
        let header_indizes = groups.get_header_indizes(TABLE)?;
        // ── Informationen zum Finanzinstrument ────────────────────────────────────
        let finanzinstrumente: Vec<FinanzinstrumentRow> = groups
            .data_rows(TABLE)
            .map(|f| {
                let vermoegenswert_kategorie =
                    if let Some(idx) = header_indizes.get("Vermögenswertkategorie") {
                        c(f, *idx).to_string()
                    } else {
                        String::new()
                    };
                let basiswert = if let Some(idx) = header_indizes.get("Basiswert") {
                    c(f, *idx).to_string()
                } else {
                    String::new()
                };
                let symbol = if let Some(idx) = header_indizes.get("Symbol") {
                    c(f, *idx).to_string()
                } else {
                    String::new()
                };
                let beschreibung = if let Some(idx) = header_indizes.get("Beschreibung") {
                    c(f, *idx).to_string()
                } else {
                    String::new()
                };
                let conid = if let Some(idx) = header_indizes.get("Conid") {
                    c(f, *idx).to_string()
                } else {
                    String::new()
                };
                let wertpapier_id = if let Some(idx) = header_indizes.get("Wertpapier-ID") {
                    c(f, *idx).to_string()
                } else {
                    String::new()
                };
                let multiplikator = if let Some(idx) = header_indizes.get("Multiplikator") {
                    fv(c(f, *idx))
                } else {
                    1.0
                };
                let boerse = if let Some(idx) = header_indizes.get("Börse") {
                    c(f, *idx).to_string()
                } else {
                    String::new()
                };
                let typ = if let Some(idx) = header_indizes.get("Typ") {
                    c(f, *idx).to_string()
                } else {
                    String::new()
                };
                let code = if let Some(idx) = header_indizes.get("Code") {
                    c(f, *idx).to_string()
                } else {
                    String::new()
                };
                FinanzinstrumentRow {
                    vermoegenswert_kategorie,
                    symbol,
                    beschreibung,
                    conid,
                    wertpapier_id,
                    basiswert,
                    boerse,
                    multiplikator,
                    typ,
                    code,
                }
            })
            .collect();
        Ok(Self {
            infos: finanzinstrumente,
        })
    }

    fn check_symbol(info_symbol: &str, symbol: &str) -> bool {
        let parts = info_symbol.split(", ");
        for p in parts {
            if p == symbol {
                return true;
            }
        }
        false
    }

    pub fn is_etf(&self, symbol: &str, isin: Option<&str>) -> Result<bool> {
        log::trace!("is_etf gestartet für symbol '{symbol}'");
        for info in &self.infos {
            if Self::check_symbol(&info.symbol, symbol) {
                return Ok(info.typ == "ETF");
            }
            if let Some(isin) = isin
                && (info.wertpapier_id == isin || info.conid == isin)
            {
                return Ok(info.typ == "ETF");
            }
        }
        Err(Error::SymbolNotFound(symbol.to_string()))
    }

    pub fn get_isin_and_name_by_symbol(&self, symbol: &str) -> Result<(String, String)> {
        for info in &self.infos {
            if Self::check_symbol(&info.symbol, symbol) {
                return Ok((info.wertpapier_id.clone(), info.beschreibung.clone()));
            }
        }
        Err(Error::SymbolNotFound(symbol.to_string()))
    }
}

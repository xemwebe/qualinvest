use std::{collections::HashMap, path::Path};

use crate::date::convert_date;
use crate::error::Error;
use crate::fx::FxRates;
use crate::parser::{parse_asset_ids, parse_jurisdiction};
use crate::quellensteuer::{Quellensteuer, QuellensteuerPerJurisdiktion};
use crate::read::instrument_information::FinanzInstrumentInfos;
use crate::vorabpauschale::{EtfPosition, EtfPositionen, FondsTyp};

mod instrument_information;

type Result<T> = std::result::Result<T, Error>;

// ============================================================
// Numeric / field helpers
// ============================================================

/// Parse an optional float from a CSV cell.
/// Returns `None` for empty strings or the sentinel "--".
/// Handles thousands-separator commas (e.g. `"542,625.03"`) and trailing `%`.
fn opt_f64(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() || s == "--" {
        return None;
    }
    let cleaned: String = s.chars().filter(|&c| c != ',').collect();
    let cleaned = cleaned.trim_end_matches('%');
    cleaned.parse().ok()
}

fn fv(s: &str) -> f64 {
    opt_f64(s).unwrap_or(0.0)
}

fn iv(s: &str) -> i64 {
    let cleaned: String = s.trim().chars().filter(|&c| c != ',').collect();
    cleaned.parse().unwrap_or(0)
}

/// Get field `i` from a row slice, returning `""` if out of bounds.
fn c(row: &[String], i: usize) -> &str {
    row.get(i).map(String::as_str).unwrap_or("")
}

// ============================================================
// Table structs
// ============================================================

/// `Statement` – broker metadata (key → value pairs).
#[derive(Debug)]
#[allow(dead_code)]
pub struct StatementRow {
    pub feldname: String,
    pub feldwert: String,
}

/// `Kontoinformation` – account information (key → value pairs).
#[derive(Debug)]
#[allow(dead_code)]
pub struct KontoinformationRow {
    pub feldname: String,
    pub feldwert: String,
}

/// `Nettovermögenswert` – net asset value per asset class.
#[derive(Debug)]
#[allow(dead_code)]
pub struct NettovermoegensRow {
    pub assetklasse: String,
    pub vorheriger_gesamtwert: f64,
    pub aktuell_long: f64,
    pub aktuell_short: f64,
    pub aktueller_gesamtwert: f64,
    pub veraenderung: f64,
}

/// `Nettovermögenswert` (sub-table) – time-weighted return.
#[derive(Debug)]
#[allow(dead_code)]
pub struct ZeitgewichteteRenditeRow {
    /// Return expressed as a percentage value (e.g. `15.813252668`).
    pub rendite_pct: f64,
}

/// `Veränderung des NAV` – NAV change summary (key → numeric value).
#[derive(Debug)]
#[allow(dead_code)]
pub struct VeraenderungNavRow {
    pub feldname: String,
    pub feldwert: f64,
}

/// `Mark-to-Market-Performance-Überblick` – MTM P&L per position.
#[derive(Debug)]
#[allow(dead_code)]
pub struct MtmPerformanceRow {
    pub vermoegenswert_kategorie: String,
    pub symbol: String,
    /// `None` when the position was newly opened (displayed as "--" in the CSV).
    pub vorher_menge: Option<f64>,
    /// `None` when the position was fully closed.
    pub aktuell_menge: Option<f64>,
    /// `None` for new positions.
    pub vorher_kurs: Option<f64>,
    /// `None` for closed positions.
    pub aktuell_kurs: Option<f64>,
    pub mtm_pl_position: f64,
    pub mtm_pl_transaktion: f64,
    pub mtm_pl_provisionen: f64,
    pub mtm_pl_sonstige: f64,
    pub mtm_pl_gesamt: f64,
    pub code: String,
}

/// `Übersicht  zur realisierten und unrealisierten Performance`.
/// (Note: the table name in the CSV contains two spaces after "Übersicht".)
#[derive(Debug)]
#[allow(dead_code)]
pub struct PerformanceUebersichtRow {
    pub vermoegenswert_kategorie: String,
    pub symbol: String,
    pub kostenanpassung: f64,
    pub realisiert_st_gewinn: f64,
    pub realisiert_st_verlust: f64,
    pub realisiert_lt_gewinn: f64,
    pub realisiert_lt_verlust: f64,
    pub realisiert_gesamt: f64,
    pub unrealisiert_st_gewinn: f64,
    pub unrealisiert_st_verlust: f64,
    pub unrealisiert_lt_gewinn: f64,
    pub unrealisiert_lt_verlust: f64,
    pub unrealisiert_gesamt: f64,
    pub gesamt: f64,
    pub code: String,
}

/// `Cash-Bericht` – cash movements per currency.
#[derive(Debug)]
#[allow(dead_code)]
pub struct CashBerichtRow {
    pub waehrungsuebersicht: String,
    pub waehrung: String,
    pub gesamt: f64,
    pub wertpapiere: f64,
    pub futures: f64,
}

/// `Offene Positionen` – individual open positions.
#[derive(Debug)]
#[allow(dead_code)]
pub struct OffenePositionRow {
    pub data_discriminator: String,
    pub vermoegenswert_kategorie: String,
    pub waehrung: String,
    pub symbol: String,
    pub menge: Option<f64>,
    pub multiplikator: f64,
    pub einstands_kurs: f64,
    pub kostenbasis: f64,
    pub schlusskurs: Option<f64>,
    pub wert: f64,
    pub unrealisierter_gv: f64,
    pub code: String,
}

/// `Devisenpositionen` – foreign-currency cash positions.
#[derive(Debug)]
#[allow(dead_code)]
pub struct DevisenpositionRow {
    pub vermoegenswert_kategorie: String,
    pub waehrung: String,
    pub beschreibung: String,
    pub menge: f64,
    pub einstands_kurs: f64,
    pub kostenbasis_eur: f64,
    pub schlusskurs: f64,
    pub wert_eur: f64,
    pub unrealisierter_gv_eur: f64,
    pub code: String,
}

/// `Netto-Aktienpositionsübersicht` – net stock positions.
#[derive(Debug)]
#[allow(dead_code)]
pub struct NettoAktienpositionRow {
    pub vermoegenswert_kategorie: String,
    pub waehrung: String,
    pub symbol: String,
    pub beschreibung: String,
    pub aktien_bei_ib: i64,
    pub aktien_geliehen: i64,
    pub aktien_verliehen: i64,
    pub netto_aktien: i64,
}

/// `Transaktionen` – security trades (stocks, ETFs, …).
/// Uses the first of the two `Transaktionen` headers in the CSV.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TransaktionRow {
    pub data_discriminator: String,
    pub vermoegenswert_kategorie: String,
    pub waehrung: String,
    pub symbol: String,
    pub datum_zeit: String,
    pub menge: f64,
    pub transaktions_kurs: f64,
    pub schlusskurs: f64,
    pub erloese: f64,
    pub prov_gebuehr: f64,
    pub basis: f64,
    pub realisierter_gv: f64,
    pub mtm_gv: f64,
    pub code: String,
}

/// `Transaktionen` – foreign-exchange trades.
/// Uses the second `Transaktionen` header (column 7 is empty, column 12 is MTM in EUR).
#[derive(Debug)]
#[allow(dead_code)]
pub struct DevisenTransaktionRow {
    pub data_discriminator: String,
    pub vermoegenswert_kategorie: String,
    pub waehrung: String,
    pub symbol: String,
    pub datum_zeit: String,
    pub menge: f64,
    pub transaktions_kurs: f64,
    pub erloese: f64,
    pub provisionen_eur: f64,
    pub mtm_eur: f64,
    pub code: String,
}

/// `Transaktionsgebühren` – transaction fees (e.g. PTM levy).
#[derive(Debug)]
#[allow(dead_code)]
pub struct TransaktionsgebuehrRow {
    pub vermoegenswert_kategorie: String,
    pub waehrung: String,
    pub datum_zeit: String,
    pub symbol: String,
    pub beschreibung: String,
    pub menge: f64,
    pub handelskurs: f64,
    pub betrag: f64,
}

/// `Transfers` - Übertrag von assets
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TransferRow {
    pub vermoegenswert_kategorie: String,
    pub waehrung: String,
    pub symbol: String,
    pub datum: String,
    pub typ: String,
    pub richtung: String,
    pub überweisendes_unternehmen: String,
    pub transfer_konto: String,
    pub menge: f64,
    pub transferpreis: String,
    pub marktwert: f64,
    pub realisierter_guv: f64,
    pub barbetrag: f64,
    pub code: String,
}

/// `Kapitalmaßnahmen` – corporate actions.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct KapitalmassnahmeRow {
    pub vermoegenswert_kategorie: String,
    pub waehrung: String,
    pub berichtsdatum: String,
    pub datum_zeit: String,
    pub beschreibung: String,
    pub menge: f64,
    pub erloese: f64,
    pub wert: f64,
    pub realisierter_gv: f64,
    pub code: String,
}

/// `Einzahlungen & Auszahlungen` – deposits and withdrawals.
#[derive(Debug)]
#[allow(dead_code)]
pub struct EinAuszahlungRow {
    pub waehrung: String,
    pub abwicklungsdatum: String,
    pub beschreibung: String,
    pub betrag: f64,
}

/// `Dividenden` – dividend income.
#[derive(Debug)]
#[allow(dead_code)]
pub struct DividendeRow {
    pub waehrung: String,
    pub datum: String,
    pub beschreibung: String,
    pub betrag: f64,
}

/// `Quellensteuer` – withholding tax.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct QuellensteuerRow {
    pub waehrung: String,
    pub datum: String,
    pub beschreibung: String,
    pub betrag: f64,
    pub code: String,
}

/// `Zinsen` – interest income.
#[derive(Debug)]
#[allow(dead_code)]
pub struct ZinsRow {
    pub waehrung: String,
    pub datum: String,
    pub beschreibung: String,
    pub betrag: f64,
}

/// `Aufgelaufene Zinsen` – accrued interest (key → numeric value per currency).
#[derive(Debug)]
#[allow(dead_code)]
pub struct AufgelaufeneZinsRow {
    pub waehrung: String,
    pub feldname: String,
    pub feldwert: f64,
}

/// `Veränderung im Dividendenanfall` – changes in accrued dividends.
#[derive(Debug)]
#[allow(dead_code)]
pub struct DividendenanfallRow {
    pub vermoegenswert_kategorie: String,
    pub waehrung: String,
    pub symbol: String,
    pub datum: String,
    pub ex_tag: String,
    pub zahlungsdatum: String,
    pub menge: f64,
    pub steuer: f64,
    pub gebuehr: f64,
    pub bruttosatz: f64,
    pub bruttobetrag: f64,
    pub nettobetrag: f64,
    pub code: String,
}

/// `Stock Yield Enhancement Program Securities Lent` – current SYEP loans outstanding.
#[derive(Debug)]
#[allow(dead_code)]
pub struct SyepLentRow {
    pub vermoegenswert_kategorie: String,
    pub waehrung: String,
    pub symbol: String,
    pub transaktions_id: String,
    pub menge: f64,
    pub zinssatz_pct: f64,
    pub sicherheitsbetrag: f64,
}

/// `Stock Yield Enhancement Program Securities Lent Activity` – SYEP loan events.
/// The CSV header has an intentionally blank column between `Beschreibung` and
/// `Transaktions ID-Nummer`; that blank field is skipped when parsing.
#[derive(Debug)]
#[allow(dead_code)]
pub struct SyepActivityRow {
    pub vermoegenswert_kategorie: String,
    pub waehrung: String,
    pub symbol: String,
    pub datum: String,
    pub beschreibung: String,
    pub transaktions_id: String,
    pub menge: f64,
    pub sicherheitsbetrag: f64,
}

/// `Stock Yield Enhancement Program Securities Lent Interest Details`.
#[derive(Debug)]
#[allow(dead_code)]
pub struct SyepInterestRow {
    pub waehrung: String,
    pub abwicklungsdatum: String,
    pub symbol: String,
    pub anfangsdatum: String,
    pub menge: f64,
    pub sicherheitsbetrag: f64,
    pub market_based_rate_pct: f64,
    pub zinssatz_pct: f64,
    pub zinsen_an_kunden: f64,
    pub code: String,
}

/// `Codes` – transaction code glossary.
#[derive(Debug)]
#[allow(dead_code)]
pub struct CodeRow {
    pub code: String,
    pub bedeutung: String,
    pub code_forts: String,
    pub bedeutung_forts: String,
}

/// `Hinweise/Rechtshinweise` – legal notices.
#[derive(Debug)]
#[allow(dead_code)]
pub struct HinweisRow {
    pub typ: String,
    pub hinweis: String,
}

// ============================================================
// Top-level data container
// ============================================================

#[allow(dead_code)]
#[derive(Debug)]
pub struct KontoauszugData {
    pub statement: Vec<StatementRow>,
    pub kontoinformation: Vec<KontoinformationRow>,
    pub nettovermoegenswert: Vec<NettovermoegensRow>,
    pub zeitgewichtete_rendite: Vec<ZeitgewichteteRenditeRow>,
    pub veraenderung_nav: Vec<VeraenderungNavRow>,
    pub mtm_performance: Vec<MtmPerformanceRow>,
    pub performance_uebersicht: Vec<PerformanceUebersichtRow>,
    pub cash_bericht: Vec<CashBerichtRow>,
    pub offene_positionen: Vec<OffenePositionRow>,
    pub devisenpositionen: Vec<DevisenpositionRow>,
    pub netto_aktien: Vec<NettoAktienpositionRow>,
    pub transaktionen: Vec<TransaktionRow>,
    pub transfers: Vec<TransferRow>,
    pub devisen_transaktionen: Vec<DevisenTransaktionRow>,
    pub transaktionsgebuehren: Vec<TransaktionsgebuehrRow>,
    pub kapitalmassnnahmen: Vec<KapitalmassnahmeRow>,
    pub ein_auszahlungen: Vec<EinAuszahlungRow>,
    pub dividenden: Vec<DividendeRow>,
    pub quellensteuer: Vec<QuellensteuerRow>,
    pub zinsen: Vec<ZinsRow>,
    pub aufgelaufene_zinsen: Vec<AufgelaufeneZinsRow>,
    pub dividendenanfall: Vec<DividendenanfallRow>,
    pub syep_lent: Vec<SyepLentRow>,
    pub syep_activity: Vec<SyepActivityRow>,
    pub syep_interest: Vec<SyepInterestRow>,
    pub finanzinstrumente: FinanzInstrumentInfos,
    pub codes: Vec<CodeRow>,
    pub hinweise: Vec<HinweisRow>,
    /// Total P&L for the statement period (standalone row in the CSV).
    pub gesamt_guv: f64,
}

fn convert_month(month: &str) -> Result<u32> {
    match month {
        "Januar" => Ok(1),
        "Februar" => Ok(2),
        "März" => Ok(3),
        "April" => Ok(4),
        "Mai" => Ok(5),
        "Juni" => Ok(6),
        "Juli" => Ok(7),
        "August" => Ok(8),
        "September" => Ok(9),
        "Oktober" => Ok(10),
        "November" => Ok(11),
        "Dezember" => Ok(12),
        _ => Err(Error::InvalidMonthName(month.to_string())),
    }
}

impl KontoauszugData {
    pub fn get_timestamp(&self) -> Result<i64> {
        let re = regex::Regex::new(r"([a-zA-Z]*) (\d+), (\d{4})$").unwrap();
        for row in &self.statement {
            if row.feldname == "Period"
                && let Some(caps) = re.captures(&row.feldwert)
                && let Some(month) = caps.get(1)
            {
                let month = convert_month(month.as_str())?;
                if let Some(day_string) = caps.get(2) {
                    let day: u32 = day_string.as_str().parse()?;
                    if let Some(year_string) = caps.get(3) {
                        let year: i32 = year_string.as_str().parse()?;
                        let date = chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap();
                        return Ok(date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp());
                    }
                }
            }
        }
        Err(Error::DateNotFound)
    }
}
// ============================================================
// Raw CSV loading
// ============================================================

/// Raw rows grouped by table name.
/// Each entry: `(row_kind, fields_after_table_name_and_row_kind)`.
#[derive(Default, Debug)]
struct Groups {
    groups: HashMap<String, Vec<(String, Vec<String>)>>,
}

impl Groups {
    fn load_groups(path: &Path) -> Result<Self> {
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_path(path)?;

        let mut groups: HashMap<String, Vec<(String, Vec<String>)>> = HashMap::new();

        for result in rdr.records() {
            let rec = result?;
            if rec.len() < 2 {
                continue;
            }
            let table = rec[0].trim().to_string();
            if table.is_empty() {
                continue;
            }
            let kind = rec[1].trim().to_string();
            let fields: Vec<String> = rec.iter().skip(2).map(str::to_string).collect();
            groups.entry(table).or_default().push((kind, fields));
        }
        Ok(Self { groups })
    }

    /// Iterate over all `Data`-kind rows for a given table.
    fn data_rows<'a>(&'a self, table: &str) -> impl Iterator<Item = &'a Vec<String>> {
        self.groups
            .get(table)
            .into_iter()
            .flatten()
            .filter(|(kind, _)| kind == "Data")
            .map(|(_, fields)| fields)
    }

    /// Create HashMap of Header names to column indices
    fn get_header_indizes(&self, table: &str) -> Result<HashMap<String, usize>> {
        if let Some(table) = self.groups.get(table) {
            for v in table {
                if v.0 == "Header" {
                    return Ok(v
                        .1
                        .iter()
                        .enumerate()
                        .map(|(i, s)| (s.to_owned(), i))
                        .collect());
                }
            }
        }
        Err(crate::error::Error::FailedToParseHeaderOfTable(
            table.to_string(),
        ))
    }
}
// ============================================================
// Parsing
// ============================================================

pub fn parse_kontoauszug(path: &Path) -> Result<KontoauszugData> {
    let groups = Groups::load_groups(path)?;

    // ── Statement ────────────────────────────────────────────────────────────
    let statement: Vec<StatementRow> = groups
        .data_rows("Statement")
        .map(|f| StatementRow {
            feldname: c(f, 0).to_string(),
            feldwert: c(f, 1).to_string(),
        })
        .collect();

    // ── Kontoinformation ─────────────────────────────────────────────────────
    let kontoinformation: Vec<KontoinformationRow> = groups
        .data_rows("Kontoinformation")
        .map(|f| KontoinformationRow {
            feldname: c(f, 0).to_string(),
            feldwert: c(f, 1).to_string(),
        })
        .collect();

    // ── Nettovermögenswert ───────────────────────────────────────────────────
    // This table has two consecutive header rows with different schemas.
    // We track which header is currently active as we walk the rows in order.
    let (nettovermoegenswert, zeitgewichtete_rendite) = {
        let mut main_rows: Vec<NettovermoegensRow> = Vec::new();
        let mut zgr_rows: Vec<ZeitgewichteteRenditeRow> = Vec::new();
        let mut in_zgr = false;

        for (kind, fields) in groups
            .groups
            .get("Nettovermögenswert")
            .into_iter()
            .flatten()
        {
            match kind.as_str() {
                "Header" => {
                    in_zgr = c(fields, 0) == "Zeitgewichtete Rendite";
                }
                "Data" if !in_zgr && c(fields, 0) != "Gesamt" => {
                    main_rows.push(NettovermoegensRow {
                        assetklasse: c(fields, 0).to_string(),
                        vorheriger_gesamtwert: fv(c(fields, 1)),
                        aktuell_long: fv(c(fields, 2)),
                        aktuell_short: fv(c(fields, 3)),
                        aktueller_gesamtwert: fv(c(fields, 4)),
                        veraenderung: fv(c(fields, 5)),
                    });
                }
                "Data" if in_zgr => {
                    zgr_rows.push(ZeitgewichteteRenditeRow {
                        rendite_pct: fv(c(fields, 0)),
                    });
                }
                _ => {}
            }
        }
        (main_rows, zgr_rows)
    };

    // ── Veränderung des NAV ──────────────────────────────────────────────────
    let veraenderung_nav: Vec<VeraenderungNavRow> = groups
        .data_rows("Veränderung des NAV")
        .map(|f| VeraenderungNavRow {
            feldname: c(f, 0).to_string(),
            feldwert: fv(c(f, 1)),
        })
        .collect();

    // ── Mark-to-Market-Performance-Überblick ─────────────────────────────────
    // Skip aggregate rows (Gesamt, Gesamt (Alle Vermögenswerte), Brokers Zinsaufwand …)
    // by requiring the category to be "Aktien" or "Devisen".
    let mtm_performance: Vec<MtmPerformanceRow> = groups
        .data_rows("Mark-to-Market-Performance-Überblick")
        .filter(|f| matches!(c(f, 0), "Aktien" | "Devisen"))
        .map(|f| MtmPerformanceRow {
            vermoegenswert_kategorie: c(f, 0).to_string(),
            symbol: c(f, 1).to_string(),
            vorher_menge: opt_f64(c(f, 2)),
            aktuell_menge: opt_f64(c(f, 3)),
            vorher_kurs: opt_f64(c(f, 4)),
            aktuell_kurs: opt_f64(c(f, 5)),
            mtm_pl_position: fv(c(f, 6)),
            mtm_pl_transaktion: fv(c(f, 7)),
            mtm_pl_provisionen: fv(c(f, 8)),
            mtm_pl_sonstige: fv(c(f, 9)),
            mtm_pl_gesamt: fv(c(f, 10)),
            code: c(f, 11).to_string(),
        })
        .collect();

    // ── Übersicht zur realisierten und unrealisierten Performance ────────────
    // Note the double space in the table name as it appears in the CSV.
    let performance_uebersicht: Vec<PerformanceUebersichtRow> = groups
        .data_rows("Übersicht  zur realisierten und unrealisierten Performance")
        .filter(|f| matches!(c(f, 0), "Aktien" | "Devisen"))
        .map(|f| PerformanceUebersichtRow {
            vermoegenswert_kategorie: c(f, 0).to_string(),
            symbol: c(f, 1).to_string(),
            kostenanpassung: fv(c(f, 2)),
            realisiert_st_gewinn: fv(c(f, 3)),
            realisiert_st_verlust: fv(c(f, 4)),
            realisiert_lt_gewinn: fv(c(f, 5)),
            realisiert_lt_verlust: fv(c(f, 6)),
            realisiert_gesamt: fv(c(f, 7)),
            unrealisiert_st_gewinn: fv(c(f, 8)),
            unrealisiert_st_verlust: fv(c(f, 9)),
            unrealisiert_lt_gewinn: fv(c(f, 10)),
            unrealisiert_lt_verlust: fv(c(f, 11)),
            unrealisiert_gesamt: fv(c(f, 12)),
            gesamt: fv(c(f, 13)),
            code: c(f, 14).to_string(),
        })
        .collect();

    // ── Cash-Bericht ─────────────────────────────────────────────────────────
    let cash_bericht: Vec<CashBerichtRow> = groups
        .data_rows("Cash-Bericht")
        .map(|f| CashBerichtRow {
            waehrungsuebersicht: c(f, 0).to_string(),
            waehrung: c(f, 1).to_string(),
            gesamt: fv(c(f, 2)),
            wertpapiere: fv(c(f, 3)),
            futures: fv(c(f, 4)),
        })
        .collect();

    // ── Offene Positionen ─────────────────────────────────────────────────────
    // Only "Summary" discriminator rows are individual positions;
    // aggregate sub-rows use row-kind "Total" and are already excluded by
    // the `data_rows` filter.
    let offene_positionen: Vec<OffenePositionRow> = groups
        .data_rows("Offene Positionen")
        .filter(|f| c(f, 0) == "Summary")
        .map(|f| OffenePositionRow {
            data_discriminator: c(f, 0).to_string(),
            vermoegenswert_kategorie: c(f, 1).to_string(),
            waehrung: c(f, 2).to_string(),
            symbol: c(f, 3).to_string(),
            menge: opt_f64(c(f, 4)),
            multiplikator: fv(c(f, 5)),
            einstands_kurs: fv(c(f, 6)),
            kostenbasis: fv(c(f, 7)),
            schlusskurs: opt_f64(c(f, 8)),
            wert: fv(c(f, 9)),
            unrealisierter_gv: fv(c(f, 10)),
            code: c(f, 11).to_string(),
        })
        .collect();

    // ── Devisenpositionen ─────────────────────────────────────────────────────
    // The last row has category "Gesamt"; skip it.
    let devisenpositionen: Vec<DevisenpositionRow> = groups
        .data_rows("Devisenpositionen")
        .filter(|f| c(f, 0) == "Devisen")
        .map(|f| DevisenpositionRow {
            vermoegenswert_kategorie: c(f, 0).to_string(),
            waehrung: c(f, 1).to_string(),
            beschreibung: c(f, 2).to_string(),
            menge: fv(c(f, 3)),
            einstands_kurs: fv(c(f, 4)),
            kostenbasis_eur: fv(c(f, 5)),
            schlusskurs: fv(c(f, 6)),
            wert_eur: fv(c(f, 7)),
            unrealisierter_gv_eur: fv(c(f, 8)),
            code: c(f, 9).to_string(),
        })
        .collect();

    // ── Netto-Aktienpositionsübersicht ────────────────────────────────────────
    let netto_aktien: Vec<NettoAktienpositionRow> = groups
        .data_rows("Netto-Aktienpositionsübersicht")
        .map(|f| NettoAktienpositionRow {
            vermoegenswert_kategorie: c(f, 0).to_string(),
            waehrung: c(f, 1).to_string(),
            symbol: c(f, 2).to_string(),
            beschreibung: c(f, 3).to_string(),
            aktien_bei_ib: iv(c(f, 4)),
            aktien_geliehen: iv(c(f, 5)),
            aktien_verliehen: iv(c(f, 6)),
            netto_aktien: iv(c(f, 7)),
        })
        .collect();

    // ── Transaktionen ─────────────────────────────────────────────────────────
    // The table uses two different headers (one for securities, one for FX).
    // Both have 14 fields after skipping table name and row kind; we dispatch
    // on `Vermögenswertkategorie` (field 1) rather than tracking the active header.
    //
    // SubTotal / Total rows use row-kind "SubTotal" / "Total" and are already
    // excluded by `data_rows`.
    let (transaktionen, devisen_transaktionen) = {
        let mut stock: Vec<TransaktionRow> = Vec::new();
        let mut fx: Vec<DevisenTransaktionRow> = Vec::new();

        for f in groups.data_rows("Transaktionen") {
            match c(f, 1) {
                "Aktien" => stock.push(TransaktionRow {
                    data_discriminator: c(f, 0).to_string(),
                    vermoegenswert_kategorie: c(f, 1).to_string(),
                    waehrung: c(f, 2).to_string(),
                    symbol: c(f, 3).to_string(),
                    datum_zeit: c(f, 4).to_string(),
                    menge: fv(c(f, 5)),
                    transaktions_kurs: fv(c(f, 6)),
                    schlusskurs: fv(c(f, 7)),
                    erloese: fv(c(f, 8)),
                    prov_gebuehr: fv(c(f, 9)),
                    basis: fv(c(f, 10)),
                    realisierter_gv: fv(c(f, 11)),
                    mtm_gv: fv(c(f, 12)),
                    code: c(f, 13).to_string(),
                }),
                // FX header: col 7 is blank; col 9 = Provisionseink. EUR; col 12 = MTM in EUR
                "Devisen" => fx.push(DevisenTransaktionRow {
                    data_discriminator: c(f, 0).to_string(),
                    vermoegenswert_kategorie: c(f, 1).to_string(),
                    waehrung: c(f, 2).to_string(),
                    symbol: c(f, 3).to_string(),
                    datum_zeit: c(f, 4).to_string(),
                    menge: fv(c(f, 5)),
                    transaktions_kurs: fv(c(f, 6)),
                    // col 7 intentionally blank in FX header
                    erloese: fv(c(f, 8)),
                    provisionen_eur: fv(c(f, 9)),
                    // cols 10 and 11 intentionally blank in FX header
                    mtm_eur: fv(c(f, 12)),
                    code: c(f, 13).to_string(),
                }),
                _ => {} // skip empty / unexpected categories
            }
        }
        (stock, fx)
    };

    // ── Transaktionsgebühren ──────────────────────────────────────────────────
    let transaktionsgebuehren: Vec<TransaktionsgebuehrRow> = groups
        .data_rows("Transaktionsgebühren")
        .filter(|f| !c(f, 0).starts_with("Gesamt"))
        .map(|f| TransaktionsgebuehrRow {
            vermoegenswert_kategorie: c(f, 0).to_string(),
            waehrung: c(f, 1).to_string(),
            datum_zeit: c(f, 2).to_string(),
            symbol: c(f, 3).to_string(),
            beschreibung: c(f, 4).to_string(),
            menge: fv(c(f, 5)),
            handelskurs: fv(c(f, 6)),
            betrag: fv(c(f, 7)),
        })
        .collect();

    // ── Transfers ──────────────────────────────────────────────────
    let transfers: Vec<TransferRow> = groups
        .data_rows("Transfers")
        .filter(|f| !c(f, 0).starts_with("Gesamt"))
        .map(|f| TransferRow {
            vermoegenswert_kategorie: c(f, 0).to_string(),
            waehrung: c(f, 1).to_string(),
            symbol: c(f, 2).to_string(),
            datum: c(f, 3).to_string(),
            typ: c(f, 4).to_string(),
            richtung: c(f, 5).to_string(),
            überweisendes_unternehmen: c(f, 6).to_string(),
            transfer_konto: c(f, 7).to_string(),
            menge: fv(c(f, 8)),
            transferpreis: c(f, 9).to_string(),
            marktwert: fv(c(f, 10)),
            realisierter_guv: fv(c(f, 11)),
            barbetrag: fv(c(f, 12)),
            code: c(f, 13).to_string(),
        })
        .collect();

    // ── Kapitalmaßnahmen ──────────────────────────────────────────────────────
    let kapitalmassnnahmen: Vec<KapitalmassnahmeRow> = groups
        .data_rows("Kapitalmaßnahmen")
        .filter(|f| !c(f, 0).starts_with("Gesamt"))
        .map(|f| KapitalmassnahmeRow {
            vermoegenswert_kategorie: c(f, 0).to_string(),
            waehrung: c(f, 1).to_string(),
            berichtsdatum: c(f, 2).to_string(),
            datum_zeit: c(f, 3).to_string(),
            beschreibung: c(f, 4).to_string(),
            menge: fv(c(f, 5)),
            erloese: fv(c(f, 6)),
            wert: fv(c(f, 7)),
            realisierter_gv: fv(c(f, 8)),
            code: c(f, 9).to_string(),
        })
        .collect();

    // ── Einzahlungen & Auszahlungen ───────────────────────────────────────────
    // Skip summary rows: they have an empty Abwicklungsdatum (field 1).
    let ein_auszahlungen: Vec<EinAuszahlungRow> = groups
        .data_rows("Einzahlungen & Auszahlungen")
        .filter(|f| !c(f, 1).is_empty())
        .map(|f| EinAuszahlungRow {
            waehrung: c(f, 0).to_string(),
            abwicklungsdatum: c(f, 1).to_string(),
            beschreibung: c(f, 2).to_string(),
            betrag: fv(c(f, 3)),
        })
        .collect();

    // ── Dividenden ────────────────────────────────────────────────────────────
    // Real rows have a non-empty Datum (field 1); summary rows (Gesamt, etc.) don't.
    let dividenden: Vec<DividendeRow> = groups
        .data_rows("Dividenden")
        .filter(|f| !c(f, 1).is_empty())
        .map(|f| DividendeRow {
            waehrung: c(f, 0).to_string(),
            datum: c(f, 1).to_string(),
            beschreibung: c(f, 2).to_string(),
            betrag: fv(c(f, 3)),
        })
        .collect();

    // ── Quellensteuer ─────────────────────────────────────────────────────────
    let quellensteuer: Vec<QuellensteuerRow> = groups
        .data_rows("Quellensteuer")
        .filter(|f| !c(f, 1).is_empty())
        .map(|f| QuellensteuerRow {
            waehrung: c(f, 0).to_string(),
            datum: c(f, 1).to_string(),
            beschreibung: c(f, 2).to_string(),
            betrag: fv(c(f, 3)),
            code: c(f, 4).to_string(),
        })
        .collect();

    // ── Zinsen ────────────────────────────────────────────────────────────────
    let zinsen: Vec<ZinsRow> = groups
        .data_rows("Zinsen")
        .map(|f| {
            let mut zins = ZinsRow {
                waehrung: c(f, 0).to_string(),
                datum: c(f, 1).to_string(),
                beschreibung: c(f, 2).to_string(),
                betrag: fv(c(f, 3)),
            };
            if zins.beschreibung.is_empty() {
                // re-arange summary row
                zins.beschreibung = zins.waehrung;
                zins.waehrung = "".to_string();
            }
            zins
        })
        .collect();

    // ── Aufgelaufene Zinsen ───────────────────────────────────────────────────
    let aufgelaufene_zinsen: Vec<AufgelaufeneZinsRow> = groups
        .data_rows("Aufgelaufene Zinsen")
        .map(|f| AufgelaufeneZinsRow {
            waehrung: c(f, 0).to_string(),
            feldname: c(f, 1).to_string(),
            feldwert: fv(c(f, 2)),
        })
        .collect();

    // ── Veränderung im Dividendenanfall ───────────────────────────────────────
    // Real data rows have category "Aktien" (field 0); all other rows are summary
    // or boundary rows (Anfangsstand / Endstand / Gesamt / Gesamtwert …).
    let dividendenanfall: Vec<DividendenanfallRow> = groups
        .data_rows("Veränderung im Dividendenanfall")
        .filter(|f| c(f, 0) == "Aktien")
        .map(|f| DividendenanfallRow {
            vermoegenswert_kategorie: c(f, 0).to_string(),
            waehrung: c(f, 1).to_string(),
            symbol: c(f, 2).to_string(),
            datum: c(f, 3).to_string(),
            ex_tag: c(f, 4).to_string(),
            zahlungsdatum: c(f, 5).to_string(),
            menge: fv(c(f, 6)),
            steuer: fv(c(f, 7)),
            gebuehr: fv(c(f, 8)),
            bruttosatz: fv(c(f, 9)),
            bruttobetrag: fv(c(f, 10)),
            nettobetrag: fv(c(f, 11)),
            code: c(f, 12).to_string(),
        })
        .collect();

    // ── SYEP Securities Lent ──────────────────────────────────────────────────
    let syep_lent: Vec<SyepLentRow> = groups
        .data_rows("Stock Yield Enhancement Program Securities Lent")
        .filter(|f| !c(f, 0).starts_with("Gesamt"))
        .map(|f| SyepLentRow {
            vermoegenswert_kategorie: c(f, 0).to_string(),
            waehrung: c(f, 1).to_string(),
            symbol: c(f, 2).to_string(),
            transaktions_id: c(f, 3).to_string(),
            menge: fv(c(f, 4)),
            zinssatz_pct: fv(c(f, 5)),
            sicherheitsbetrag: fv(c(f, 6)),
        })
        .collect();

    // ── SYEP Securities Lent Activity ─────────────────────────────────────────
    // The CSV header has a blank column (index 5) between "Beschreibung" and
    // "Transaktions ID-Nummer"; we skip it.
    let syep_activity: Vec<SyepActivityRow> = groups
        .data_rows("Stock Yield Enhancement Program Securities Lent Activity")
        .filter(|f| c(f, 0) == "Aktien")
        .map(|f| SyepActivityRow {
            vermoegenswert_kategorie: c(f, 0).to_string(),
            waehrung: c(f, 1).to_string(),
            symbol: c(f, 2).to_string(),
            datum: c(f, 3).to_string(),
            beschreibung: c(f, 4).to_string(),
            // field index 5 is the blank separator column
            transaktions_id: c(f, 6).to_string(),
            menge: fv(c(f, 7)),
            sicherheitsbetrag: fv(c(f, 8)),
        })
        .collect();

    // ── SYEP Interest Details ─────────────────────────────────────────────────
    let syep_interest: Vec<SyepInterestRow> = groups
        .data_rows("Stock Yield Enhancement Program Securities Lent Interest Details")
        .filter(|f| !c(f, 0).starts_with("Gesamt"))
        .map(|f| SyepInterestRow {
            waehrung: c(f, 0).to_string(),
            abwicklungsdatum: c(f, 1).to_string(),
            symbol: c(f, 2).to_string(),
            anfangsdatum: c(f, 3).to_string(),
            menge: fv(c(f, 4)),
            sicherheitsbetrag: fv(c(f, 5)),
            market_based_rate_pct: fv(c(f, 6)),
            zinssatz_pct: fv(c(f, 7)),
            zinsen_an_kunden: fv(c(f, 8)),
            code: c(f, 9).to_string(),
        })
        .collect();

    // ── Informationen zum Finanzinstrument ────────────────────────────────────
    let finanzinstrumente = FinanzInstrumentInfos::parse_from_groups(&groups)?;

    // ── Codes ─────────────────────────────────────────────────────────────────
    let codes: Vec<CodeRow> = groups
        .data_rows("Codes")
        .map(|f| CodeRow {
            code: c(f, 0).to_string(),
            bedeutung: c(f, 1).to_string(),
            code_forts: c(f, 2).to_string(),
            bedeutung_forts: c(f, 3).to_string(),
        })
        .collect();

    // ── Hinweise/Rechtshinweise ───────────────────────────────────────────────
    let hinweise: Vec<HinweisRow> = groups
        .data_rows("Hinweise/Rechtshinweise")
        .map(|f| HinweisRow {
            typ: c(f, 0).to_string(),
            hinweis: c(f, 1).to_string(),
        })
        .collect();

    // ── Gesamt-G&V ────────────────────────────────────────────────────────────
    // This is a standalone row: `Gesamt-G&V des Kontoauszugszeitraums,,…,<value>,`
    // The value sits at field index 10 (after stripping the table name and the
    // empty row-kind field).
    let gesamt_guv = groups
        .groups
        .get("Gesamt-G&V des Kontoauszugszeitraums")
        .and_then(|rows| rows.first())
        .map(|(_, fields)| fv(c(fields, 10)))
        .unwrap_or(0.0);

    Ok(KontoauszugData {
        statement,
        kontoinformation,
        nettovermoegenswert,
        zeitgewichtete_rendite,
        veraenderung_nav,
        mtm_performance,
        performance_uebersicht,
        cash_bericht,
        offene_positionen,
        devisenpositionen,
        netto_aktien,
        transaktionen,
        transfers,
        devisen_transaktionen,
        transaktionsgebuehren,
        kapitalmassnnahmen,
        ein_auszahlungen,
        dividenden,
        quellensteuer,
        zinsen,
        aufgelaufene_zinsen,
        dividendenanfall,
        syep_lent,
        syep_activity,
        syep_interest,
        finanzinstrumente,
        codes,
        hinweise,
        gesamt_guv,
    })
}

impl KontoauszugData {
    pub fn get_quellensteuer(
        &self,
        fx_rates: &FxRates,
    ) -> Result<(QuellensteuerPerJurisdiktion, QuellensteuerPerJurisdiktion)> {
        let mut aktien_qsteuer = QuellensteuerPerJurisdiktion::default();
        let mut etf_qsteuer = QuellensteuerPerJurisdiktion::default();
        for tax in &self.quellensteuer {
            if tax.beschreibung.contains("Kreditzinsen") {
                log::warn!(
                    "Quellensteuer auf Zinsen werden ignoriert: {}",
                    tax.beschreibung
                );
                continue;
            }
            let timestamp = convert_date(&tax.datum)?;
            let fx = fx_rates.get_fx_rate(timestamp, &tax.waehrung)?;
            let (symbol, isin) = parse_asset_ids(&tax.beschreibung)?;
            let is_etf = self.finanzinstrumente.is_etf(&symbol, Some(&isin))?;
            let jurisdiction = parse_jurisdiction(&tax.beschreibung)?;
            let qtax_by_jurisdiction = if is_etf {
                &mut etf_qsteuer
            } else {
                &mut aktien_qsteuer
            };
            qtax_by_jurisdiction.insert(
                jurisdiction,
                Quellensteuer {
                    beschreibung: tax.beschreibung.clone(),
                    währung: tax.waehrung.clone(),
                    betrag: tax.betrag,
                    eur_betrag: fx * tax.betrag,
                    datum: tax.datum.clone(),
                },
            );
        }
        Ok((aktien_qsteuer, etf_qsteuer))
    }

    pub fn get_open_etf_positions(&self) -> Result<EtfPositionen> {
        log::debug!("get_open_etf_positions gestartet");
        let mut positionen = EtfPositionen::default();
        for position in &self.offene_positionen {
            log::trace!("Check position {position:?}");
            let is_etf = self.finanzinstrumente.is_etf(&position.symbol, None)?;
            if let Some(menge) = position.menge
                && is_etf
            {
                positionen.add_position(
                    &position.symbol,
                    EtfPosition::new(menge, position.wert, FondsTyp::Aktien, 12),
                );
            }
        }
        Ok(positionen)
    }

    pub fn get_account_info(&self) -> Option<(String, String)> {
        for row in &self.kontoinformation {
            if &row.feldname == "Account" {
                return Some(("InteractiveBrokers".to_string(), row.feldwert.clone()));
            }
        }
        None
    }
}

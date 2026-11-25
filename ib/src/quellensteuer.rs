use crate::formatting::round2;
use serde::{Deserialize, Serialize};

use std::{
    collections::HashMap,
    fmt::{self, Display},
};

//type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Quellensteuer {
    pub beschreibung: String,
    pub datum: String,
    pub währung: String,
    pub betrag: f64,
    pub eur_betrag: f64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct QuellensteuerPerJurisdiktion {
    // Abegführte Quellenstuer nach Jufisdiktionen
    qsteuer_per_juris: HashMap<String, Vec<Quellensteuer>>,
}

impl QuellensteuerPerJurisdiktion {
    pub fn insert(&mut self, jurisdiktion: String, qtax: Quellensteuer) {
        if let Some(val) = self.qsteuer_per_juris.get_mut(&jurisdiktion) {
            val.push(qtax);
        } else {
            self.qsteuer_per_juris.insert(jurisdiktion, vec![qtax]);
        }
    }
}

impl Display for QuellensteuerPerJurisdiktion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut key_count = 0;
        let mut sum = 0.0;
        if let Some(german_qtax) = self.qsteuer_per_juris.get("DE") {
            key_count += 1;
            writeln!(
                f,
                "Abgeführte deutsche Quellensteuer auf Dividenden (inkl. Solidaritätszuschlag"
            )?;
            writeln!(
                f,
                r#"#table(
    columns: (auto, auto, auto),
    align: (left, right, right),
    stroke: 0.5pt,
    inset: 8pt,
    table.header([*Beschreibung*],[*Datum*],[*Abgeführte Quellensteuer*]),"#
            )?;
            for tax in german_qtax {
                writeln!(
                    f,
                    "[{}],[{}],[{:.2} {}],",
                    tax.beschreibung,
                    tax.datum,
                    round2(tax.betrag),
                    tax.währung,
                )?;
                sum += tax.betrag;
            }
            writeln!(f, ")\nGesamtbetrag in EUR: {:9.2}", round2(sum))?;
        } else {
            writeln!(f, "Es wurden keine deutschen Quellensteuern abgeführt.\n")?;
        }
        if self.qsteuer_per_juris.len() > key_count {
            writeln!(
                f,
                "\nAbgeführte ausländische Quellensteuer nach Jurisdiktion"
            )?;
            for jurisdiction in self.qsteuer_per_juris.keys() {
                if jurisdiction == "DE" {
                    continue;
                }
                writeln!(f, "\nJurisdiction: {jurisdiction}")?;
                let mut waehrung = None;
                let mut eur_sum = 0.0;
                let mut curr_sum = 0.0;
                writeln!(
                    f,
                    r#"#table(
        columns: (auto, auto, auto, auto),
        align: (left, right, right, right),
        stroke: 0.5pt,
        inset: 8pt,
        table.header([*Beschreibung*],[*Datum*],[*Abgeführte Quellensteuer in FW*],[*in EUR*]),"#
                )?;
                for tax in &self.qsteuer_per_juris[jurisdiction] {
                    if let Some(waehrung) = waehrung {
                        if waehrung != tax.währung {
                            log::warn!("Inkonsistente Währung in derselben Jurisdiktion!");
                        }
                    } else {
                        waehrung = Some(tax.währung.as_str());
                    }
                    writeln!(
                        f,
                        "[{}],[{}],[{:.2} {:3}],[{:.2} EUR],",
                        tax.beschreibung,
                        tax.datum,
                        round2(tax.betrag),
                        tax.währung,
                        round2(tax.eur_betrag),
                    )?;
                    curr_sum += tax.betrag;
                    eur_sum += tax.eur_betrag;
                    sum += tax.eur_betrag;
                }
                writeln!(
                    f,
                    ")\nGesamtbetrag in {}: {:.2} oder {:.2} EUR",
                    waehrung.unwrap_or("unknown"),
                    round2(curr_sum),
                    round2(eur_sum)
                )?;
            }
        } else {
            writeln!(
                f,
                "Es wurden keine Quellensteuer für ausländische Jurisdiktionen abgeführt.\n"
            )?;
        }
        if sum != 0.0 {
            writeln!(
                f,
                "Gesamtbetrag über alle Jurisdiktionen (einschl. EUR): {sum:9.2} EUR"
            )?;
        }
        Ok(())
    }
}

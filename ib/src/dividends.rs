use crate::date::convert_date;
use crate::error::Error;
use crate::formatting::round2;
use crate::fx::FxRates;
use crate::parser::parse_asset_ids;
use crate::read::KontoauszugData;
use serde::{Deserialize, Serialize};
use std::fmt;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Dividende {
    pub beschreibung: String,
    pub date: String,
    pub betrag: f64,
    pub währung: String,
    pub eur_betrag: f64,
    pub is_etf: bool,
}

pub fn berechne_dividenden(
    kontoauszug: &KontoauszugData,
    fx_rates: &FxRates,
) -> Result<(Dividenden, Dividenden)> {
    let mut aktien_dividenden = Dividenden::default();
    let mut etf_dividenden = Dividenden::default();
    for div in &kontoauszug.dividenden {
        let date = convert_date(&div.datum)?;
        let fx = fx_rates.get_fx_rate(date, &div.waehrung)?;
        let eur_betrag = fx * div.betrag;
        let (symbol, isin) = parse_asset_ids(&div.beschreibung)?;
        let dividende = Dividende {
            beschreibung: div.beschreibung.clone(),
            date: div.datum.clone(),
            betrag: round2(div.betrag),
            währung: div.waehrung.clone(),
            eur_betrag: round2(eur_betrag),
            is_etf: kontoauszug.finanzinstrumente.is_etf(&symbol, Some(&isin))?,
        };
        if kontoauszug.finanzinstrumente.is_etf(&symbol, Some(&isin))? {
            etf_dividenden.add(dividende);
        } else {
            aktien_dividenden.add(dividende);
        }
    }
    Ok((aktien_dividenden, etf_dividenden))
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Dividenden {
    dividenden: Vec<Dividende>,
}

impl Dividenden {
    fn add(&mut self, dividende: Dividende) {
        self.dividenden.push(dividende);
    }
}
impl fmt::Display for Dividenden {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut last_curr = "";
        let mut curr_sum = 0.0;
        let mut eur_curr_sum = 0.0;
        let mut eur_sum = 0.0;

        for div in &self.dividenden {
            if last_curr != div.währung {
                if !last_curr.is_empty() {
                    writeln!(
                        f,
                        ")\nSumme Dividenden in {last_curr}: {} {last_curr} oder {} EUR\n",
                        round2(curr_sum),
                        round2(eur_curr_sum),
                    )?;
                }
                writeln!(
                    f,
                    r#"#table(
        columns: (auto, auto, auto, auto),
        align: (left, right, right, right),
        stroke: 0.5pt,
        inset: 8pt,
        table.header([*Beschreibung*],[*Datum*],[*Betrag in FW*],[*Betrag in EUR*]),"#
                )?;
                last_curr = &div.währung;
                curr_sum = 0.0;
                eur_curr_sum = 0.0;
            }
            curr_sum += div.betrag;
            eur_sum += div.eur_betrag;
            eur_curr_sum += div.eur_betrag;
            writeln!(
                f,
                "[{}],[{}],[{:.2} {:3}],[{:.2} EUR],",
                div.beschreibung, div.date, div.betrag, div.währung, div.eur_betrag,
            )?;
        }
        if !last_curr.is_empty() {
            writeln!(
                f,
                ")\nSumme Dividenden in {last_curr}: {} {last_curr} oder {} EUR\n",
                round2(curr_sum),
                round2(eur_curr_sum),
            )?;
        }
        writeln!(f, "Summe aller Dividenden in EUR: {}", round2(eur_sum))?;
        Ok(())
    }
}

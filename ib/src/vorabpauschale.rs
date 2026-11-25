use crate::{
    asset_events::{AssetEvent, AssetEventList},
    error::{Error, Result},
    formatting::round2,
    read::KontoauszugData,
};

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum FondsTyp {
    Aktien,
    Mixed,
}

#[derive(Debug, Clone)]
pub struct EtfPosition {
    menge: f64,
    eur_betrag: f64,
    fonds_typ: FondsTyp,
    dividenden: f64,
    monate: u8,
}

impl EtfPosition {
    pub fn new(menge: f64, eur_betrag: f64, fonds_typ: FondsTyp, monate: u8) -> Self {
        Self {
            menge,
            eur_betrag,
            fonds_typ,
            dividenden: 0.0,
            monate,
        }
    }
}

#[derive(Debug, Default)]
pub struct EtfPositionen {
    positionen: HashMap<String, Vec<EtfPosition>>,
}

impl EtfPositionen {
    pub fn add_position(&mut self, symbol: &str, position: EtfPosition) {
        if self.positionen.contains_key(symbol) {
            if let Some(pos) = self.positionen.get_mut(symbol) {
                pos.push(position);
            }
        } else {
            self.positionen.insert(symbol.to_owned(), vec![position]);
        }
    }

    pub fn get_positionen(&self, symbol: &str) -> Option<&Vec<EtfPosition>> {
        self.positionen.get(symbol)
    }

    pub fn get_alle_positionen(&self) -> &HashMap<String, Vec<EtfPosition>> {
        &self.positionen
    }

    pub fn update_unterjährig(
        &mut self,
        kontoauszug: &KontoauszugData,
        fx_rates: &crate::fx::FxRates,
    ) -> Result<()> {
        let event_list = AssetEventList::von_kontoauszug(kontoauszug)?;
        for (date, events) in event_list.events {
            for event in &events {
                match event {
                    AssetEvent::Kauf(t) => {
                        if kontoauszug.finanzinstrumente.is_etf(&t.symbol, None)? {
                            // Käufe in ETF Positionen aufnehmen
                            let effektiver_kurs =
                                (t.menge * t.transaktions_kurs + t.prov_gebuehr) / t.menge;
                            let fx = fx_rates.get_fx_rate(date, &t.waehrung)?;
                            let eur_betrag = fx * t.menge * effektiver_kurs;
                            let fonds_typ = FondsTyp::Aktien;
                            let monate = crate::date::get_remaining_months(date)?;
                            let position = EtfPosition::new(t.menge, eur_betrag, fonds_typ, monate);
                            self.add_position(&t.symbol, position);
                        }
                    }
                    AssetEvent::Verkauf(t) => {
                        if kontoauszug.finanzinstrumente.is_etf(&t.symbol, None)? {
                            if let Some(käufe) = self.positionen.get_mut(&t.symbol) {
                                let mut verbleibende_verkäufe = -t.menge;
                                for p in käufe.iter_mut() {
                                    if p.menge < verbleibende_verkäufe {
                                        verbleibende_verkäufe -= p.menge;
                                        p.menge = 0.0;
                                    } else {
                                        p.menge -= verbleibende_verkäufe;
                                        break;
                                    }
                                }
                            } else {
                                log::warn!(
                                    "Verkäufe von ETFs werden irgnoriert, da Position nicht gefunden wurde: {t:#?}"
                                );
                            }
                        }
                    }
                    AssetEvent::Transfer(t) => {
                        if kontoauszug.finanzinstrumente.is_etf(&t.symbol, None)? {
                            log::warn!("Transfers von ETFs werden ignoriert: {t:#?}");
                        }
                    }
                    AssetEvent::Kapitalmaßnahme(k) => {
                        if kontoauszug
                            .finanzinstrumente
                            .is_etf(&k.altes_symbol, None)?
                        {
                            if let Some(käufe) = self.positionen.get(&k.altes_symbol) {
                                let factor = k.neue_menge / k.alte_menge;
                                let mut neue_käufe = (*käufe).clone();
                                neue_käufe.iter_mut().for_each(|p| {
                                    p.menge *= factor;
                                });
                                self.positionen.insert(k.neues_symbol.clone(), neue_käufe);
                                self.positionen.remove(&k.altes_symbol);
                            } else {
                                log::warn!(
                                    "Kapitalmaßnahmen für ETFs werden ignoriert, da Position nicht gefunden wurde: {k:#?}"
                                );
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VorabpauschaleInfo {
    /// Symbol
    symbol: String,
    /// Fondswert zu Beginn der Periode (Jahresanfang oder Kaufdatum falls jünger)
    start_wert: f64,
    /// Fondswert zum Edne der Periode
    end_wert: f64,
    /// Typ des Fonds
    fonds_typ: FondsTyp,
    /// ausgesschüttete Dividenden zwischen Start und Ende der Periode
    dividenden: f64,
    /// Angrebochene Anzahl der Monate, die der Fonds gehalten wurden (12 für ein ganzes Jahr)
    monate: u8,
    /// ISIN des ETFs
    isin: String,
    /// Bechreibung des ETFs
    name: String,
}

impl VorabpauschaleInfo {
    pub fn calc(&self, basis_rate: f64) -> f64 {
        let basisertrag =
            (self.start_wert * basis_rate / 100.0 * 0.7) * (self.monate as f64) / 12.0;
        let unrealisierter_gewinn = self.end_wert - self.start_wert;
        let basisertrag = basisertrag.min(unrealisierter_gewinn);
        let effektiver_basisertrag = (basisertrag - self.dividenden).max(0.0);
        let teilfreistellung = match self.fonds_typ {
            FondsTyp::Aktien => 0.3,
            FondsTyp::Mixed => 0.15,
        };
        effektiver_basisertrag * (1.0 - teilfreistellung) * 0.25 * 1.055
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Vorabpauschalen {
    pauschalen_infos: Vec<VorabpauschaleInfo>,
    /// Basiszinssatz in %
    basis_rate: f64,
}

impl Vorabpauschalen {
    pub fn sammle_vorabpauschalen_infos(
        last_kontoauszug: &KontoauszugData,
        kontoauszug: &KontoauszugData,
        basis_rate: f64,
        fx_rates: &crate::fx::FxRates,
    ) -> Result<Self> {
        log::debug!("sammle_vorabpauschalen_infos gestartet");
        let mut pauschalen_infos = Vec::new();
        let mut offene_positionen_start = last_kontoauszug.get_open_etf_positions()?;
        log::trace!("offene ETF Positionen zu Beginn: {offene_positionen_start:?}");
        offene_positionen_start.update_unterjährig(kontoauszug, fx_rates)?;
        let offene_positionen_ende = kontoauszug.get_open_etf_positions()?;
        for (etf, position) in offene_positionen_ende.get_alle_positionen() {
            if position.is_empty() {
                continue;
            }
            if position.len() != 1 {
                return Err(Error::EtfEndPositionUngleichEins(etf.to_owned()));
            }
            let position = &position[0];
            if let Some(start_positionen) = offene_positionen_start.get_positionen(etf) {
                for start_position in start_positionen {
                    let (isin, name) = kontoauszug
                        .finanzinstrumente
                        .get_isin_and_name_by_symbol(etf)?;
                    pauschalen_infos.push(VorabpauschaleInfo {
                        symbol: etf.to_string(),
                        isin,
                        name,
                        start_wert: start_position.eur_betrag,
                        end_wert: position.eur_betrag,
                        fonds_typ: position.fonds_typ,
                        dividenden: start_position.dividenden,
                        monate: start_position.monate,
                    });
                }
            } else {
                return Err(Error::PassendeEtfStartPositionFehlt(etf.to_string()));
            }
        }
        Ok(Vorabpauschalen {
            pauschalen_infos,
            basis_rate,
        })
    }
}

impl fmt::Display for Vorabpauschalen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Vorjahresbasiszins: {}\n", self.basis_rate)?;
        writeln!(
            f,
            r#"#table(
columns: (auto, auto, auto, auto, auto),
align: (left, right, right, right, right),
stroke: 0.5pt,
inset: 8pt,
table.header([*Wertpapier*],[*Monate*],[*Wert am Anfang der Haltedauer*],[*Wert Jahresende*],[*Vorabpauschale*]),"#
        )?;

        for info in &self.pauschalen_infos {
            writeln!(
                f,
                "[*{}* ({}) \\ {}],[{}],[{:.2} EUR],[{:.2} EUR],[{:.2} EUR],",
                info.isin,
                info.symbol,
                info.name,
                info.monate,
                round2(info.start_wert),
                round2(info.end_wert),
                round2(info.calc(self.basis_rate)),
            )?;
        }
        writeln!(f, ")\n")?;
        Ok(())
    }
}

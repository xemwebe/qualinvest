use crate::date::convert_date;
use crate::error::Result;
use crate::parser::parse_symbols_von_kapitalmaßnahme;
use crate::read::{KapitalmassnahmeRow, KontoauszugData, TransaktionRow, TransferRow};
use std::collections::{BTreeMap, HashMap, VecDeque};

#[derive(Debug)]
pub enum AssetEvent {
    Kauf(TransaktionRow),
    Verkauf(TransaktionRow),
    Transfer(TransferRow),
    Kapitalmaßnahme(Kapitalmaßnahme),
}

#[derive(Debug, Default)]
pub struct AssetEventList {
    pub events: BTreeMap<i64, VecDeque<AssetEvent>>,
}

#[derive(Debug, Default, Clone)]
pub struct Kapitalmaßnahme {
    pub altes_symbol: String,
    pub neues_symbol: String,
    pub alte_menge: f64,
    pub neue_menge: f64,
}

impl AssetEventList {
    pub fn von_kontoauszug(kontoauszug: &KontoauszugData) -> Result<AssetEventList> {
        let mut events = AssetEventList::default();
        for t in &kontoauszug.transaktionen {
            let timestamp = convert_date(&t.datum_zeit)?;
            if t.menge > 0.0 {
                // Kauf
                events.add(timestamp, AssetEvent::Kauf(t.clone()));
            } else {
                events.add(timestamp, AssetEvent::Verkauf(t.clone()));
            }
        }
        for t in &kontoauszug.transfers {
            let timestamp = convert_date(&t.datum)?;
            events.add(timestamp, AssetEvent::Transfer(t.clone()));
        }
        events.process_kapitalmaßnahmen(&kontoauszug.kapitalmassnnahmen)?;
        Ok(events)
    }

    fn add(&mut self, timestamp: i64, event: AssetEvent) {
        if !self.events.contains_key(&timestamp) {
            self.events.insert(timestamp, VecDeque::new());
        }
        match event {
            AssetEvent::Kauf(_) | AssetEvent::Transfer(_) | AssetEvent::Kapitalmaßnahme(_) => {
                self.events.get_mut(&timestamp).unwrap().push_front(event)
            }
            AssetEvent::Verkauf(_) => self.events.get_mut(&timestamp).unwrap().push_back(event),
        }
    }

    fn process_kapitalmaßnahmen(
        &mut self,
        kapitalmaßnahmen: &[KapitalmassnahmeRow],
    ) -> Result<()> {
        let mut maßnahmen_per_datum_und_asset = BTreeMap::new();
        // Sortiere Kapitalmßnahmen nach Datum und Asset
        for k in kapitalmaßnahmen {
            if k.beschreibung.contains("Dividende") {
                // ignore Maßnahmen bzgl. Dividenden
                log::warn!("Kapitalmaßnahme {} wird ignoriert", k.beschreibung);
                continue;
            }
            let timestamp = convert_date(&k.datum_zeit)?;
            if !maßnahmen_per_datum_und_asset.contains_key(&timestamp) {
                maßnahmen_per_datum_und_asset.insert(timestamp, HashMap::new());
            }
            let (old_symbol, target_symbol) = parse_symbols_von_kapitalmaßnahme(&k.beschreibung)?;
            let symbol = old_symbol.clone();
            let maßnahme: &mut Kapitalmaßnahme = maßnahmen_per_datum_und_asset
                .entry(timestamp)
                .or_default()
                .entry(symbol)
                .or_default();
            if old_symbol == target_symbol {
                maßnahme.altes_symbol = old_symbol.clone();
                maßnahme.alte_menge = -k.menge;
            } else {
                maßnahme.neues_symbol = target_symbol;
                maßnahme.neue_menge = k.menge;
            }
        }

        for (timestamp, map) in maßnahmen_per_datum_und_asset {
            for maßnahme in map.values() {
                self.add(timestamp, AssetEvent::Kapitalmaßnahme((*maßnahme).clone()));
            }
        }

        Ok(())
    }
}

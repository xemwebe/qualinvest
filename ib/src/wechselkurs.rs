use crate::cash::CashFlow;
use crate::date::convert_timestamp_to_date_string;
use crate::error::Result;
use crate::fifo::FifoStore;
use crate::fifo::PurchaseInfo;
use crate::formatting::round2;
use crate::fx::FxRates;

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Serialize, Deserialize)]
pub struct Währungsverkauf {
    datum: String,
    währung: String,
    erlös: f64,
    fx: f64,
    einstandskosten: f64,
}

impl Währungsverkauf {
    fn eur_erlös(&self) -> f64 {
        self.fx * self.erlös
    }

    fn eur_gewinn(&self) -> f64 {
        self.eur_erlös() - self.einstandskosten
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct WährungsVerkäufe {
    verkäufe: Vec<Währungsverkauf>,
}

impl WährungsVerkäufe {
    pub fn parse(
        cash_flows: &[CashFlow],
        fx_rates: &FxRates,
        fifo: &mut FifoStore,
    ) -> Result<Self> {
        let mut verkäufe = WährungsVerkäufe::default();
        for c in cash_flows {
            if c.curr == "EUR" {
                // Keine Währungsgewinne aus EUR-Positionen
                continue;
            }
            let fx = fx_rates.get_fx_rate(c.date, &c.curr)?;
            // Nur Verkäufe sind relevant
            if c.amount >= 0.0 {
                // Käufe in fifo aufnehmen
                fifo.add(&c.curr, c.date, PurchaseInfo::new(c.amount, fx))?;
                continue;
            }
            let einstandskosten = fx * fifo.reduce(&c.curr, c.date, -c.amount)?;
            let verkauf = Währungsverkauf {
                datum: convert_timestamp_to_date_string(c.date)?,
                währung: c.curr.clone(),
                erlös: -fx * c.amount,
                fx,
                einstandskosten,
            };
            verkäufe.add(verkauf);
        }
        Ok(verkäufe)
    }

    fn add(&mut self, verkauf: Währungsverkauf) {
        self.verkäufe.push(verkauf)
    }
}

impl fmt::Display for WährungsVerkäufe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.verkäufe.is_empty() {
            return writeln!(
                f,
                "Es wurden keine Veräußerungsgeschäfte in diesem Zeitraum getätigt.\n"
            );
        }
        let mut sum = 0.0;
        writeln!(
            f,
            r#"#table(
columns: (auto, auto, auto, auto, auto),
align: (left, right, right, right, right),
stroke: 0.5pt,
inset: 8pt,
table.header([*Datum*],[*Währungsbetrag*],[*Erlöso*],[*Einstandskosten*],[*Gewinn*]),"#
        )?;
        for c in &self.verkäufe {
            let eur_gewinn = c.eur_gewinn();
            sum += eur_gewinn;
            writeln!(
                f,
                "[{}],[{:.2} {:3}],[{:.2} EUR],[{:.2} EUR],[{:.2} EUR],",
                c.datum,
                round2(c.erlös),
                c.währung,
                round2(c.eur_erlös()),
                round2(c.einstandskosten),
                round2(eur_gewinn),
            )?;
        }

        writeln!(
            f,
            ")\nGesamtsumme Kapitalerträge in EUR: {:.2}",
            round2(sum)
        )?;
        Ok(())
    }
}

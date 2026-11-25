use crate::cash;
use crate::dividends::{Dividenden, berechne_dividenden};
use crate::error::{Error, Result};
use crate::fifo::FifoStore;
use crate::formatting::round2;
use crate::fx;
use crate::quellensteuer::QuellensteuerPerJurisdiktion;
use crate::read::{self, parse_kontoauszug};
use crate::settings::Settings;
use crate::veraeusserung::{Veräußerungen, berechne_veräußerungsgewinne};
use crate::vorabpauschale::Vorabpauschalen;
use crate::wechselkurs::WährungsVerkäufe;
use serde::{Deserialize, Serialize};
use std::{fmt, fs::File};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Report {
    jahr: u32,
    eur_zinsen: f64,
    aktien_dividenden: Dividenden,
    etf_dividenden: Dividenden,
    aktien_qtax: QuellensteuerPerJurisdiktion,
    etf_qtax: QuellensteuerPerJurisdiktion,
    aktien_veräußerungsgewinne: Veräußerungen,
    etf_veräußerungsgewinne: Veräußerungen,
    wechselkurs_gewinne: WährungsVerkäufe,
    fifo: FifoStore,
    curr_fifo: FifoStore,
    vorabpauschalen: Vorabpauschalen,
}

impl Report {
    pub fn from_file(jahr: u32, settings: &Settings) -> Result<Self> {
        let report_path = settings
            .zwischenergebnisse
            .join(format!("report_{}.json", jahr));
        let report_file = File::open(&report_path)?;
        let report: Report = serde_json::from_reader(report_file)?;
        Ok(report)
    }

    pub fn new(jahr: u32) -> Self {
        Self {
            jahr,
            ..Default::default()
        }
    }

    pub fn init(&mut self, settings: &Settings) -> Result<()> {
        let mut fifo_initialized = false;
        let mut curr_fifo_initialized = false;
        let fx_rates = fx::read_fx_rates(&settings.fx_rates)?;

        // Try initalize with data from last year
        let last_report = Report::from_file(&self.jahr - 1, settings);
        match last_report {
            Ok(report) => {
                self.fifo = report.fifo.clone();
                fifo_initialized = true;
                self.curr_fifo = report.curr_fifo.clone();
                curr_fifo_initialized = true;
            }
            Err(error) => {
                log::warn!("Report vom letzten Jahr nicht gefunden oder nicht lesbar: {error}");
            }
        };
        // Try initialize FIFO with last years FIFO-file
        if !fifo_initialized {
            let fifo_in_path = &settings
                .zwischenergebnisse
                .join(format!("fifo_{}.json", self.jahr - 1));
            if let Ok(fifo_file) = File::open(fifo_in_path) {
                let fifo = serde_json::from_reader::<_, FifoStore>(&fifo_file);
                if let Ok(fifo) = fifo {
                    self.fifo = fifo;
                    fifo_initialized = true;
                    println!(
                        "FIFO-Informationen wurden von Datei '{}' eingelesen",
                        fifo_in_path.display()
                    );
                }
            }
        }
        if !curr_fifo_initialized {
            let curr_fifo_in_path = &settings
                .zwischenergebnisse
                .join(format!("curr_fifo_{}.join", self.jahr - 1));
            if let Ok(curr_fifo_file) = File::open(curr_fifo_in_path) {
                let curr_fifo = serde_json::from_reader::<_, FifoStore>(&curr_fifo_file);
                if let Ok(curr_fifo) = curr_fifo {
                    self.curr_fifo = curr_fifo;
                    curr_fifo_initialized = true;
                    println!(
                        "FIFO-Informationen für Währungspositionen wurden von Datei '{}' eingelesen",
                        curr_fifo_in_path.display()
                    );
                }
            }
        }

        if !fifo_initialized {
            eprintln!(
                "FIFO-Informationen konnten nicht geladen werden, startet mit leerer FIFO-Info"
            )
        }
        if !curr_fifo_initialized {
            eprintln!(
                "FIFO-Informationen für Währungspositionen konnten nicht geladen werden, startet mit leerer FIFO-Info"
            )
        }

        let kontoauszug_pfad = &settings
            .jährliche_daten
            .get(&self.jahr)
            .ok_or(Error::KontoauszugFehlt(self.jahr))?
            .kontoauszug;
        let kontoauszug = parse_kontoauszug(kontoauszug_pfad)?;
        self.eur_zinsen = Self::gesamt_eur_zinsen(&kontoauszug.zinsen)?;
        (self.aktien_dividenden, self.etf_dividenden) =
            berechne_dividenden(&kontoauszug, &fx_rates)?;
        (self.aktien_qtax, self.etf_qtax) = kontoauszug.get_quellensteuer(&fx_rates)?;

        (
            self.aktien_veräußerungsgewinne,
            self.etf_veräußerungsgewinne,
        ) = berechne_veräußerungsgewinne(&kontoauszug, &fx_rates, &mut self.fifo, settings)?;

        let cash_pfad = &settings
            .jährliche_daten
            .get(&self.jahr)
            .ok_or(Error::KontoauszugFehlt(self.jahr))?
            .cashbericht;
        let cfs = cash::read_cash_flows(cash_pfad)?;
        let mut cash_flows = cfs;
        cash_flows.sort_by(|x, y| {
            if x.date == y.date {
                if x.amount < 0.0 {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Less
                }
            } else {
                x.date.cmp(&y.date)
            }
        });
        self.wechselkurs_gewinne =
            WährungsVerkäufe::parse(&cash_flows, &fx_rates, &mut self.curr_fifo)?;

        if let Err(error) = self.berechne_vorabpauschalen_infos(settings, &fx_rates) {
            log::warn!("Vorabpauschalen konnten nicht berechnet werden: {error}");
        }
        Ok(())
    }

    fn berechne_vorabpauschalen_infos(
        &mut self,
        settings: &Settings,
        fx_rates: &crate::fx::FxRates,
    ) -> Result<()> {
        let kontoauszug_anfang_pfad = &settings
            .jährliche_daten
            .get(&(self.jahr - 2))
            .ok_or(Error::KontoauszugFehlt(self.jahr - 22))?
            .kontoauszug;
        let kontoauszug_start = read::parse_kontoauszug(kontoauszug_anfang_pfad)?;
        let kontoauszug_ende_pfad = &settings
            .jährliche_daten
            .get(&(self.jahr - 1))
            .ok_or(Error::KontoauszugFehlt(self.jahr - 1))?
            .kontoauszug;
        let kontoauszug_ende = read::parse_kontoauszug(kontoauszug_ende_pfad)?;
        let basis_rate = settings
            .jährliche_daten
            .get(&(self.jahr - 1))
            .ok_or(Error::BasisRateFehlt(self.jahr))?
            .basiszins;
        self.vorabpauschalen = Vorabpauschalen::sammle_vorabpauschalen_infos(
            &kontoauszug_start,
            &kontoauszug_ende,
            basis_rate,
            fx_rates,
        )?;

        Ok(())
    }

    fn gesamt_eur_zinsen(zinsen: &[read::ZinsRow]) -> Result<f64> {
        for z in zinsen {
            if z.beschreibung == "Gesamt Zinsen in EUR" {
                return Ok(z.betrag);
            }
        }
        Err(Error::GesamtZinsenNichtGefunden)
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "#set page(flipped: true)")?;
        writeln!(f, "= Steuerbericht für das Jahr {}", self.jahr)?;

        writeln!(f, "\n== Zinsen")?;
        writeln!(f, "Realisierte Zinsen: {} EUR", round2(self.eur_zinsen))?;

        writeln!(f, "\n== Erhaltene Dividenden auf Aktien")?;
        writeln!(f, "{}", self.aktien_dividenden)?;

        writeln!(f, "\n== Erhaltene Dividenden auf ETF")?;
        writeln!(f, "{}", self.etf_dividenden)?;

        writeln!(
            f,
            "\n== Abgeführte Quellensteuer auf Aktien\n{}",
            self.aktien_qtax
        )?;

        writeln!(
            f,
            "\n== Abgeführte Quellensteuer auf ETFs\n{}",
            self.etf_qtax
        )?;

        writeln!(
            f,
            "\n== Gewinne aus Veräußerung von Aktien\n{}",
            self.aktien_veräußerungsgewinne
        )?;
        writeln!(
            f,
            "\n== Gewinne aus Veräußerung von ETFs\n{}",
            self.etf_veräußerungsgewinne
        )?;
        writeln!(
            f,
            "\n== Gewinne aus Veräußerung von Fremdwährungen\n{}",
            self.wechselkurs_gewinne
        )?;

        writeln!(
            f,
            "\n== Vorabpauschalen auf ETFs und Fonds\n{}",
            self.vorabpauschalen
        )?;
        Ok(())
    }
}

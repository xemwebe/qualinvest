use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use config::Config;
use std::{error::Error, io::Write, path::PathBuf};

use ib::{
    cash,
    date::{convert_date, convert_timestamp_to_date_string},
    dividends, error, fifo, fx, read,
    read_transactions::{self, BuySell},
    report::Report,
    settings, veraeusserung,
    vorabpauschale::Vorabpauschalen,
};

#[derive(Parser, Debug)]
#[command(name = "gibtax")]
struct Cli {
    /// Pfad für die Konfigurationsdatei
    #[arg(short, long)]
    config_path: Option<String>,

    /// Calculate FIFO information from transaction history
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Erstellen einen vollständigen Report für ein bestimmtes Jahr
    Report(ReportArgs),
    /// Berechne FIFO Liste der Trades Calculate FIFO stack based on trade history
    Fifo(FifoArgs),
    /// Erstelle einen verkürzten Report auf Basis der vorgegbenen Daten
    SimpleReport(SimpleReportArgs),
    /// Erstelle einen Report für Wechselkursgewinne
    CurrReport(CurrReportArgs),
    /// Berechne Vorabpauschale für ETFs
    Vorabpauschale(VorabpauschaleArgs),
}

#[derive(Debug, Args)]
struct ReportArgs {
    #[arg(short, long)]
    /// Jahr, für den der Report erstellt werden soll
    jahr: u32,
}

#[derive(Debug, Args)]
struct FifoArgs {
    /// Pfad zu einer oder mehreren Transaction-History-Dateien (AllTransactions*.csv)
    #[arg(short, long)]
    transactions: Vec<PathBuf>,
    /// Pfad zu einem Kontoauszug, dessen offene Position zur Initialisizerung der FiFo-Historie dienen
    #[arg(short, long)]
    initial_positions: Option<PathBuf>,
    /// Nur Transactions vor diesem Datum werden berücksichtigt
    #[arg(short, long)]
    max_time: String,
    /// Pfad in den der FIFO status geschrieben werden soll
    #[arg(short, long)]
    out_file: PathBuf,
    /// Pfad zu Datei mit den EZB-Referenzwechselkurshistorie
    #[arg(short, long)]
    fx_rates: PathBuf,
}

#[derive(Debug, Args)]
struct SimpleReportArgs {
    /// Pfad zur Kontoauszugsdatei
    #[arg(short, long)]
    konto_auszug: PathBuf,
    /// Pfad zu Datei mit den EZB-Referenzwechselkurshistorie
    #[arg(short, long)]
    fx_rates: PathBuf,
    /// FIFO-Stand als Basis zur Berechnung der Veräußerungsgewinne
    #[arg(short = 'F', long)]
    fifo_state: Option<PathBuf>,
    /// FIFO-Stand als Basis zur Berechnung der Veräußerungsgewinne
    #[arg(short, long)]
    output_fifo_state: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct CurrReportArgs {
    /// Pfad zum CashReport
    #[arg(short, long)]
    cash_report: PathBuf,
    /// Pfad zu Datei mit den EZB-Referenzwechselkurshistorie
    #[arg(short, long)]
    fx_rates: PathBuf,
    /// Währungs-FIFO-Stand als Basis zur Berechnung der Veräußerungsgewinne
    #[arg(short = 'F', long)]
    fifo_state: Option<PathBuf>,
    /// Währungs-FIFO-Stand als Basis zur Berechnung der Veräußerungsgewinne
    #[arg(short, long)]
    output_fifo_state: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct VorabpauschaleArgs {
    /// Jahr, für die die Vorabschlagpauschale berechnet werden soll
    #[arg(short, long)]
    jahr: u32,
    /// Pfad zu Datei mit den EZB-Referenzwechselkurshistorie
    #[arg(short, long)]
    fx_rates: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    simple_logger::init_with_env().context("failed to init logger")?;
    log::info!("Logging has started.");

    let cli = Cli::parse();
    let mut settings = Config::builder();
    if let Some(config_path) = cli.config_path {
        settings = settings.add_source(config::File::with_name(&config_path));
    }
    let settings = settings
        .build()
        .context("Laden der Konfigurationsdatei fehlgeschlagen")?;

    let settings = settings
        .try_deserialize::<settings::Settings>()
        .context("Konfigurationsdatei laden ist fehlgeschlagen")?;

    match cli.command {
        Commands::Report(args) => {
            let mut report = Report::new(args.jahr);
            report.init(&settings)?;
            let report_file_path = settings
                .zwischenergebnisse
                .join(format!("report_{}.typ", args.jahr));
            let mut report_file = std::fs::File::create(&report_file_path)?;
            writeln!(report_file, "{report}")
                .context("Reportdatei schreiben ist fehlgeschlagen")?;
            let report_json_path = settings
                .zwischenergebnisse
                .join(format!("report_{}.json", args.jahr));
            let report_json_file = std::fs::File::create(&report_json_path)?;
            serde_json::to_writer_pretty(report_json_file, &report)?;
        }
        Commands::SimpleReport(args) => {
            println!("Reading {} …\n", args.konto_auszug.display());
            let d = read::parse_kontoauszug(&args.konto_auszug)?;
            let fx_rates = fx::read_fx_rates(&args.fx_rates)?;

            println!("\nSteueraufstellung");
            println!();
            print_total_interest(&d.zinsen);
            println!();
            let (aktien_dividenden, etf_dividenden) =
                dividends::berechne_dividenden(&d, &fx_rates)?;
            println!("Erhaltene Dividenden auf Aktien\n{aktien_dividenden}");
            println!("\nErhaltene Dividenden auf ETF\n{etf_dividenden}");
            println!();
            let (aktien_qtax, etf_qtax) = d.get_quellensteuer(&fx_rates)?;
            println!("Abgeführte Quellensteuer auf Aktien\n{aktien_qtax}");
            println!("Abgeführte Quellensteuer auf ETFs\n{etf_qtax}");
            println!();
            let mut fifo = if let Some(fifo_state) = args.fifo_state {
                let fifo_file = std::fs::File::open(&fifo_state)?;
                let fifo: fifo::FifoStore = serde_json::from_reader(&fifo_file)?;
                fifo
            } else {
                fifo::FifoStore::new(0)
            };
            let (aktien_veräußerungsgewinne, etf_veräußerungsgewinne) =
                veraeusserung::berechne_veräußerungsgewinne(
                    &d,
                    &fx_rates,
                    &mut fifo,
                    &settings::Settings::default(),
                )?;
            println!("Gewinne aus Veräußerung von Aktien\n{aktien_veräußerungsgewinne}");
            println!("Gewinne aus Veräußerung von ETFs\n{etf_veräußerungsgewinne}");
            if let Some(fifo_output) = args.output_fifo_state {
                let out_file = std::fs::File::create(&fifo_output)?;
                serde_json::to_writer_pretty(&out_file, &fifo)?;
            }
        }
        Commands::Fifo(args) => {
            let fx_rates = fx::read_fx_rates(&args.fx_rates)?;
            let mut fifo = if let Some(initial_postions) = args.initial_positions {
                println!("Reading {} …\n", initial_postions.display());
                let d = read::parse_kontoauszug(&initial_postions)?;
                let timestamp = d.get_timestamp()?;
                fifo::FifoStore::from_open_positions(&d.offene_positionen, timestamp, &fx_rates)?
            } else {
                fifo::FifoStore::new(0)
            };
            let mut fifo_transactions = Vec::new();
            for path in args.transactions {
                println!("Reading transaction history {} …", path.display());
                let th = read_transactions::parse_transaction_history(&path)?;
                let mut transactions = th.extract_purchase_infos(&fx_rates)?;
                fifo_transactions.append(&mut transactions);
            }
            fifo_transactions.sort();
            println!("fifo_transactions: {fifo_transactions:?}");
            let max_time_stamp = convert_date(&args.max_time)?;
            for transaction in fifo_transactions {
                println!("process transaction: {transaction:?}");
                if transaction.get_timestamp() >= fifo.get_timestamp()
                    && transaction.get_timestamp() < max_time_stamp
                {
                    match transaction.buy_sell {
                        BuySell::Buy => {
                            fifo.add(
                                &transaction.symbol,
                                transaction.timestamp,
                                fifo::PurchaseInfo::new(transaction.quantity, transaction.price),
                            )?;
                        }
                        BuySell::Sell => {
                            let _ = fifo.reduce(
                                &transaction.symbol,
                                transaction.timestamp,
                                transaction.quantity,
                            )?;
                        }
                    }
                }
            }
            let out_file = std::fs::File::create(&args.out_file)?;
            serde_json::to_writer_pretty(out_file, &fifo)?;
        }
        Commands::CurrReport(args) => {
            let path = &args.cash_report;
            println!("Reading cash report {} …", path.display());
            let cfs = cash::read_cash_flows(path)?;
            let fx_rates = fx::read_fx_rates(&args.fx_rates)?;
            let mut fifo = if let Some(fifo_state) = args.fifo_state {
                let fifo_file = std::fs::File::open(&fifo_state)?;
                let fifo: fifo::FifoStore = serde_json::from_reader(&fifo_file)?;
                fifo
            } else {
                fifo::FifoStore::new(0)
            };
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
            print_währungs_verkäufe(&cash_flows, &fx_rates, &mut fifo)?;
            if let Some(fifo_output) = args.output_fifo_state {
                let out_file = std::fs::File::create(&fifo_output)?;
                serde_json::to_writer_pretty(&out_file, &fifo)?;
            }
        }
        Commands::Vorabpauschale(args) => {
            let fx_rates = fx::read_fx_rates(&args.fx_rates)?;
            let kontoauszug_anfang_pfad = &settings
                .jährliche_daten
                .get(&(args.jahr - 2))
                .ok_or(error::Error::KontoauszugFehlt(args.jahr - 22))?
                .kontoauszug;
            let kontoauszug_start = read::parse_kontoauszug(kontoauszug_anfang_pfad)?;
            let kontoauszug_ende_pfad = &settings
                .jährliche_daten
                .get(&(args.jahr - 1))
                .ok_or(error::Error::KontoauszugFehlt(args.jahr - 1))?
                .kontoauszug;
            let kontoauszug_ende = read::parse_kontoauszug(kontoauszug_ende_pfad)?;
            let basis_rate = settings
                .jährliche_daten
                .get(&(args.jahr + 1))
                .ok_or(error::Error::BasisRateFehlt(args.jahr))?
                .basiszins;
            let vorabpauschalen = Vorabpauschalen::sammle_vorabpauschalen_infos(
                &kontoauszug_start,
                &kontoauszug_ende,
                basis_rate,
                &fx_rates,
            )?;
            println!("Vorabpauschalen:\n{vorabpauschalen}");
        }
    }

    Ok(())
}

fn print_währungs_verkäufe(
    cash_flows: &[cash::CashFlow],
    fx_rates: &fx::FxRates,
    fifo: &mut fifo::FifoStore,
) -> Result<()> {
    println!("Gewinne und Verluste aus Währungsverkäufen");
    let mut sum = 0.0;
    for c in cash_flows {
        if c.curr == "EUR" {
            // Keine Währungsgewinne aus EUR-Positionen
            continue;
        }
        let fx = fx_rates.get_fx_rate(c.date, &c.curr)?;
        // Nur Verkäufe sind relevant
        if c.amount >= 0.0 {
            // Käufe in fifo aufnehmen
            fifo.add(&c.curr, c.date, fifo::PurchaseInfo::new(c.amount, fx))?;
            continue;
        }
        let purchase_cost = fifo.reduce(&c.curr, c.date, -c.amount)?;
        let eur_betrag = fx * c.amount + purchase_cost;
        sum += eur_betrag;
        println!(
            "Verkauf am {} von {:8.2} {} ({:8.2} EUR) mit Einstand {:8.2} EUR und real. GuV {:8.2} EUR",
            convert_timestamp_to_date_string(c.date)?,
            -c.amount,
            c.curr,
            fx * (-c.amount),
            purchase_cost,
            eur_betrag,
        )
    }

    println!("Gesamtsumme Kapitalerträge in EUR: {}", sum);
    Ok(())
}

fn print_total_interest(zinsen: &[read::ZinsRow]) {
    for z in zinsen {
        if z.beschreibung == "Gesamt Zinsen in EUR" {
            println!("Realisierte Zinsen: {}", z.betrag);
            return;
        }
    }
    println!("#Fehler#: Gesamt Zinsen in EUR nicht gefunden");
}

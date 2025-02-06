//!
//! # qualinvest_cli
//!
//! This command line tool is part of qualinvest, a collection of tools for quantitative investment analysis.
//! For more information, see [qualinvest on github](https://github.com/xemwebe/qualinvest)
//!

use std::fs;
use std::io::{stdout, BufReader, Write};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use chrono::Local;
use clap::{Args, Parser, Subcommand};
use glob::glob;
use log::info;

use finql::datatypes::{
    date_time_helper::date_time_from_str_standard, Currency, QuoteHandler, Ticker,
    TransactionHandler,
};
use finql::postgres::PostgresDB;
use finql::{portfolio::calc_position, Market};

use qualinvest_core::{
    accounts::AccountHandler,
    performance::calc_performance,
    read_pdf::{parse_and_store, sha256_hash},
    setup_market, Config,
};

pub mod plot;

#[derive(Parser)]
#[clap(
    name = "qualinvest",
    author = "Mark Beinker <mwb@quantlink.de>",
    version = "0.3.0",
    about = "Tools for quantitative analysis and management of financial asset portfolios"
)]
struct Cli {
    /// Sets a custom config file
    #[arg(short, long, value_name = "file")]
    config: Option<String>,
    /// Set config file format to JSON (default is TOML)
    #[arg(short = 'J', long)]
    json_config: bool,
    /// Prints additional information for debugging purposes
    #[arg(short, long)]
    debug: bool,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
#[command()]
enum Command {
    /// Calculate SHA256 hash sum of given file
    Hash(Hash),
    /// Clear all data in database
    CleanDb,
    Parse(Parse),
    Position(Position),
    Update(Update),
    Insert(Insert),
    FillGaps(FillGaps),
    Performance(Performance),
    PdfUpload(PdfUpload),
}

#[derive(Args)]
struct Hash {
    /// Input file of which to calculate hash from
    #[arg(required = true, index = 1)]
    input: PathBuf,
}

#[derive(Args)]
struct Parse {
    /// Path of pdf file or directory
    #[arg(required = true, index = 1)]
    path: String,
    /// Parse all files in the given directory
    #[arg(short, long)]
    directory: bool,
    /// Print warning if pdf file has already been parsed, otherwise ignore silently
    #[arg(long)]
    warn_old: bool,
    /// Process in spite of failed consistency check, but add note
    #[arg(long)]
    ignore_consistency_check: bool,
    /// In case of duplicate asset names with different ISIN or WKN, rename asset by appending ' (NEW)'
    #[arg(long)]
    rename_asset: bool,
    /// Specify (existing) account id to which transactions should be assigned if no account details could not be found
    #[arg(long)]
    default_account: Option<i32>,
}

/// Calculate the position per asset
#[derive(Args)]
struct Position {
    /// Display output in JSON format (default is csv)
    #[arg(short, long)]
    json: bool,
    /// Calculate position for the given account only
    #[arg(short, long)]
    account: Option<i32>,
    /// Include fields for latest quotes
    #[arg(short, long)]
    quote: bool,
}

/// Update quotes for all tickers
#[derive(Args)]
struct Update {
    /// Update only the given ticker id
    #[arg(short, long)]
    ticker_id: Option<i32>,
    /// Update history of quotes, only allowed in single ticker mode
    #[arg(short, long)]
    history: bool,
    /// Start of history to be updated
    #[arg(short, long)]
    start: Option<String>,
    /// End of history to be updated
    #[arg(short, long)]
    end: Option<String>,
}

/// Insert object into database
#[derive(Args)]
struct Insert {
    /// Ticker to be inserted as string in JSON format
    #[arg(required = true, index = 1)]
    json_object: String,
}

/// Find gaps in quotes time series' and try to fill them
#[derive(Args)]
struct FillGaps {
    /// Ignore gaps with lass than 'min_size' days.
    #[arg(short, long)]
    min_size: Option<String>,
}

/// Calculate total performance of set of transactions
#[derive(Args)]
struct Performance {
    /// Account id for performance graph
    #[arg(short, long)]
    account: i32,
    /// Start date for performance calculation (default 2000-01-01)
    #[arg(short, long)]
    start: Option<String>,
    /// End date for performance calculation (default today)
    #[arg(short, long)]
    end: Option<String>,
    /// Base currency for performance calculation
    #[arg(short, long)]
    currency: Option<String>,
    /// Output file
    #[arg(short, long)]
    output: Option<String>,
}

/// Upload missing pdf to database
#[derive(Args)]
struct PdfUpload {
    /// Source directory, if missing use the standard pdf file directory configured in config file
    #[arg(short, long)]
    source: Option<String>,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    info!("Entering main function.");

    let args = Cli::parse();
    let config = args.config.unwrap_or("qualinvest.toml".to_string());

    let mut config: Config = if args.json_config {
        let config_file = fs::File::open(config).unwrap();
        let config_reader = BufReader::new(config_file);
        serde_json::from_reader(config_reader).unwrap()
    } else {
        let config_file = fs::read_to_string(config).unwrap();
        toml::from_str(&config_file).unwrap()
    };

    let db = PostgresDB::new(&config.db.url).await.unwrap();

    if args.debug {
        config.debug = true;
    }

    let db = Arc::new(db);
    let market = Market::new(db.clone()).await;

    match args.command {
        Command::Hash(args) => {
            let pdf_file = args.input;
            match sha256_hash(&pdf_file) {
                Err(err) => {
                    println!(
                        "Failed to calculate hash of file {:?} with error {:?}",
                        pdf_file, err
                    );
                }
                Ok(hash) => {
                    println!("Hash is {}.", hash);
                }
            }
        }
        Command::CleanDb => {
            print!("Cleaning database...");
            db.clean_accounts().await.unwrap();
            db.clean().await.unwrap();
            db.init_accounts().await.unwrap();
            println!("done");
        }
        Command::Parse(args) => {
            let path = args.path;
            if args.directory {
                // Parse complete directory
                let pattern = format!("{}/*.pdf", path);
                let mut count_transactions = 0_i32;
                let mut count_docs = 0_i32;
                let mut count_failed = 0_i32;
                let mut count_skipped = 0_i32;
                for file in glob(&pattern).expect("Failed to read directory") {
                    count_docs += 1;
                    let path = file.unwrap();
                    let file_name = path.file_name().unwrap().to_str().unwrap();
                    let transactions =
                        parse_and_store(&path, file_name, db.clone(), &config.pdf, &market).await;
                    match transactions {
                        Err(err) => {
                            count_failed += 1;
                            println!("Failed to parse file {} with error {:?}", file_name, err);
                        }
                        Ok(count) => {
                            if count == 0 {
                                count_skipped += 1;
                            }
                            count_transactions += count;
                        }
                    }
                }
                println!("{} documents found, {} skipped, {} failed, {} parsed successfully, {} transaction(s) stored in database.",
                    count_docs, count_skipped, count_failed, count_docs-count_skipped-count_failed, count_transactions);
            } else {
                // parse single file
                let path = std::path::Path::new(&path);
                let file_name = path.file_name().unwrap().to_str().unwrap();
                let transactions = parse_and_store(path, file_name, db, &config.pdf, &market).await;
                match transactions {
                    Err(err) => {
                        println!(
                            "Failed to parse file {} with error {:?}",
                            path.as_os_str().to_string_lossy(),
                            err
                        );
                    }
                    Ok(count) => {
                        println!("{} transaction(s) stored in database.", count);
                    }
                }
            }
        }
        Command::Position(args) => {
            let currency = Currency::from_str("EUR").unwrap();
            let account_id = args.account;
            let transactions = match account_id {
                Some(account_id) => db
                    .get_all_transactions_with_account(account_id)
                    .await
                    .unwrap(),
                None => db.get_all_transactions().await.unwrap(),
            };
            let market = Market::new(db).await;
            let mut position = calc_position(currency, &transactions, None, market.clone())
                .await
                .unwrap();
            position
                .get_asset_names(market.db().into_arc_dispatch())
                .await
                .unwrap();

            if args.quote {
                let time = Local::now();
                position.add_quote(time, &market).await;
            }

            if args.json {
                println!("{}", serde_json::to_string(&position).unwrap());
            } else {
                let mut wtr = csv::Writer::from_writer(stdout());
                wtr.serialize(position.cash).unwrap();
                for (_, pos) in position.assets {
                    wtr.serialize(pos).unwrap();
                }
                wtr.flush().unwrap();
            }
        }
        Command::Update(args) => {
            if args.history {
                let ticker_id = args
                    .ticker_id
                    .expect("Ticker id must be given for history update.");
                let end = if args.end.is_some() {
                    date_time_from_str_standard(&args.end.unwrap(), 18, None).unwrap()
                } else {
                    Local::now()
                };
                let start = if args.start.is_some() {
                    date_time_from_str_standard(&args.start.unwrap(), 9, None).unwrap()
                } else {
                    date_time_from_str_standard("2014-01-01", 9, None).unwrap()
                };
                market
                    .update_quote_history(ticker_id, start, end)
                    .await
                    .unwrap();
            } else if let Some(ticker_id) = args.ticker_id {
                market.update_quote_for_ticker(ticker_id).await.unwrap();
            } else {
                let market = setup_market(db.clone(), &config.market_data).await;
                let failed_ticker = market.update_quotes().await.unwrap();
                if !failed_ticker.is_empty() {
                    println!("Some ticker could not be updated: {:?}", failed_ticker);
                }
            }
        }
        Command::Insert(args) => {
            let ticker = args.json_object;
            let ticker: Ticker = serde_json::from_str(&ticker).unwrap();
            db.insert_ticker(&ticker).await.unwrap();
        }
        Command::FillGaps(args) => {
            let min_size = if let Some(size) = args.min_size {
                usize::from_str(&size).unwrap()
            } else {
                1
            };
            let mut market = setup_market(db.clone(), &config.market_data).await;
            qualinvest_core::fill_quote_gaps(&mut market, min_size)
                .await
                .unwrap();
        }
        Command::Performance(args) => {
            let account_id = args.account;

            let start_date = if let Some(start) = args.start {
                date_time_from_str_standard(&start, 9, None)
                    .unwrap()
                    .naive_local()
                    .date()
            } else {
                date_time_from_str_standard("2000-01-01", 9, None)
                    .unwrap()
                    .naive_local()
                    .date()
            };
            let end_date = if let Some(end) = args.end {
                date_time_from_str_standard(&end, 9, None)
                    .unwrap()
                    .naive_local()
                    .date()
            } else {
                Local::now().naive_local().date()
            };
            let market = Market::new_with_date_range(db.clone(), start_date, end_date)
                .await
                .unwrap();
            let currency = if let Some(currency) = args.currency {
                market
                    .get_currency_from_str(&currency)
                    .await
                    .expect("Currency not found")
            } else {
                market
                    .get_currency_from_str("EUR")
                    .await
                    .expect("Currency not found")
            };
            let file_name = if args.output.is_some() {
                args.output.unwrap().to_string()
            } else {
                "total_performance.json".to_string()
            };

            let transactions = db
                .get_all_transactions_with_account_before(account_id, end_date)
                .await
                .unwrap();

            let total_performance = calc_performance(
                currency,
                &transactions,
                start_date,
                end_date,
                &market,
                "TARGET",
            )
            .await
            .unwrap();
            let mut file = fs::File::create(file_name).unwrap();
            write!(file, "{:?}", total_performance).unwrap();
        }
        Command::PdfUpload(args) => {
            let source = if let Some(source) = args.source {
                source
            } else {
                config.pdf.doc_path.clone()
            };
            let source_dir = std::path::Path::new(&source);
            let missing_pdfs = db.get_missing_pdfs().await.unwrap();
            for file in missing_pdfs {
                let file_path = source_dir.join(file.2);
                if file_path.is_file() {
                    let hash = sha256_hash(&file_path).unwrap();
                    if hash != file.1 {
                        eprintln!("Skipping file {}, hash differs!", file_path.display());
                    }
                    let buffer = std::fs::read(&file_path).unwrap();
                    db.store_pdf(file.0, &buffer).await.unwrap();
                } else {
                    eprintln!("file not found: {}", file_path.display());
                }
            }
        }
    }
}

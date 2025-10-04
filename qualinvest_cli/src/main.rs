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

use clap::{Args, Parser, Subcommand};
use log::info;
use time::OffsetDateTime;

use finql::datatypes::{
    date_time_helper::offset_date_time_from_str_standard, Currency, QuoteHandler, Ticker,
    TransactionHandler,
};
use finql::postgres::PostgresDB;
use finql::{portfolio::calc_position, Market};

use qualinvest_core::{
    accounts::AccountHandler, performance::calc_performance, setup_market, Config,
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
    /// Clear all data in database
    CleanDb,
    Position(Position),
    Update(Update),
    Insert(Insert),
    FillGaps(FillGaps),
    Performance(Performance),
}

#[derive(Args)]
struct Hash {
    /// Input file of which to calculate hash from
    #[arg(required = true, index = 1)]
    input: PathBuf,
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
        Command::CleanDb => {
            print!("Cleaning database...");
            db.clean_accounts().await.unwrap();
            db.clean().await.unwrap();
            db.init_accounts().await.unwrap();
            println!("done");
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
                let time =
                    OffsetDateTime::now_local().expect("Indeterminate local time zone offset");
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
                    offset_date_time_from_str_standard(&args.end.unwrap(), 18, None).unwrap()
                } else {
                    OffsetDateTime::now_local().expect("Indeterminate local time zone offset")
                };
                let start = if args.start.is_some() {
                    offset_date_time_from_str_standard(&args.start.unwrap(), 9, None).unwrap()
                } else {
                    offset_date_time_from_str_standard("2014-01-01", 9, None).unwrap()
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
                offset_date_time_from_str_standard(&start, 9, None)
                    .unwrap()
                    .date()
            } else {
                offset_date_time_from_str_standard("2000-01-01", 9, None)
                    .unwrap()
                    .date()
            };
            let end_date = if let Some(end) = args.end {
                offset_date_time_from_str_standard(&end, 9, None)
                    .unwrap()
                    .date()
            } else {
                OffsetDateTime::now_local()
                    .expect("Indeterminate local time zone offset")
                    .date()
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
    }
}

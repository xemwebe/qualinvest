///! # qualinvest_cli
///! 
///! This library is part of a set of tools for quantitative investments.
///! For mor information, see [qualinvest on github](https://github.com/xemwebe/qualinvest)
///!

use std::fs;
use std::io::{stdout, BufReader};
use std::str::FromStr;
use std::sync::Arc;

use chrono::{DateTime, Local, Utc};
use glob::glob;
use clap::{App, AppSettings, Arg, SubCommand};

use finql_data::{Ticker, Currency, QuoteHandler, TransactionHandler, date_time_helper::date_time_from_str_standard};
use finql_postgres::PostgresDB;
use finql::{Market, portfolio::calc_position};

use qualinvest_core::accounts::AccountHandler;
use qualinvest_core::read_pdf::{parse_and_store, sha256_hash};
use qualinvest_core::Config;

#[tokio::main]
async fn main() {
    let matches = App::new("qualinvest")
        .setting(AppSettings::ArgRequiredElseHelp)
        .setting(AppSettings::ColoredHelp)
        .version("0.3.0")
        .author("Mark Beinker <mwb@quantlink.de>")
        .about("Tools for quantitative analysis and management of financial asset portfolios")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("file")
                .help("Sets a custom config file")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("json-config")
                .short("J")
                .long("json-config")
                .help("Set config file format ot JSON (default is TOML)"),
        )
        .arg(
            Arg::with_name("debug")
                .short("d")
                .long("debug")
                .help("Prints additional information for debugging purposes")
        )
        .subcommand(
            SubCommand::with_name("hash")
                .about("Calculate SHA256 hash sum of given file")
                .setting(AppSettings::ColoredHelp)
                .arg(Arg::with_name("INPUT")
                    .help("Input file of which to calculate hash from")
                    .required(true)
                    .index(1))
        )
        .subcommand(
            SubCommand::with_name("clean-db")
                .about("Clears all data in database. Use with care!")
                .setting(AppSettings::ColoredHelp)
            )
        .subcommand(
            SubCommand::with_name("parse")
                .about("Parse one or more pdf files with transaction informations and insert assets/transactions into database")
                .setting(AppSettings::ColoredHelp)
                .arg(
                Arg::with_name("PATH")
                    .help("Path of pdf file or directory")
                    .required(true)
                    .index(1)
            )
            .arg(
                Arg::with_name("directory")
                    .short("D")
                    .long("directory")
                    .help("Parse all files in the given directory")
            )
            .arg(
                Arg::with_name("warn-old")
                    .long("warn-if-old")
                    .help("Print warning if pdf file has already been parsed, otherwise ignore silently")
                    .takes_value(false),
            )
            .arg(
                Arg::with_name("ignore-consistency-check")
                    .long("ignore-consistency-check")
                    .help("Process in spite of failed consistency check, but add note")
                    .takes_value(false),
            )
            .arg(
                Arg::with_name("rename-asset")
                    .long("rename-asset")
                    .help("In case of duplicate asset names with different ISIN or WKN, rename asset by appending ' (NEW)'")
                    .takes_value(false),
            )
            .arg(
                Arg::with_name("default-account")
                    .long("default-account")
                    .help("Specify (existing) account id to which transactions should be assigned if no account details could not be found")
                    .takes_value(true),
            ))
        .subcommand(
            SubCommand::with_name("position")
                .about("Calculate the position per asset")
                .setting(AppSettings::ColoredHelp)
                .arg(
                    Arg::with_name("json")
                        .long("json")
                        .short("j")
                        .help("Display output in JSON format (default is csv)")
                )
                .arg(
                    Arg::with_name("account")
                        .long("account")
                        .short("a")
                        .help("Calculate position for the given account only")
                        .takes_value(true)
                )
                .arg(
                    Arg::with_name("quote")
                        .long("quote")
                        .short("q")
                        .help("Include fields for latest quotes")
                )
            )
        .subcommand(
            SubCommand::with_name("update")
                .about("Update all active ticker to most recent quote")
                .setting(AppSettings::ColoredHelp)
                .arg(
                    Arg::with_name("ticker-id")
                        .long("ticker-id")
                        .short("t")
                        .help("Update only the given ticker id")
                        .takes_value(true)
                )
                .arg(
                    Arg::with_name("history")
                        .long("history")
                        .short("h")
                        .help("Update history of quotes, only allowed in single ticker mode")
                )
                .arg(
                    Arg::with_name("start")
                        .long("start")
                        .short("s")
                        .help("Start of history to be updated")
                        .takes_value(true)
                )
                .arg(
                    Arg::with_name("end")
                        .long("end")
                        .short("e")
                        .help("End of history to be updated")
                        .takes_value(true)
                )
            )
        .subcommand(
            SubCommand::with_name("insert")
                .about("Insert object into database")
                .setting(AppSettings::ColoredHelp)
                .subcommand(
                    SubCommand::with_name("ticker")
                        .about("Insert ticker into database")
                        .setting(AppSettings::ColoredHelp)
                        .arg(Arg::with_name("JSON-OBJECT")
                            .help("ticker to be inserted as string in JSON format")
                            .required(true)
                            .index(1)
                        )
                    )
            )
        .get_matches();

    let config = matches.value_of("config").unwrap_or("qualinvest.toml");

    let mut config: Config = match matches.is_present("json-config") {
        true => {
            let config_file = fs::File::open(config).unwrap();
            let config_reader = BufReader::new(config_file);
            serde_json::from_reader(config_reader).unwrap()
        }
        false => {
            let config_file = fs::read_to_string(config).unwrap();
            toml::from_str(&config_file).unwrap()
        }
    };

    let postgres_url = format!(
        "postgresql:///{db_name}?user={user}&password={password}&sslmode=disable",
        db_name=config.db.name, user=config.db.user, password=config.db.password
    );
    let db = PostgresDB::new(&postgres_url).await.unwrap();

    if matches.is_present("debug") {
        config.debug = true;
    }

    // Handling commands
    if matches.subcommand_matches("clean-db").is_some() {
        print!("Cleaning database...");
        db.clean_accounts().await.unwrap();
        db.clean().await.unwrap();
        db.init_accounts().await.unwrap();
        println!("done");
        return;
    }

    let db = Arc::new(db);

    if let Some(matches) = matches.subcommand_matches("hash") {
        let pdf_file = std::path::Path::new(matches.value_of("INPUT").unwrap());
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
        return;
    }

    if let Some(matches) = matches.subcommand_matches("parse") {
        // Handle flags for parsing
        if matches.is_present("warn-old") {
            config.pdf.warn_old = true;
        }
        config.pdf.default_account = match matches.value_of("default-account") {
            Some(s) => {
                let num = s.parse::<usize>();
                if num.is_err() {
                    println!("Default account id could not be read");
                }
                Some(num.unwrap())
            },
            None => None,
        };
        if matches.is_present("ignore-consistency-check") {
            config.pdf.consistency_check = false;
        }
        if matches.is_present("rename-asset") {
            config.pdf.rename_asset = true;
        }

        let path = matches.value_of("PATH").unwrap();

        if matches.is_present("directory") {
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
                let transactions = parse_and_store(&path, file_name, db.clone(), &config.pdf).await;
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
            let pdf_file = matches.value_of("parse-pdf").unwrap();
            let path = std::path::Path::new(pdf_file);
            let file_name = path.file_name().unwrap().to_str().unwrap();
            let transactions = parse_and_store(&path, file_name, db, &config.pdf).await;
            match transactions {
                Err(err) => {
                    println!("Failed to parse file {} with error {:?}", pdf_file, err);
                }
                Ok(count) => {
                    println!("{} transaction(s) stored in database.", count);
                }
            }
        }

        return;
    }

    if let Some(matches) = matches.subcommand_matches("position") {
        let currency = Currency::from_str("EUR").unwrap();
        let account_id = matches.value_of("account");
        let transactions = match account_id {
            Some(account_id) => db
                .get_all_transactions_with_account(usize::from_str(&account_id).unwrap())
                .await.unwrap(),
            None => db.get_all_transactions().await.unwrap(),
        };
        let mut position = calc_position(currency, &transactions, None).unwrap();
        position.get_asset_names(db.clone()).await.unwrap();
        
        if matches.is_present("quote") {
            let time = DateTime::from(Local::now());
            let market = Market::new(db);
            position.add_quote(time, &market).await;
        }

        if matches.is_present("json") {
            println!("{}", serde_json::to_string(&position).unwrap());
        } else {
            let mut wtr = csv::Writer::from_writer(stdout());
            wtr.serialize(position.cash).unwrap();
            for (_, pos) in position.assets {
                wtr.serialize(pos).unwrap();
            }
            wtr.flush().unwrap();
        }
        return;
    }

    if let Some(matches) = matches.subcommand_matches("update") {
        if matches.is_present("history") {
            let ticker_id = usize::from_str(matches.value_of("ticker-id").unwrap()).unwrap();
            let end = if matches.is_present("end") {
                date_time_from_str_standard(matches.value_of("end").unwrap(), 18).unwrap()
            } else {
                Utc::now()
            };
            let start = if matches.is_present("start") {
                date_time_from_str_standard(matches.value_of("start").unwrap(), 9).unwrap()
            } else {
                date_time_from_str_standard("2014-01-01", 9).unwrap()
            };
            qualinvest_core::update_quote_history(ticker_id, start, end, db, &config.market_data)
                .await.unwrap();
        } else if matches.is_present("ticker-id") {
            let ticker_id = usize::from_str(matches.value_of("ticker-id").unwrap()).unwrap();
            qualinvest_core::update_ticker(ticker_id, db.clone(), &config.market_data).await.unwrap();
        } else {
            let failed_ticker = qualinvest_core::update_quotes(db, &config.market_data).await.unwrap();
            if !failed_ticker.is_empty() {
                println!("Some ticker could not be updated: {:?}", failed_ticker);
            }
        }
        return;
    }

    if let Some(matches) = matches.subcommand_matches("insert") {
        if let Some(matches) = matches.subcommand_matches("ticker") {
            let ticker = matches.value_of("JSON-OBJECT").unwrap();
            let ticker: Ticker = serde_json::from_str(&ticker).unwrap();
            db.insert_ticker(&ticker).await.unwrap();
        } else {
            println!("Nothing inserted, unknown object type, use `help insert` to display all supported types.");
        }
        return;
    }
}

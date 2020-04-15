///! # qualinvest
///! A cloud based tool for quantitative analysis and management of financial asset portfolios
use clap::{App, AppSettings, Arg, SubCommand};
use finql::postgres_handler::PostgresDB;
use glob::glob;
use std::fs::File;
use std::io::{stdout, BufReader, Write};

use qualinvest_core::accounts::AccountHandler;
use qualinvest_core::read_pdf::{parse_and_store, sha256_hash};
use qualinvest_core::Config;

fn main() {
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
            Arg::with_name("debug")
                .short("d")
                .long("debug")
                .help("Prints additional information for debugging purposes")
                .takes_value(false),
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
                    .help("Path of pdf file or directoy")
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
                    .help("If account details could not be found, use the account 'unassigned'")
                    .takes_value(false),
            )
        ).get_matches();

    let config = matches.value_of("config").unwrap_or("qualinvest.json");
    let config_file = File::open(config).unwrap();
    let config_reader = BufReader::new(config_file);
    let mut config: Config = serde_json::from_reader(config_reader).unwrap();

    let connect_str = format!(
        "host={} user={} password={} dbname={} sslmode=disable",
        config.db_host, config.db_user, config.db_password, config.db_name
    );
    let mut db = PostgresDB::connect(&connect_str).unwrap();

    if matches.is_present("debug") {
        config.debug = true;
    }

    // Handling commands
    if matches.subcommand_matches("clean-db").is_some() {
        print!("Cleaning database...");
        stdout().flush().unwrap();
        db.clean_accounts().unwrap();
        db.clean().unwrap();
        db.init_accounts().unwrap();
        println!("done");
        return;
    }

    if let Some(matches) = matches.subcommand_matches("hash") {
        let pdf_file = matches.value_of("INPUT").unwrap();
        match sha256_hash(&pdf_file) {
            Err(err) => {
                println!(
                    "Failed to calculate hash of file {} with error {:?}",
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
            config.warn_old = true;
        }
        if matches.is_present("default-account") {
            config.default_account = true;
        }
        if matches.is_present("ignore-consistency-check") {
            config.consistency_check = false;
        }
        if matches.is_present("rename-asset") {
            config.rename_asset = true;
        }

        let path = matches.value_of("PATH").unwrap();

        if matches.is_present("directory") {
            // Parse complete directory
            let pattern = format!("{}/*.pdf", path);
            let mut count_transactions = 0;
            let mut count_docs = 0;
            let mut count_failed = 0;
            let mut count_skipped = 0;
            for file in glob(&pattern).expect("Failed to read directory") {
                count_docs += 1;
                let filename = file.unwrap().to_str().unwrap().to_owned();
                let transactions = parse_and_store(&filename, &mut db, &config);
                match transactions {
                    Err(err) => {
                        count_failed += 1;
                        println!("Failed to parse file {} with error {:?}", filename, err);
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
            let transactions = parse_and_store(&pdf_file, &mut db, &config);
            match transactions {
                Err(err) => {
                    println!("Failed to parse file {} with error {:?}", pdf_file, err);
                }
                Ok(count) => {
                    println!("{} transaction(s) stored in database.", count);
                }
            }
        }
    }
}
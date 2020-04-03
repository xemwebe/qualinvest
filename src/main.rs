///! # qualinvest
///! A cloud based tool for quantitative analysis and management of financial asset portfolios
use clap::{App, Arg};
use finql::postgres_handler::PostgresDB;
use glob::glob;
use read_pdf::parse_and_store;
use serde::Deserialize;
use std::fs::File;
use std::io::{stdout, BufReader, Write};

#[macro_use]
extern crate lazy_static;

pub mod accounts;
pub mod read_pdf;
use accounts::AccountHandler;

/// Configuration parameters
#[derive(Debug, Deserialize)]
pub struct Config {
    db_host: String,
    db_name: String,
    db_user: String,
    db_password: String,
    debug: bool,
    doc_path: String,
    warn_old: bool,
    consistency_check: bool,
    rename_asset: bool,
    default_account: bool,
}

fn main() {
    let matches = App::new("qualinvest")
        .version("0.1.0")
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
            Arg::with_name("parse-pdf")
                .short("p")
                .long("parse-pdf")
                .value_name("pdf-file")
                .help("Parses a pdf file and insert assets/transactions into database")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("pdf-dir")
                .short("P")
                .long("pdf-directory")
                .value_name("pdf-dir")
                .help("Parses all pdf files from a given directory and insert assets/transactions into database")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("clean-db")
                .short("C")
                .long("clean-db")
                .help("Clears all data in database before doing anything else, use with care!")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("warn-old")
                .long("warn-if-old")
                .help("Print warning if pdf file has already been parsed, otherwise ignore silently")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("debug")
                .short("d")
                .long("debug")
                .help("Prints additional information for debugging purposes")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("hash")
                .short("h")
                .long("hash")
                .help("Calculate SHA256 hash sum of given file")
                .takes_value(true),
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
        .get_matches();

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
    if matches.is_present("clean-db") {
        print!("Cleaning database...");
        stdout().flush().unwrap();
        db.clean_accounts().unwrap();
        db.clean().unwrap();
        db.init_accounts().unwrap();
        println!("done");
    }
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
    if matches.is_present("hash") {
        let pdf_file = matches.value_of("hash").unwrap();
        match read_pdf::pdf_store::sha256_hash(&pdf_file) {
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
    }
    if matches.is_present("parse-pdf") {
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
    if matches.is_present("pdf-dir") {
        let pdf_dir = matches.value_of("pdf-dir").unwrap();
        let pattern = format!("{}/*.pdf", pdf_dir);
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
                    if count == 0 { count_skipped += 1; }
                    count_transactions += count;
                }
            }
        }
        println!("{} documents found, {} skipped, {} failed, {} parsed successfully, {} transaction(s) stored in database.", 
            count_docs, count_skipped, count_failed, count_docs-count_skipped-count_failed, count_transactions);
    }
}

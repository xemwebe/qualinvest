///! # qualinvest
///! A cloud based tool for quantitative analysis and management of financial asset portfolios
use clap::{App, Arg};
use finql::postgres_handler::PostgresDB;
use read_pdf::parse_and_store;
use serde::Deserialize;
use std::fs::File;
use std::io::{stdout, BufReader, Write};
use glob::glob;

#[macro_use]
extern crate lazy_static;

pub mod read_pdf;

/// Configuration parameters
#[derive(Debug, Deserialize)]
struct Config {
    db_host: String,
    db_name: String,
    db_user: String,
    db_password: String,
}

fn main() {
    let matches = App::new("qualinvest")
        .version("0.1.0")
        .author("Mark Beinker <mwb@quantlink.de>")
        .about("Tools for quantitative analysis and management of financtial asset portfolios")
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
        .get_matches();

    let config = matches.value_of("config").unwrap_or("qualinvest.json");
    let config_file = File::open(config).unwrap();
    let config_reader = BufReader::new(config_file);
    let config: Config = serde_json::from_reader(config_reader).unwrap();

    let connect_str = format!(
        "host={} user={} password={} dbname={} sslmode=disable",
        config.db_host, config.db_user, config.db_password, config.db_name
    );
    let mut db = PostgresDB::connect(&connect_str).unwrap();
    if matches.is_present("clean-db") {
        print!("Cleaning database...");
        stdout().flush().unwrap();
        db.clean().unwrap();
        println!("done");
    }
    if matches.is_present("parse-pdf") {
        let pdf_file = matches.value_of("parse-pdf").unwrap();
        let transactions = parse_and_store(&pdf_file, &mut db).unwrap();
        println!("{} transaction(s) stored in database.", transactions);
    }
    if matches.is_present("pdf-dir") {
        let pdf_dir = matches.value_of("pdf-dir").unwrap();
        let pattern = format!("{}/*.pdf", pdf_dir);
        let mut count_transactions = 0;
        for file in glob(&pattern).expect("Failed to read directory") {
            let filename = file.unwrap().to_str().unwrap().to_owned();
            let transactions = parse_and_store(&filename, &mut db);
            match transactions {
                Err(err) => { println!("Failed to parse file {} with error {:?}", filename, err); },
                Ok(count) => { count_transactions+=count; }
            } 
        }
        println!("{} transaction(s) stored in database.", count_transactions);  
    }
}

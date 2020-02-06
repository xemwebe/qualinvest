///! # qualinvest
///! A cloud based tool for quantitative analysis and management of financial asset portfolios

use serde::Deserialize;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use read_pdf::text_from_pdf;
use read_pdf::read_transactions::parse_transactions;
use finql::data_handler::DataHandler;
use finql::postgres_handler::PostgresDB;

#[macro_use] extern crate lazy_static;

pub mod read_pdf;


fn parse_and_store<DB: DataHandler>(pdf_file: &str, db: &mut DB) {
    let text = text_from_pdf(pdf_file);
    match text {
        Ok(text) => {
            let transactions = parse_transactions(&text);
            match transactions {
                Ok((transactions, asset)) =>  {
                    println!("Found underlying\n{:#?}", asset);
                    if transactions.len() == 0 {
                        println!("Could not parse any transactions!");
                    } else {
                        println!("Found {} transaction{}:", transactions.len(),
                            if transactions.len()>1 {"s"} else {""});
                        for trans in transactions {
                            println!("{:#?}", trans);
                        }
                    }
                },
                Err(err) => {
                    println!("Reading transaction from parsed pdf failed with error {:?}.", err);
                },
            }
        },
        Err(err) => {
            println!("Extracting text from pdf failed with error {}.", err);
        }
    }
}

/// Configuration parameters
#[derive(Debug,Deserialize)]
struct Config {
    db_host: String,
    db_user: String,
    db_password: String,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
	assert!(args.len() == 2, format!("usage: {} <pdf document>", args[0]) );
    let pdf_file = &args[1];

    let config_file = File::open("qualinvest.json").unwrap();
    let config_reader = BufReader::new(config_file);
    let config: Config = serde_json::from_reader(config_reader).unwrap();

    let connect_str = format!("host={} user={} password={} dbname={} sslmode=disable", config.db_host, config.db_user, config.db_password);
    let mut db = PostgresDB::connect(connect_str).unwrap();
    db.clean().unwrap();
    transaction_tests(&mut db);

    parse_and_store(&pdf_file, &mut db);
}
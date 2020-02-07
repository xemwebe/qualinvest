use finql::postgres_handler::PostgresDB;
use read_pdf::parse_and_store;
///! # qualinvest
///! A cloud based tool for quantitative analysis and management of financial asset portfolios
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;

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
    let args: Vec<String> = std::env::args().collect();
    assert!(
        args.len() == 2,
        format!("usage: {} <pdf document>", args[0])
    );
    let pdf_file = &args[1];

    let config_file = File::open("qualinvest.json").unwrap();
    let config_reader = BufReader::new(config_file);
    let config: Config = serde_json::from_reader(config_reader).unwrap();

    let connect_str = format!(
        "host={} user={} password={} dbname={} sslmode=disable",
        config.db_host, config.db_user, config.db_password, config.db_name
    );
    let mut db = PostgresDB::connect(&connect_str).unwrap();
    //db.clean().unwrap();

    let (assets, transactions) = parse_and_store(&pdf_file, &mut db).unwrap();
    println!(
        "{} asset(s) and {} transaction(s) stored in database.",
        assets, transactions
    );
}

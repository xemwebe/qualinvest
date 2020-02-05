///! # qualinvest
///! A cloud based tool for quantitative analysis and management of financial asset portfolios

pub mod read_pdf;
use read_pdf::text_from_pdf;
use read_pdf::read_transaction::parse_transaction;
#[macro_use] extern crate lazy_static;

fn main() {
    let args: Vec<String> = std::env::args().collect();
	assert!(args.len() == 2, format!("usage: {} <pdf document>", args[0]) );
    let pdf_file = &args[1];
    let text = text_from_pdf(&pdf_file);
    match text {
        Ok(text) => {
            let result = parse_transaction(&text);
            match result {
                Ok((transaction, asset)) =>  {
                    println!("Could read transaction\n{:?}\non asset\n{:?}", transaction, asset);
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
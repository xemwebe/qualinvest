///! # qualinvest
///! A cloud based tool for quantitative analysis and management of financial asset portfolios

pub mod read_pdf;
use read_pdf::text_from_pdf;
use read_pdf::read_transactions::parse_transactions;
#[macro_use] extern crate lazy_static;

fn main() {
    let args: Vec<String> = std::env::args().collect();
	assert!(args.len() == 2, format!("usage: {} <pdf document>", args[0]) );
    let pdf_file = &args[1];
    let text = text_from_pdf(&pdf_file);
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
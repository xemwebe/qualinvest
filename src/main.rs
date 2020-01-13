///! # qualinvest
///! A cloud based tool for quantitative analysis and management of financial asset portfolios


pub mod read_pdf;
use read_pdf::text_from_pdf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
	assert!(args.len() == 2, format!("usage: {} <pdf document>", args[0]) );
    let pdf_file = &args[1];
    println!("File {} contains:\n{}", pdf_file, text_from_pdf(&pdf_file).unwrap() );
}
///! Parse text files and extract asset and transaction data
///! Currently, transaction documents from comdirect bank are supported
use super::german_string_to_date;
use super::german_string_to_float;
use super::ReadPDFError;
use crate::Config;
use chrono::{Datelike, TimeZone, Utc, NaiveDate};
use finql::asset::Asset;
use finql::fx_rates::insert_fx_quote;
use finql::memory_handler::InMemoryDB;
use finql::transaction::{Transaction, TransactionType};
use finql::{CashAmount, CashFlow, Currency};
use regex::{Regex,RegexSet};
use std::str::FromStr;

struct AssetInfo {
    asset: Asset,
    // reserved for later use
    _ex_div_day: Option<NaiveDate>,
    position: Option<f64>,
}

// Collect all parsed data that is required to construct by category distinct cash flow transactions
pub struct ParsedTransactionInfo {
    doc_type: DocumentType,
    asset: Asset,
    base_currency: Currency,
    // Price pay/received for buying/selling asset in base currency
    base_asset: f64,
    base_fee: f64,
    base_tax: f64,
    base_accrued: f64,
    // Total payment in account currency
    base_total: f64,
    foreign_currency: Currency,
    foreign_fee: f64,
    foreign_tax: f64,
    foreign_accrued: f64,
    // Price pay/received for buying/selling asset in base currency
    foreign_asset: f64,
    valuta: NaiveDate,
}

/// Extract asset information from text file
fn parse_asset(text: &str) -> Result<AssetInfo, ReadPDFError> {
    lazy_static! {
        static ref NAME_WKN_ISIN: Regex = Regex::new(
            r"(?m)WPKNR/ISIN\n(.*)\s\s\s*([A-Z0-9]{6})\s*\n\s*(.*)\s\s\s*([A-Z0-9]{12})"
        )
        .unwrap();
        // Search for asset in dividend documents
        static ref NAME_WKN_ISIN_DIV: Regex = Regex::new(
            r"(?m)WKN/ISIN\n\s*per\s+([.0-9]*)\s+(.*)\s+([A-Z0-9]{6})\s*\n\s*STK\s+([.,0-9]*)\s+(.*)\s\s\s*([A-Z0-9]{12})"
        )
        .unwrap();
        // Search for asset in dividend documents
        static ref WKN_ISIN_TAX: Regex = Regex::new(r"WKN\s*/\s*ISIN:\s+([A-Z0-9]{6})\s*/\s*([A-Z0-9]{12})").unwrap();
    }
    match NAME_WKN_ISIN.captures(text) {
        Some(cap) => {
            let wkn = Some(cap[2].to_string());
            let isin = Some(cap[4].to_string());
            let name = format!("{} {}", cap[1].trim(), cap[3].trim());
            Ok(AssetInfo {
                asset: Asset {
                    id: None,
                    name,
                    wkn,
                    isin,
                    note: None,
                },
                _ex_div_day: None,
                position: None,
            })
        },
        None => match NAME_WKN_ISIN_DIV.captures(text) {
            Some(cap) => {
                let wkn = Some(cap[3].to_string());
                let isin = Some(cap[6].to_string());
                let name = format!("{} {}", cap[2].trim(), cap[5].trim());
                let ex_div_day = Some(german_string_to_date(&cap[1])?);
                let position = Some(german_string_to_float(&cap[4])?);
                Ok(AssetInfo {
                    asset: Asset {
                        id: None,
                        name,
                        wkn,
                        isin,
                        note: None,
                    },
                    _ex_div_day: ex_div_day,
                    position,
                })
            },
            None => match WKN_ISIN_TAX.captures(text) {
                // The document does not provide the full name, leave name empty and search in database by ISIN/WKN
                Some(cap) => {
                    let wkn = Some(cap[1].to_string());
                    let isin = Some(cap[2].to_string());
                    Ok(AssetInfo {
                        asset: Asset {
                            id: None,
                            name: String::new(),
                            wkn,
                            isin,
                            note: None,
                        },
                        _ex_div_day: None,
                        position: None,
                    })
                },
                None => Err(ReadPDFError::NotFound("asset")),
            },
        },
    }
}

fn parse_amount(regex: &Regex, text: &str) -> Result<Option<CashAmount>, ReadPDFError> {
    match regex.captures(text) {
        None => Ok(None),
        Some(cap) => {
            let amount = german_string_to_float(&cap[2])?;
            let currency =
                Currency::from_str(&cap[1]).map_err(|err| ReadPDFError::ParseCurrency(err))?;
            Ok(Some(CashAmount { amount, currency }))
        }
    }
}

fn must_have(
    amount: Option<CashAmount>,
    error_message: &'static str,
) -> Result<CashAmount, ReadPDFError> {
    match amount {
        None => Err(ReadPDFError::NotFound(error_message)),
        Some(amount) => Ok(amount),
    }
}

fn parse_fx_rate(text: &str) -> Result<(Option<f64>, Option<CashAmount>), ReadPDFError> {
    lazy_static! {
        static ref EXCHANGE_RATE: Regex = Regex::new(
            r"Umrechn. zum Dev. kurs\s+([0-9,.]*)\s+vom\s+[0-9.]*\s+:\s+([A-Z]{3})\s+([-0-9,.]+)"
        )
        .unwrap();
        static ref EXCHANGE_RATE_DIV: Regex = Regex::new(
            r"zum Devisenkurs:\s+[A-Z/]{7}\s+([0-9,.]+)\s\s+([A-Z]{3})\s+([-0-9,.]+)"
        )
        .unwrap();
    }
    let mut cap = EXCHANGE_RATE.captures(text);
    if cap.is_none() {
        cap = EXCHANGE_RATE_DIV.captures(text);
        if cap.is_none() {
            return Ok((None, None));
        }
    }

    let cap = cap.unwrap();
    let fx_rate = german_string_to_float(&cap[1])?;
    let amount = german_string_to_float(&cap[3])?;
    let currency = Currency::from_str(&cap[2]).map_err(|err| ReadPDFError::ParseCurrency(err))?;

    Ok((Some(fx_rate), Some(CashAmount { amount, currency })))
}

fn rounded_equal(x: f64, y: f64, precision: i32) -> bool {
    let factor = 10.0_f64.powi(precision);
    return (x * factor).round() == (y * factor).round();
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum DocumentType {
    Buy,
    Sell,
    Dividend,
    Tax,
}

fn parse_doc_type(text: &str) -> Result<DocumentType, ReadPDFError> {
    lazy_static! {
        static ref DOC_TYPE_SET: RegexSet = RegexSet::new(&[
            r"(?m)^\s*Wertpapierkauf",
            r"(?m)^\s*Wertpapierverkauf",
            r"(?m)^\s*Dividendengutschrift",
            r"(?m)^\s*Ertragsgutschrift",
        ]).unwrap();
        static ref TAX_TYPE: Regex = Regex::new(r"Steuerliche Behandlung:\s+(\w+)\s+(\w+)").unwrap();
    }
    
    let matches: Vec<_> = DOC_TYPE_SET.matches(text).into_iter().collect();
    if matches.len() == 1 {
        // Found document type
        match matches[0] {
            0 => Ok(DocumentType::Buy),
            1 => Ok(DocumentType::Sell),
            2|3 => Ok(DocumentType::Dividend),
            // should never happen
            _ => Err(ReadPDFError::UnknownDocumentType),
        }
    } else if matches.len() == 0 {
        // No document type found, must be tax document
        match TAX_TYPE.captures(text) {
            Some(_) => {
                Ok(DocumentType::Tax)
            },
            None => Err(ReadPDFError::UnknownDocumentType),
        }
    } else {
        // Found more than one document type; this should not happen
        Err(ReadPDFError::UnknownDocumentType)
    }
 }

fn parse_pre_tax(text: &str, doc_type: DocumentType) -> Result<(CashAmount, NaiveDate), ReadPDFError> {
    lazy_static! {
        static ref PRE_TAX_AMOUNT: Regex = Regex::new(
            r"(?m)Zu Ihren Lasten vor Steuern\s*\n.*\s*([0-9.]{10})\s*([A-Z]{3})\s*([-0-9.,]+)"
        )
        .unwrap();
        static ref PRE_TAX_AMOUNT_SELL: Regex = Regex::new(
            r"(?m)Zu Ihren Gunsten vor Steuern\s*\n.*\s*([0-9.]{10})\s*([A-Z]{3})\s*([-0-9.,]+)"
        )
        .unwrap();
        static ref PRE_TAX_AMOUNT_TAX: Regex = Regex::new(
            r"Zu Ihren Gunsten vor Steuern:\s*([A-Z]{3})\s*([-0-9.,]+)"
        )
        .unwrap();
        static ref VALUTA: Regex = Regex::new(
            r"erfolgt mit Valuta\s*([0-9.]{10})"
        )
        .unwrap();
        static ref VALUTA_ALT: Regex = Regex::new(
            r"Datum:\s+([0-9.]{10})"
        )
        .unwrap();
    }

    if doc_type == DocumentType::Tax {
        return match PRE_TAX_AMOUNT_TAX.captures(text) {
            None => Err(ReadPDFError::NotFound("pre-tax amount")),
            Some(cap) => {
                let amount = german_string_to_float(&cap[2])?;
                let currency =
                    Currency::from_str(&cap[1]).map_err(|err| ReadPDFError::ParseCurrency(err))?;
                let valuta = match VALUTA.captures(text) {
                    Some(cap) => Ok(german_string_to_date(&cap[1])?),
                    None => match VALUTA_ALT.captures(text) {
                        Some(cap) => Ok(german_string_to_date(&cap[1])?),
                        None => Err(ReadPDFError::NotFound("pre-tax amount")),
                    },
                }?;
                Ok((CashAmount { amount, currency }, valuta))
            },
        }
    }

    let matches = if doc_type == DocumentType::Sell || doc_type == DocumentType::Dividend { 
        PRE_TAX_AMOUNT_SELL.captures(text) 
    } else { 
        PRE_TAX_AMOUNT.captures(text)
     };
    
    match matches {
        None => Err(ReadPDFError::NotFound("pre-tax amount")),
        Some(cap) => {
            let amount = german_string_to_float(&cap[3])?;
            let currency =
                Currency::from_str(&cap[2]).map_err(|err| ReadPDFError::ParseCurrency(err))?;
            let valuta = german_string_to_date(&cap[1])?;
            Ok((CashAmount { amount, currency }, valuta))
        }
    }
}


/// Extract transaction information from text files
pub fn parse_transactions(
    text: &str,
    config: &Config,
) -> Result<(Vec<Transaction>, Asset), ReadPDFError> {
    lazy_static! {
        static ref TOTAL_POSITION: Regex = Regex::new(
            r"Summe\s+St.\s+([0-9.,]+)\s+[A-Z]{3}\s+[0-9,.]+\s+([A-Z]{3})\s+([-0-9,.]+)"
        )
        .unwrap();
        static ref BOND_POSITION: Regex = Regex::new(r"[A-Z]{3}\s+([0-9.,]+)\s+[0-9.,]+%").unwrap();
        static ref POSITION: Regex = Regex::new(r"St.\s+([0-9.,]+)").unwrap();
        static ref TRADE_VALUE: Regex =
            Regex::new(r"Kurswert\s*:\s+([A-Z]{3})\s+([-0-9,.]*)").unwrap();
        static ref DIV_PRE_TAX: Regex =
            Regex::new(r"Bruttobetrag\s*:\s+([A-Z]{3})\s+([-0-9,.]*)").unwrap();
        static ref PROVISION: Regex =
            Regex::new(r"(?:Gesamtprovision|Provision)\s*:\s+([A-Z]{3})\s+([-0-9,.]*)").unwrap();
        static ref EXCHANGE_FEE: Regex =
            Regex::new(r"Börsenplatzabhäng. Entgelt\s*:\s+([A-Z]{3})\s+([-0-9,.]*)").unwrap();
        static ref VARIABLE_EXCHANGE_FEE: Regex =
            Regex::new(r"Variable Börsenspesen\s*:\s+([A-Z]{3})\s+([-0-9,.]*)").unwrap();
        static ref TRANSFER_FEE: Regex =
            Regex::new(r"Umschreibeentgelt\s*:\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap();
        static ref FOREIGN_EXPENSES: Regex =
            Regex::new(r"[fF]remde Spesen\s*:?\s+([A-Z]{3})\s+([-0-9,.]+ ?-?)").unwrap();
        static ref BROKER_FEE: Regex =
            Regex::new(r"Maklercourtage\s*:\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap();
        static ref FOREIGN_AFTER_FEE: Regex =
            Regex::new(r"Ausmachender Betrag\s*:?\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap();
        static ref FEE_REDUCTION: Regex =
            Regex::new(r"Reduktion Kaufaufschlag\s*:?\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap();
        static ref AFTER_TAX_AMOUNT: Regex =
            Regex::new(r"Zu Ihren Lasten nach Steuern: *([A-Z]{3}) *([-0-9.,]+)").unwrap();
        static ref AFTER_TAX_AMOUNT_SELL: Regex =
            Regex::new(r"Zu Ihren Gunsten nach Steuern: *([A-Z]{3}) *([-0-9.,]+)").unwrap();
        static ref UNSPECIFIED_FEE: Regex =
            Regex::new(r"\n\s*Entgelte\s*:\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap();
        static ref CLEARSTREAM_FEE: Regex =
            Regex::new(r"Abwickl.entgelt Clearstream\s*:\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap();
        static ref TAX_PAY_BACK_FEE: Regex =
            Regex::new(r"Provision für Steuererstattung\s+([A-Z]{3})\s+([-0-9,.]+ ?-?)").unwrap();
        static ref VAT_TAX: Regex =
            Regex::new(r"Mehrwertsteuer auf\s+[A-Z]{3}\s+[-0-9,.]+\s+([A-Z]{3})\s+([-0-9,.]+ ?-?)").unwrap();
        static ref CAPITAL_GAIN_TAX: Regex =
            Regex::new(r"Kapitalertragsteuer\s*\(?[0-9]?\)?\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap();
        static ref SOLIDARITAETS_TAX: Regex =
            Regex::new(r"Solidaritätszuschlag\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap();
        static ref CHURCH_TAX: Regex =
            Regex::new(r"(?m)Kirchensteuer\s+([A-Z]{3})\s*\n\s*_*\s*\n\s*+([-0-9,.]+)").unwrap();
        static ref ACCRUED_INTEREST: Regex =
            Regex::new(r"[0-9]+\s+Tage Zinsen\s+:\s*([A-Z]{3})\s+([-0-9,.]+)").unwrap();
        static ref FOREIGN_DIV_TAX: Regex = 
            Regex::new(r"Quellensteuer\s+([A-Z]{3})\s+([-0-9,.]+ ?-?)").unwrap();
        static ref FOREIGN_DIV_TAX_BACK: Regex = 
            Regex::new(r"Quellensteuervergütung\s+([A-Z]{3})\s+([-0-9,.]+ ?-?)").unwrap();
        static ref AMENDMENT: Regex = Regex::new(r"Nachtragsabrechnung").unwrap();
    }

    let doc_type = parse_doc_type(text)?;
    let mut asset_info = parse_asset(text)?;
    let is_amendment = AMENDMENT.is_match(text);

    let mut transactions = Vec::new();
    // temporary storage for fx rates
    let mut fx_db = InMemoryDB::new();
    let mut pre_tax_fee_value = None;
    if asset_info.position.is_none() {
        // position could not been extracted while parsing asset in file
        asset_info.position = match TOTAL_POSITION.captures(text) {
            None => match POSITION.captures(text) {
                Some(position) => Some(german_string_to_float(&position[1])?),
                None => match BOND_POSITION.captures(text) {
                    Some(position) => Some(german_string_to_float(&position[1])?),
                    None => None,
                },
            },
            Some(position) => {
                let amount = german_string_to_float(&position[3])?;
                let currency =
                    Currency::from_str(&position[2]).map_err(|err| ReadPDFError::ParseCurrency(err))?;
                    pre_tax_fee_value = Some(CashAmount { amount, currency });
                Some(german_string_to_float(&position[1])?)
            }
        };    
    }

    let (pre_tax, valuta) = parse_pre_tax(text, doc_type)?;

    if pre_tax_fee_value.is_none() {
        if doc_type == DocumentType::Buy || doc_type == DocumentType::Sell {
            pre_tax_fee_value = parse_amount(&TRADE_VALUE, text)?;
        } else if doc_type == DocumentType::Dividend {
            pre_tax_fee_value = parse_amount(&DIV_PRE_TAX, text)?;
        } else if doc_type == DocumentType::Tax {
            pre_tax_fee_value = Some(pre_tax);
        }
    }
    let pre_tax_fee_value = must_have(pre_tax_fee_value, "can't find value before taxes and fees")?;

    let time = Utc
        .ymd(valuta.year(), valuta.month(), valuta.day())
        .and_hms_milli(18, 0, 0, 0);
    let base_currency = pre_tax.currency;
    let (fx_rate, converted_amount) = parse_fx_rate(text)?;
    if fx_rate.is_some() {
        let foreign_currency = pre_tax_fee_value.currency;
        let time = Utc
            .ymd(valuta.year(), valuta.month(), valuta.day())
            .and_hms_milli(0, 0, 0, 0);
        insert_fx_quote(
            fx_rate.unwrap(),
            base_currency,
            foreign_currency,
            time,
            &mut fx_db,
        )?;
    }
    let provision = parse_amount(&PROVISION, text)?;
    let exchange_fee = parse_amount(&EXCHANGE_FEE, text)?;
    let transfer_fee = parse_amount(&TRANSFER_FEE, text)?;
    let variable_exchange_fee = parse_amount(&VARIABLE_EXCHANGE_FEE, text)?;
    let foreign_expenses = parse_amount(&FOREIGN_EXPENSES, text)?;
    let unspecified_fee = parse_amount(&UNSPECIFIED_FEE, text)?;
    let clearstream_fee = parse_amount(&CLEARSTREAM_FEE, text)?;
    let tax_pay_back_fee = parse_amount(&TAX_PAY_BACK_FEE, text)?;
    let broker_fee = parse_amount(&BROKER_FEE, text)?;
    let fee_reduction = parse_amount(&FEE_REDUCTION, text)?;
    let accrued_interest = parse_amount(&ACCRUED_INTEREST, text)?;

    let mut total_fee = CashAmount {
        amount: 0.0,
        currency: base_currency,
    };
    total_fee
        .add_opt(provision, time, &mut fx_db)?
        .add_opt(exchange_fee, time, &mut fx_db)?
        .add_opt(transfer_fee, time, &mut fx_db)?
        .add_opt(variable_exchange_fee, time, &mut fx_db)?
        .add_opt(foreign_expenses, time, &mut fx_db)?
        .add_opt(unspecified_fee, time, &mut fx_db)?
        .add_opt(clearstream_fee, time, &mut fx_db)?
        .add_opt(broker_fee, time, &mut fx_db)?
        .add_opt(fee_reduction, time, &mut fx_db)?
        .add_opt(tax_pay_back_fee, time, &mut fx_db)?;

    let foreign_tax = parse_amount(&FOREIGN_DIV_TAX, text)?;
    let foreign_tax_back = parse_amount(&FOREIGN_DIV_TAX_BACK, text)?;
    let mut total_foreign_tax = CashAmount {
        amount: 0.0,
        currency: pre_tax_fee_value.currency,
    };
    total_foreign_tax
        .add_opt(foreign_tax, time, &mut fx_db)?
        .add_opt(foreign_tax_back, time, &mut fx_db)?;

    let capital_gain_tax = parse_amount(&CAPITAL_GAIN_TAX, text)?;
    let solidaritaets_tax = parse_amount(&SOLIDARITAETS_TAX, text)?;
    let church_tax = parse_amount(&CHURCH_TAX, text)?;
    let vat_tax = parse_amount(&VAT_TAX, text)?;

    let mut warnings = String::new();
    let mut total_tax = CashAmount {
        amount: 0.0,
        currency: base_currency,
    };
    total_tax
        .add_opt(capital_gain_tax, time, &mut fx_db)?
        .add_opt(solidaritaets_tax, time, &mut fx_db)?
        .add_opt(church_tax, time, &mut fx_db)?
        .add_opt(vat_tax, time, &mut fx_db)?;

    if config.debug {
        println!(
            "value before tax and fees: {}\npre_tax: {}\nvaluta: {}\nbase_currency: {}\nfx_rate: {:?}",
            pre_tax_fee_value, pre_tax, valuta, base_currency, fx_rate
        );
        println!("provision: {:?}\nexchange_fee: {:?}\ntransfer_fee: {:?}\nvariable_exchange_fee {:?}\nforeign_expenses: {:?}",
                provision, exchange_fee, transfer_fee, variable_exchange_fee, foreign_expenses);
        println!(
            "unspecified_fee: {:?}\ncleartream_fee: {:?}\ntotal_fee: {}",
            unspecified_fee, clearstream_fee, total_fee
        );
        println!(
            "capital_gain_tax: {:?}\nsolidaritaets_tax: {:?}\nchurch_tax: {:?}\ntotal_tax: {}",
            capital_gain_tax, solidaritaets_tax, church_tax, total_tax
        );
    }

    let mut pre_tax_calculated = pre_tax_fee_value;
    pre_tax_calculated.add(total_fee, time, &mut fx_db)?;
    pre_tax_calculated.add(total_foreign_tax, time, &mut fx_db)?;
    if fx_rate.is_some() {
        pre_tax_calculated = CashAmount {
            amount: pre_tax_calculated.amount / fx_rate.unwrap(),
            currency: base_currency,
        }
    }

    pre_tax_calculated.add_opt(accrued_interest, time, &mut fx_db)?
        .add_opt(vat_tax, time, &mut fx_db)?;
    // skip this check if its an amendment, because this tends to become very complex
    if !is_amendment && pre_tax_calculated.amount != 0.0 {
        if !rounded_equal(pre_tax_calculated.amount, pre_tax.amount, 2) {
            warnings = format!("{}\nCalculated pre-tax value {} differs from reported pre-tax value {}: missed some fees or taxes?", 
                    warnings, pre_tax_calculated, pre_tax);
        }
    }

    if fx_rate.is_some() {
        let mut foreign_calculated = parse_amount(&FOREIGN_AFTER_FEE, text)?;
        if foreign_calculated.is_none() && doc_type == DocumentType::Dividend {
            foreign_calculated =  Some(pre_tax_fee_value);
        };
        if foreign_calculated.is_none() {
            return Err(ReadPDFError::NotFound("Unable to calculate foreign amount after taxes and fees"));
        }
        let mut foreign_calculated = foreign_calculated.unwrap();
        let calculated_converted_amount = CashAmount {
            amount: foreign_calculated.amount / fx_rate.unwrap(),
            currency: base_currency,
        };
        if !rounded_equal(
            calculated_converted_amount.amount,
            converted_amount.unwrap().amount,
            2,
        ) {
            warnings = format!(
                "{}\nConverted foreign amount {} differs form calculated foreign amount {}.",
                warnings,
                converted_amount.unwrap(),
                calculated_converted_amount
            );
        }
        foreign_calculated.sub_opt(foreign_expenses, time, &mut fx_db)?;
        if fee_reduction.is_some() {
            if fee_reduction.unwrap().currency == pre_tax_fee_value.currency {
                foreign_calculated.sub_opt(fee_reduction, time, &mut fx_db)?;
            }
        }
        foreign_calculated.sub(total_foreign_tax, time, &mut fx_db)?;
        if !rounded_equal(foreign_calculated.amount, pre_tax_fee_value.amount, 2)
            || foreign_calculated.currency != pre_tax_fee_value.currency
        {
            warnings = format!(
                "{}\nCalculated foreign amount {} differs from parsed amount {}.",
                warnings, foreign_calculated, pre_tax_fee_value
            );
        }
    }

    // Do some consistency checks to verify if implicit assumptions are correct
    // These should probably be disabled once parsing is complete
    let mut calculated_after_tax = pre_tax;
    let mut after_tax;
    if doc_type == DocumentType::Sell { 
        after_tax = parse_amount(&AFTER_TAX_AMOUNT_SELL, text)?;
        calculated_after_tax.add(total_tax, time, &mut fx_db)?;
    } else { 
        after_tax = parse_amount(&AFTER_TAX_AMOUNT, text)?;
        if after_tax.is_some() {
            after_tax = Some(CashAmount{amount: -after_tax.unwrap().amount, currency: after_tax.unwrap().currency});
        }
        calculated_after_tax.sub(total_tax, time, &mut fx_db)?;
    }
    if after_tax.is_some() {
        if !rounded_equal(after_tax.unwrap().amount, calculated_after_tax.amount, 2) {
            warnings = format!(
                "{}\nAfter tax amount {} differs from calculated after tax amount {}.",
                warnings,
                after_tax.unwrap(),
                calculated_after_tax
            );
        }
    }

    let mut note = None;
    if warnings != "" {
        if config.consistency_check {
            return Err(ReadPDFError::ConsistencyCheckFailed(warnings));
        }
        note = Some(warnings);
    }
    // End of consistency checks

    if doc_type == DocumentType::Buy || doc_type == DocumentType::Sell {
        // check if everything required for buy/sell transactions was found
        
        if asset_info.position.is_none() {
            return Err(ReadPDFError::NotFound("position"));
        }

        let sign = if doc_type == DocumentType::Sell { -1.0 } else { 1.0 };
        transactions.push(Transaction {
            id: None,
            transaction_type: TransactionType::Asset {
                asset_id: 0,
                position: sign * asset_info.position.unwrap(),
            },
            cash_flow: CashFlow {
                amount: CashAmount {
                    amount: -sign * pre_tax_fee_value.amount,
                    currency: pre_tax_fee_value.currency,
                },
                date: valuta,
            },
            note,
        });    
    } else if doc_type == DocumentType::Dividend {
        if is_amendment {
            // foreign tax pay back
            transactions.push(Transaction {
                id: None,
                transaction_type: TransactionType::Tax {
                    transaction_ref: None,
                },
                cash_flow: CashFlow {
                    amount: pre_tax,
                    date: valuta,
                },
                note: Some("foreign tax pay back".to_string()),
            })
        } else {
            transactions.push(Transaction {
                id: None, 
                transaction_type: TransactionType::Dividend {
                    asset_id: 0,
                },
                cash_flow: CashFlow {
                    amount: pre_tax_fee_value,
                    date: valuta, 
                },
                note,
            });    
        }
    }

    if total_fee.amount != 0.0 {
        // Add fee transaction
        let sign = if doc_type == DocumentType::Sell { -1.0 } else { 1.0 };
        transactions.push(Transaction {
            id: None,
            transaction_type: TransactionType::Fee {
                transaction_ref: None,
            },
            cash_flow: CashFlow {
                amount: CashAmount {
                    amount: -sign * total_fee.amount,
                    currency: total_fee.currency,
                },
                date: valuta,
            },
            note: None,
        });
    }

    if total_tax.amount != 0.0 {
        // Add tax transaction
        transactions.push(Transaction {
            id: None,
            transaction_type: TransactionType::Tax {
                transaction_ref: None,
            },
            cash_flow: CashFlow {
                amount: CashAmount {
                    amount: total_tax.amount,
                    currency: total_tax.currency,
                },
                date: valuta,
            },
            note: None,
        });
    }

    if accrued_interest.is_some() {
        // Add interest transaction
        let accrued_interest = accrued_interest.unwrap();
        transactions.push(Transaction {
            id: None,
            transaction_type: TransactionType::Interest { asset_id: 0 },
            cash_flow: CashFlow {
                amount: CashAmount {
                    amount: -accrued_interest.amount,
                    currency: accrued_interest.currency,
                },
                date: valuta,
            },
            note: None,
        });
    }

    if total_foreign_tax.amount != 0.0 {
        // Add tax transaction in foreign currency
        transactions.push(Transaction {
            id: None,
            transaction_type: TransactionType::Tax {
                transaction_ref: None,
            },
            cash_flow: CashFlow {
                amount: total_foreign_tax,
                date: valuta,
            },
            note: None,
        });
    }

    Ok((transactions, asset_info.asset))
}

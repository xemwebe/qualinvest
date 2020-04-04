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
struct ParsedTransactionInfo {
    doc_type: DocumentType,
    asset: Asset,
    position: f64,
    valuta: NaiveDate,
    fx_rate: Option<f64>,
    main_amount: CashAmount,
    total_amount: CashAmount,
    extra_fees: Vec<CashAmount>,
    extra_taxes: Vec<CashAmount>,
    accruals: Vec<CashAmount>,
    note: Option<String>,
}

impl ParsedTransactionInfo{
    fn new(doc_type: DocumentType, asset: Asset, main_amount: CashAmount, total_amount: CashAmount, fx_rate: Option<f64>, valuta: NaiveDate) -> ParsedTransactionInfo {
        ParsedTransactionInfo{
            doc_type,
            asset,
            position: 0.0,
            valuta,
            fx_rate,
            main_amount,
            total_amount,
            extra_fees: Vec::new(),
            extra_taxes: Vec::new(),
            accruals: Vec::new(),
            note: None,
        }
    }
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

fn add_or_append(payments: &mut Vec<CashAmount>, regex: &Regex, text: &str, factor: f64) -> Result<(),ReadPDFError> {
    let new_payment = parse_amount(regex, text)?;
    if new_payment.is_none() {
        return Ok(());
    }
    let new_payment = new_payment.unwrap();
    for payment in (*payments).iter_mut() {
        if payment.currency == new_payment.currency {
            payment.amount += factor * new_payment.amount;
            break;
        }
    }
    payments.push(CashAmount{ 
        amount: factor * new_payment.amount,
        currency: new_payment.currency,
    });
    Ok(())
}

fn parse_payment_components(payments: &mut Vec<CashAmount>, regex_vec: &Vec<Regex>, text: &str, factor: f64) -> Result<(), ReadPDFError> {
    for regex in regex_vec {
        add_or_append(payments, &regex, text, factor)?;
    }
    Ok(())
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
        static ref AFTER_TAX_AMOUNT: Regex = 
            Regex::new(r"Zu Ihren Lasten nach Steuern: *([A-Z]{3}) *([-0-9.,]+)").unwrap();
        static ref AFTER_TAX_AMOUNT_SELL: Regex = 
            Regex::new(r"Zu Ihren Gunsten nach Steuern: *([A-Z]{3}) *([-0-9.,]+)").unwrap();

        static ref COMDIRECT_FEES: Vec<Regex> = vec![
            Regex::new(r"(?:Gesamtprovision|Provision)\s*:\s+([A-Z]{3})\s+([-0-9,.]*)").unwrap(),
            Regex::new(r"Börsenplatzabhäng. Entgelt\s*:\s+([A-Z]{3})\s+([-0-9,.]*)").unwrap(),
            Regex::new(r"Variable Börsenspesen\s*:\s+([A-Z]{3})\s+([-0-9,.]*)").unwrap(),
            Regex::new(r"Umschreibeentgelt\s*:\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap(),
            Regex::new(r"[fF]remde Spesen\s*:?\s+([A-Z]{3})\s+([-0-9,.]+ ?-?)").unwrap(),
            Regex::new(r"Maklercourtage\s*:\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap(),
            Regex::new(r"Ausmachender Betrag\s*:?\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap(),
            Regex::new(r"Reduktion Kaufaufschlag\s*:?\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap(),
            Regex::new(r"\n\s*Entgelte\s*:\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap(),
            Regex::new(r"Abwickl.entgelt Clearstream\s*:\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap(),
            Regex::new(r"Provision für Steuererstattung\s+([A-Z]{3})\s+([-0-9,.]+ ?-?)").unwrap(),
        ];

        static ref COMDIRECT_TAXES: Vec<Regex> = vec![
            Regex::new(r"Mehrwertsteuer auf\s+[A-Z]{3}\s+[-0-9,.]+\s+([A-Z]{3})\s+([-0-9,.]+ ?-?)").unwrap(),
            Regex::new(r"Kapitalertragsteuer\s*\(?[0-9]?\)?\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap(),
            Regex::new(r"Solidaritätszuschlag\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap(),
            Regex::new(r"(?m)Kirchensteuer\s+([A-Z]{3})\s*\n\s*_*\s*\n\s*+([-0-9,.]+)").unwrap(),
            Regex::new(r"Quellensteuer\s+([A-Z]{3})\s+([-0-9,.]+ ?-?)").unwrap(),
            Regex::new(r"Quellensteuervergütung\s+([A-Z]{3})\s+([-0-9,.]+ ?-?)").unwrap(),
        ];
        
        static ref COMDIRECT_ACCRUALS: Vec<Regex> = vec![
            Regex::new(r"[0-9]+\s+Tage Zinsen\s+:\s*([A-Z]{3})\s+([-0-9,.]+)").unwrap(),
        ];

        static ref AMENDMENT: Regex = Regex::new(r"Nachtragsabrechnung").unwrap();
    }

    let doc_type = parse_doc_type(text)?;
    let mut asset_info = parse_asset(text)?;
    let is_amendment = AMENDMENT.is_match(text);

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

    // Determine final value
    let mut after_tax;
    if doc_type == DocumentType::Sell { 
        after_tax = parse_amount(&AFTER_TAX_AMOUNT_SELL, text)?;
    } else { 
        after_tax = parse_amount(&AFTER_TAX_AMOUNT, text)?;
        if after_tax.is_some() {
            after_tax = Some(CashAmount{amount: -after_tax.unwrap().amount, currency: after_tax.unwrap().currency});
        }
    }
    let after_tax = must_have(after_tax, "can't identify final payment")?;
    let (fx_rate, _) = parse_fx_rate(text)?;

    // Collect essential informations in ParsedTransactionInfo
    let mut tri = match doc_type {
        DocumentType::Buy | DocumentType::Sell => {
            let sign = if doc_type == DocumentType::Sell { -1.0 } else { 1.0 };
            if asset_info.position.is_none() {
                return Err(ReadPDFError::NotFound("position"));
            }
            let main_amount = CashAmount {
                amount: -sign * pre_tax_fee_value.amount,
                currency: pre_tax_fee_value.currency,
            };
            let mut tri = ParsedTransactionInfo::new(doc_type, asset_info.asset, main_amount, after_tax, fx_rate, valuta);
            tri.position = sign * asset_info.position.unwrap();
            tri           
        },
        DocumentType::Dividend => {
            if is_amendment {
                // foreign tax pay back
                let mut tri = ParsedTransactionInfo::new(DocumentType::Tax, asset_info.asset, pre_tax, after_tax, fx_rate, valuta);
                tri.note = Some("foreign tax pay back\n".to_string());
                tri
            } else {
                ParsedTransactionInfo::new(doc_type, asset_info.asset, pre_tax_fee_value, after_tax, fx_rate, valuta)
            }
        },
        DocumentType::Tax => {
            ParsedTransactionInfo::new(DocumentType::Tax, asset_info.asset, after_tax, after_tax, fx_rate, valuta)
        }
    };

    let sign = if tri.doc_type == DocumentType::Sell { 1.0 } else { -1.0 };
    parse_payment_components(&mut tri.extra_fees, &COMDIRECT_FEES, text, sign)?;
    parse_payment_components(&mut tri.extra_taxes, &COMDIRECT_TAXES, text, 1.0)?;
    parse_payment_components(&mut tri.accruals, &COMDIRECT_ACCRUALS, text, -1.0)?;


    if config.consistency_check {
        check_consistency(&tri)?;
    }

    // Generate list of transactions
    make_transactions(&tri) 
}

// Check if main payment plus all fees and taxes add up to total payment
fn check_consistency(tri: &ParsedTransactionInfo) -> Result<(),ReadPDFError> {
    let time = Utc
        .ymd(tri.valuta.year(), tri.valuta.month(), tri.valuta.day())
        .and_hms_milli(18, 0, 0, 0);

    // temporary storage for fx rates
    let mut fx_db = InMemoryDB::new();
    if tri.fx_rate.is_some() {
        insert_fx_quote(
            tri.fx_rate.unwrap(),
            tri.total_amount.currency,
            tri.main_amount.currency,
            time,
            &mut fx_db,
        )?;
    }
    // Add up all payment components and check whether they equal the final payment
    let mut check_sum = tri.main_amount;
    check_sum.sub(tri.total_amount, time, &mut fx_db)?;
    for fee in &tri.extra_fees {
        check_sum.add(*fee, time, &mut fx_db)?;
    }
    for tax in &tri.extra_taxes {
        check_sum.add(*tax, time, &mut fx_db)?;
    }
    for accrued in &tri.accruals {
        check_sum.add(*accrued, time, &mut fx_db)?;
    }
    // Final sum should be nearly zero
    if !rounded_equal(check_sum.amount, 0.0, 4) {
        let warning = format!("Sum of payments does not equal total payments, difference is {}.", check_sum.amount);
        return Err(ReadPDFError::ConsistencyCheckFailed(warning));
    } else {
        Ok(())
    }
}

fn make_transactions(tri: &ParsedTransactionInfo)-> Result<(Vec<Transaction>, Asset), ReadPDFError> 
{
    let mut transactions = Vec::new();
    // Construct main transaction
    transactions.push(Transaction {
            id: None,
            transaction_type: match tri.doc_type {
                DocumentType::Buy | DocumentType::Sell => { 
                    TransactionType::Asset {
                        asset_id: 0,
                        position: tri.position
                    }
                },
                DocumentType::Dividend => {
                    TransactionType::Dividend {
                        asset_id: 0,
                    }
                },
                DocumentType::Tax => {
                    TransactionType::Tax {
                        transaction_ref: None,
                    }
                }
            },
            cash_flow: CashFlow {
                amount: tri.main_amount,
                date: tri.valuta,
            },
            note: tri.note.clone(),
        }
    );

    for fee in &tri.extra_fees {
        transactions.push(Transaction {
            id: None,
            transaction_type: TransactionType::Fee {
                transaction_ref: None,
            },
            cash_flow: CashFlow {
                amount: *fee,
                date: tri.valuta,
            },
            note: None,
        });
    }

    for tax in &tri.extra_taxes {
        transactions.push(Transaction {
            id: None,
            transaction_type: TransactionType::Tax {
                transaction_ref: None,
            },
            cash_flow: CashFlow {
                amount: *tax,
                date: tri.valuta,
            },
            note: None,
        });
    }

    for accrued in &tri.accruals {
        transactions.push(Transaction {
            id: None,
            transaction_type: TransactionType::Interest { asset_id: 0 },
            cash_flow: CashFlow {
                amount: *accrued,
                date: tri.valuta,
            },
            note: None,
        });
    }

    Ok((transactions, tri.asset.clone()))
}

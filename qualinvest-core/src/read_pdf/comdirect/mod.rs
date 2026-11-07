//! Parse text files and extract asset and transaction data
use super::{ParsedDocument, ParsedInfo, ReadPDFError};
use crate::read_pdf::{
    read_account_info::Bank,
    utils::{add_by_currency, german_string_to_date, german_string_to_float, rounded_equal},
};
use lazy_static::lazy_static;
use log::{debug, error, trace};
use regex::{Regex, RegexSet};
use std::sync::Arc;
use time::Date;

use finql::{
    datatypes::{
        date_time_helper::make_offset_time, Asset, CashAmount, CashFlow, CurrencyISOCode, Stock,
        Transaction, TransactionType,
    },
    fx_rates::SimpleCurrencyConverter,
    Market,
};

type Result<T> = std::result::Result<T, ReadPDFError>;

mod finanzreport;

#[derive(Debug, PartialEq, Clone, Copy)]
enum DocumentType {
    Buy,
    Sell,
    Dividend,
    Tax,
    Interest,
    BondPayBack,
    Finanzreport,
}

// Collect all parsed data that is required to construct by category distinct cash flow transactions
#[derive(Debug)]
pub struct ParsedTransactionInfo {
    doc_type: DocumentType,
    asset: Asset,
    position: f64,
    valuta: Date,
    fx_rate: Option<f64>,
    main_amount: CashAmount,
    total_amount: CashAmount,
    extra_fees: Vec<CashAmount>,
    extra_taxes: Vec<CashAmount>,
    accruals: Vec<CashAmount>,
    note: Option<String>,
    account_info: Option<(String, String)>,
}

impl ParsedTransactionInfo {
    fn new(
        doc_type: DocumentType,
        asset: Asset,
        main_amount: CashAmount,
        total_amount: CashAmount,
        fx_rate: Option<f64>,
        valuta: Date,
    ) -> ParsedTransactionInfo {
        ParsedTransactionInfo {
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
            account_info: None,
        }
    }
}

impl ParsedDocument for ParsedTransactionInfo {
    fn add_account_info(&mut self, bank: Bank, account: &str) {
        self.account_info = Some((format!("{}", bank), account.to_string()));
    }
    fn get_account_info(&self) -> Option<(String, String)> {
        self.account_info.clone()
    }

    // Check if main payment plus all fees and taxes add up to total payment
    // Add up all payments separate by currencies, convert into total currency, and check if they add up to zero.
    fn check_consistency(&self) -> Result<()> {
        debug!("check_consistency called");
        let time = make_offset_time(
            self.valuta.year(),
            self.valuta.month() as u32,
            self.valuta.day() as u32,
            18,
            0,
            0,
        )
        .ok_or(ReadPDFError::ParseDate)?;
        trace!("closing time is {time}");

        // temporary storage for fx rates
        // total payment is always in base currency, but main_amount (and maybe fees or taxes) could be in foreign currency.
        let mut fx_converter = SimpleCurrencyConverter::new();
        if let Some(fx_rate) = self.fx_rate {
            trace!("fx rate to be added to fx converter: {fx_rate}");
            fx_converter.insert_fx_rate(
                self.total_amount.currency,
                self.main_amount.currency,
                fx_rate,
            );
        }

        // Add up all payment components and check whether they equal the final payment
        let mut check_sum = -self.total_amount;
        let mut foreign_check_sum = self.main_amount;
        for fee in &self.extra_fees {
            add_by_currency(fee, &mut check_sum, &mut foreign_check_sum);
        }
        for tax in &self.extra_taxes {
            add_by_currency(tax, &mut check_sum, &mut foreign_check_sum);
        }
        for accrued in &self.accruals {
            add_by_currency(accrued, &mut check_sum, &mut foreign_check_sum);
        }
        trace!("foreign check sum: {foreign_check_sum}, time: {time}");
        check_sum.add_no_db(foreign_check_sum, time, &fx_converter, true)?;
        trace!("check_sum after adding foreign check sum is {check_sum}");
        // Final sum should be nearly zero
        if !rounded_equal(check_sum.amount, 0.0, 4) {
            let warning = format!(
                "Sum of payments does not equal total payments, difference is {}.",
                check_sum.amount
            );
            error!("consistency check failed");
            Err(ReadPDFError::ConsistencyCheckFailed(warning))
        } else {
            debug!("consistency check was successfull");
            Ok(())
        }
    }

    // Transaction in foreign currency will be converted to currency of total payment amount
    fn make_transactions(&self) -> Result<(Vec<Transaction>, Option<Asset>)> {
        debug!("make_transactions called");
        let mut transactions = Vec::new();
        let time = make_offset_time(
            self.valuta.year(),
            self.valuta.month() as u32,
            self.valuta.day() as u32,
            18,
            0,
            0,
        )
        .ok_or(ReadPDFError::InvalidDate)?;

        // temporary storage for fx rates
        // total payment is always in base currency, but main_amount (and maybe fees or taxes) could be in foreign currency.
        let mut fx_converter = SimpleCurrencyConverter::new();
        if self.fx_rate.is_some() {
            fx_converter.insert_fx_rate(
                self.total_amount.currency,
                self.main_amount.currency,
                self.fx_rate.unwrap(),
            );
        }

        // Construct main transaction
        if self.main_amount.amount != 0.0 {
            transactions.push(Transaction {
                id: None,
                transaction_type: match self.doc_type {
                    DocumentType::Buy | DocumentType::Sell | DocumentType::BondPayBack => {
                        TransactionType::Asset {
                            asset_id: 0,
                            position: self.position,
                        }
                    }
                    DocumentType::Dividend => TransactionType::Dividend { asset_id: 0 },
                    DocumentType::Interest => TransactionType::Interest { asset_id: 0 },
                    DocumentType::Tax => TransactionType::Tax {
                        transaction_ref: None,
                    },
                    DocumentType::Finanzreport => {
                        return Err(ReadPDFError::UnknownDocumentType);
                    }
                },
                cash_flow: CashFlow {
                    amount: self.main_amount,
                    date: self.valuta,
                },
                note: self.note.clone(),
            });
        } else {
            // No main transaction, nothing todo
            return Ok((transactions, Some(self.asset.clone())));
        }

        let mut total_fee = CashAmount {
            amount: 0.0,
            currency: self.total_amount.currency,
        };
        for fee in &self.extra_fees {
            total_fee.add_no_db(*fee, time, &fx_converter, true)?;
        }
        if total_fee.amount != 0.0 {
            transactions.push(Transaction {
                id: None,
                transaction_type: TransactionType::Fee {
                    transaction_ref: None,
                },
                cash_flow: CashFlow {
                    amount: total_fee,
                    date: self.valuta,
                },
                note: None,
            });
        }

        let mut total_tax = CashAmount {
            amount: 0.0,
            currency: self.total_amount.currency,
        };
        for tax in &self.extra_taxes {
            total_tax.add_no_db(*tax, time, &fx_converter, true)?;
        }
        if total_tax.amount != 0.0 {
            transactions.push(Transaction {
                id: None,
                transaction_type: TransactionType::Tax {
                    transaction_ref: None,
                },
                cash_flow: CashFlow {
                    amount: total_tax,
                    date: self.valuta,
                },
                note: None,
            });
        }

        let mut total_accrued = CashAmount {
            amount: 0.0,
            currency: self.total_amount.currency,
        };
        for accrued in &self.accruals {
            total_accrued
                .add_no_db(*accrued, time, &fx_converter, true)
                .map_err(|_| ReadPDFError::CurrencyMismatch)?;
        }
        if total_accrued.amount != 0.0 {
            transactions.push(Transaction {
                id: None,
                transaction_type: TransactionType::Interest { asset_id: 0 },
                cash_flow: CashFlow {
                    amount: total_accrued,
                    date: self.valuta,
                },
                note: None,
            });
        }

        // Ensure that sum of payments equal total payments in spite of rounding errors
        transactions[0].cash_flow.amount.amount =
            self.total_amount.amount - total_accrued.amount - total_tax.amount - total_fee.amount;
        transactions[0].cash_flow.amount.currency = self.total_amount.currency;

        Ok((transactions, Some(self.asset.clone())))
    }
}

#[derive(Debug)]
struct AssetInfo {
    asset: Asset,
    // reserved for later use; could also be ex-interest date
    _ex_div_day: Option<Date>,
    // interest rate of a bond, for later use
    _interest_rate: Option<f64>,
    position: Option<f64>,
}

/// Extract asset information from text file
fn parse_asset(doc_type: DocumentType, text: &str) -> Result<AssetInfo> {
    lazy_static! {
        static ref NAME_WKN_ISIN: Regex = Regex::new(
            r"WPKNR/ISIN\s*(.*)\s+([A-Z0-9]{6})\s*(.*)\s+([A-Z0-9]{12})"
        )
        .unwrap();
        // Search for asset in dividend documents
        // r"(?m)WKN/ISIN\n\s*per\s+([.0-9]{10})\s+(.*)\s+([A-Z0-9]{6})\s*\n\s*STK\s+([.,0-9]*)\s+(.*)\s\s\s*([A-Z0-9]{12})"        )
        static ref NAME_WKN_ISIN_DIV: Regex = Regex::new(
            r"WKN/ISIN\s*per\s+([.0-9]{10})\s+([A-Z0-9]{6})\s*(.*)\s*STK\s+([.,0-9]*)\s+([A-Z0-9]{12})"
        )
        .unwrap();
        // Search for asset in interest documents
        static ref NAME_WKN_ISIN_INT: Regex = Regex::new(
            r"(?m)WKN/ISIN\n\s*per\s+([.0-9]{10})\s+([-.,0-9]+)\s+(.*)\s+([A-Z0-9]{6})\s*\n\s*[A-Z]{3}\s+([.,0-9]*)\s+(.*)\s\s\s*([A-Z0-9]{12})"
        )
        .unwrap();
        // Search for asset in tax documents
        static ref NAME_WKN_ISIN_TAX: Regex = Regex::new(r"Stk\.\s+([-.,0-9]*)\s+(.*)\s,\s*WKN\s*/\s*ISIN:\s+([A-Z0-9]{6})\s*/\s*([A-Z0-9]{12})").unwrap();
    }

    match doc_type {
        DocumentType::Interest | DocumentType::BondPayBack => {
            match NAME_WKN_ISIN_INT.captures(text) {
                Some(cap) => {
                    trace!("captures: {cap:?}");
                    let wkn = Some(cap[4].to_string());
                    let isin = Some(cap[7].to_string());
                    let name = format!("{} {}", cap[3].trim(), cap[6].trim());
                    let ex_div_day = Some(german_string_to_date(&cap[1])?);
                    let position = Some(german_string_to_float(&cap[5])?);
                    let interest_rate = Some(german_string_to_float(&cap[2])?);
                    debug!("Debug: Found asset in Interest or BondPayBack with wkn: {:?}, isin: {:?}, name: '{:?}', ex_div_day: {:?}, position: {:?}, inerest rate: {:?}",
                            wkn, isin, name, ex_div_day, position, interest_rate);
                    Ok(AssetInfo {
                        asset: Asset::Stock(Stock::new(None, name, isin, wkn, None)),
                        _ex_div_day: ex_div_day,
                        _interest_rate: interest_rate,
                        position,
                    })
                }
                None => Err(ReadPDFError::NotFound("asset")),
            }
        }
        DocumentType::Tax => {
            match NAME_WKN_ISIN_TAX.captures(text) {
                // The document does not provide the full name, leave name empty and search in database by ISIN/WKN
                Some(cap) => {
                    trace!("captures: {cap:?}");
                    let position = Some(german_string_to_float(&cap[1])?);
                    let name = cap[2].trim().to_string();
                    let wkn = Some(cap[3].to_string());
                    let isin = Some(cap[4].to_string());
                    debug!(
                        "Debug: Found asset in Tax info with wkn: {:?}, isin: {:?}",
                        wkn, isin
                    );
                    Ok(AssetInfo {
                        asset: Asset::Stock(Stock::new(None, name, isin, wkn, None)),
                        _ex_div_day: None,
                        _interest_rate: None,
                        position,
                    })
                }
                None => Err(ReadPDFError::NotFound("asset")),
            }
        }
        DocumentType::Dividend => match NAME_WKN_ISIN_DIV.captures(text) {
            Some(cap) => {
                trace!("captures: {cap:?}");
                let wkn = Some(cap[2].to_string());
                let isin = Some(cap[5].to_string());
                let name = cap[3].trim().to_string();
                let ex_div_day = Some(german_string_to_date(&cap[1])?);
                let position = Some(german_string_to_float(&cap[4])?);
                debug!("Found asset in Dividend note with wkn: {:?}, isin: {:?}, name: '{:?}', ex_div_day: {:?}, position: {:?}",
                        wkn, isin, name, ex_div_day, position);
                Ok(AssetInfo {
                    asset: Asset::Stock(Stock::new(None, name, isin, wkn, None)),
                    _ex_div_day: ex_div_day,
                    _interest_rate: None,
                    position,
                })
            }
            None => Err(ReadPDFError::NotFound("asset")),
        },
        _ => match NAME_WKN_ISIN.captures(text) {
            Some(cap) => {
                let wkn = Some(cap[2].to_string());
                let isin = Some(cap[4].to_string());
                let name = format!("{} {}", cap[1].trim(), cap[3].trim());
                Ok(AssetInfo {
                    asset: Asset::Stock(Stock::new(None, name, isin, wkn, None)),
                    _ex_div_day: None,
                    _interest_rate: None,
                    position: None,
                })
            }
            None => Err(ReadPDFError::NotFound("asset")),
        },
    }
}

async fn parse_amount(regex: &Regex, text: &str, market: &Market) -> Result<Option<CashAmount>> {
    match regex.captures(text) {
        None => Ok(None),
        Some(cap) => {
            let amount = german_string_to_float(&cap[2])?;
            let currency = market.get_currency_from_str(&cap[1]).await?;
            Ok(Some(CashAmount { amount, currency }))
        }
    }
}

fn must_have(amount: Option<CashAmount>, error_message: &'static str) -> Result<CashAmount> {
    match amount {
        None => Err(ReadPDFError::NotFound(error_message)),
        Some(amount) => Ok(amount),
    }
}

async fn parse_fx_rate(text: &str, market: &Market) -> Result<(Option<f64>, Option<CashAmount>)> {
    lazy_static! {
        static ref EXCHANGE_RATE: Regex = Regex::new(
            r"Umrechn. zum Dev. kurs\s+([0-9,.]*)\s+vom\s+[0-9.]*\s+:\s+([A-Z]{3})\s+([-0-9,.]+)"
        )
        .unwrap();
        static ref EXCHANGE_RATE_DIV: Regex =
            Regex::new(r"zum Devisenkurs:\s+[A-Z/]{7}\s+([0-9,.]+)\s+([A-Z]{3})\s+([-0-9,.]+)")
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
    debug!("Parsing FX-rate: {} {} {}", &cap[1], &cap[2], &cap[3]);
    let fx_rate = german_string_to_float(&cap[1])?;
    let amount = german_string_to_float(&cap[3])?;
    let currency = market
        .get_currency(CurrencyISOCode::new(&cap[2]).map_err(ReadPDFError::ParseCurrency)?)
        .await?;

    Ok((Some(fx_rate), Some(CashAmount { amount, currency })))
}

fn parse_doc_type(text: &str) -> Result<DocumentType> {
    lazy_static! {
        static ref DOC_TYPE_SET: RegexSet = RegexSet::new([
            r"(?m)^\s*\*?\*?Wertpapierkauf",
            r"(?m)^\s*\*?\*?Wertpapierverkauf",
            r"(?m)^\s*Dividendengutschrift",
            r"(?m)^\s*Ertragsgutschrift",
            r"(?m)^\s*Zinsgutschrift",
            r"(?m)^\s*Einlösung",
        ])
        .unwrap();
        static ref TAX_TYPE: Regex =
            Regex::new(r"Steuerliche Behandlung:\s+(\w+)\s+(\w+)").unwrap();
    }

    let matches: Vec<_> = DOC_TYPE_SET.matches(text).into_iter().collect();
    if matches.len() == 1 {
        // Found document type
        match matches[0] {
            0 => Ok(DocumentType::Buy),
            1 => Ok(DocumentType::Sell),
            2 | 3 => Ok(DocumentType::Dividend),
            4 => Ok(DocumentType::Interest),
            5 => Ok(DocumentType::BondPayBack),
            // should never happen
            _ => Err(ReadPDFError::UnknownDocumentType),
        }
    } else if matches.is_empty() {
        // No document type found, must be tax document
        match TAX_TYPE.captures(text) {
            Some(_) => Ok(DocumentType::Tax),
            None => Err(ReadPDFError::UnknownDocumentType),
        }
    } else {
        // Found more than one document type; this should not happen
        Err(ReadPDFError::UnknownDocumentType)
    }
}

async fn parse_pre_tax(
    text: &str,
    doc_type: DocumentType,
    market: &Market,
) -> Result<(CashAmount, Date)> {
    lazy_static! {
        static ref PRE_TAX_AMOUNT: Regex = Regex::new(
            r"Zu Ihren (?:Gunsten|Lasten) vor Steuern:?\s*([A-Z]{2}[0-9 ]*)\s[A-Z]{3}\s*([0-9.]{10})\s*([A-Z]{3})\s*([-0-9.,]+)"
        )
        .unwrap();
        static ref PRE_TAX_AMOUNT_TAX: Regex =
            Regex::new(r"Zu Ihren Gunsten vor\s*Steuern:\s*([A-Z]{3})\s*([-0-9.,]+)")
                .unwrap();
        static ref VALUTA: Regex = Regex::new(r"erfolgt mit Valuta\s*([0-9.]{10})").unwrap();
        static ref VALUTA_ALT: Regex = Regex::new(r"Datum:\s+([0-9.]{10})").unwrap();
    }

    if doc_type == DocumentType::Tax {
        return match PRE_TAX_AMOUNT_TAX.captures(text) {
            None => Err(ReadPDFError::NotFound("pre-tax amount")),
            Some(cap) => {
                trace!("pre tax amount tax captures: {cap:?}");
                let amount = german_string_to_float(&cap[2])?;
                let currency = market
                    .get_currency(
                        CurrencyISOCode::new(&cap[1]).map_err(ReadPDFError::ParseCurrency)?,
                    )
                    .await?;
                let valuta = match VALUTA.captures(text) {
                    Some(cap) => Ok(german_string_to_date(&cap[1])?),
                    None => match VALUTA_ALT.captures(text) {
                        Some(cap) => Ok(german_string_to_date(&cap[1])?),
                        None => Err(ReadPDFError::NotFound("pre-tax amount")),
                    },
                }?;
                Ok((CashAmount { amount, currency }, valuta))
            }
        };
    }

    match PRE_TAX_AMOUNT.captures(text) {
        Some(cap) => {
            trace!("pre tax amount2 captures: {cap:?}");
            let amount = german_string_to_float(&cap[4])?;
            let currency = market
                .get_currency(CurrencyISOCode::new(&cap[3]).map_err(ReadPDFError::ParseCurrency)?)
                .await?;
            let valuta = german_string_to_date(&cap[2])?;
            Ok((CashAmount { amount, currency }, valuta))
        }
        None => Err(ReadPDFError::NotFound("pre-tax amount")),
    }
}

async fn add_or_append(
    payments: &mut Vec<CashAmount>,
    regex: &Regex,
    text: &str,
    factor: f64,
    market: Market,
) -> Result<()> {
    let new_payment = parse_amount(regex, text, &market).await?;
    if new_payment.is_none() {
        return Ok(());
    }
    let new_payment = new_payment.unwrap();
    for payment in (*payments).iter_mut() {
        if payment.currency == new_payment.currency {
            payment.amount += factor * new_payment.amount;
            return Ok(());
        }
    }
    payments.push(CashAmount {
        amount: factor * new_payment.amount,
        currency: new_payment.currency,
    });
    Ok(())
}

async fn parse_payment_components(
    payments: &mut Vec<CashAmount>,
    regex_vec: &[Regex],
    text: &str,
    factor: f64,
    market: &Market,
) -> Result<()> {
    for regex in regex_vec {
        add_or_append(payments, regex, text, factor, market.clone()).await?;
    }
    Ok(())
}

/// Extract transaction information from text files
pub async fn parse_document(text: &str, market: &Market) -> Result<ParsedInfo> {
    lazy_static! {
        static ref TOTAL_POSITION: Regex = Regex::new(
            r"Summe\s+St.\s+([0-9.,]+)\s+[A-Z]{3}\s+[0-9,.]+\s+([A-Z]{3})\s+([-0-9,.]+)"
        )
        .unwrap();
        static ref BOND_POSITION: Regex = Regex::new(r"[A-Z]{3}\s+([0-9.,]+)\s+[0-9.,]+%").unwrap();
        static ref POSITION: Regex = Regex::new(r"Stk?.\s+([0-9.,]+)").unwrap();
        static ref TRADE_VALUE: Regex =
            Regex::new(r"Kurswert\s*:\s+([A-Z]{3})\s+([-0-9,.]*)").unwrap();
        static ref DIV_PRE_TAX: Regex =
            Regex::new(r"Bruttobetrag\s*:\s+([A-Z]{3})\s+([-0-9,.]*)").unwrap();
        static ref TOTAL_AMOUNT: Regex = Regex::new(
            r"Zu Ihren (?:Lasten|Gunsten) nach Steuern:\*?\*?\s*\*?\*?([A-Z]{3}) *([-0-9.,]+)"
        )
        .unwrap();
        static ref BOND_PAYBACK: Regex =
            Regex::new(r"Kurswert Einlösung\s+([A-Z]{3}) *([-0-9.,]+)").unwrap();
        static ref PAID_TAX: Regex =
            Regex::new(r"(?:abgeführte|erstattete) Steuern\s+([A-Z]{3})\s+([-0-9,.]+)\s+").unwrap();
        static ref COMDIRECT_FEES: Vec<Regex> = vec![
            Regex::new(r"(?:Gesamtprovision|Provision)\s*:\s+([A-Z]{3})\s+([-0-9,.]*)").unwrap(),
            Regex::new(r"Börsenplatzabhäng. Entgelt\s*:\s+([A-Z]{3})\s+([-0-9,.]*)").unwrap(),
            Regex::new(r"Variable Börsenspesen\s*:\s+([A-Z]{3})\s+([-0-9,.]*)").unwrap(),
            Regex::new(r"Umschreibeentgelt\s*:\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap(),
            Regex::new(r"[fF]remde Spesen\s*:?\s+([A-Z]{3})\s+([-0-9,.]+ ?-?)").unwrap(),
            Regex::new(r"Maklercourtage\s*:\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap(),
            Regex::new(r"Reduktion Kaufaufschlag\s*:?\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap(),
            Regex::new(r"\n\s*Entgelte\s*:\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap(),
            Regex::new(r"Abwickl.entgelt Clearstream\s*:\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap(),
            Regex::new(r"Provision für Steuererstattung\s+([A-Z]{3})\s+([-0-9,.]+ ?-?)").unwrap(),
        ];
        static ref COMDIRECT_TAXES: Vec<Regex> = vec![
            Regex::new(
                r"Mehrwertsteuer auf\s+[A-Z]{3}\s+[-0-9,.]+\s+[`]*\s*([A-Z]{3})\s+([-0-9,.]+\s?-?)"
            )
            .unwrap(),
            Regex::new(r"Kapitalertragsteuer\s*\(?[0-9]?\)?\s+[`]*\s*([A-Z]{3})\s+([-0-9,.]+)")
                .unwrap(),
            Regex::new(r"Solidaritätszuschlag\s+[`]*\s*([A-Z]{3})\s+([-0-9,.]+)").unwrap(),
            Regex::new(r"(?m)Kirchensteuer\s+[`]*\s*([A-Z]{3})\s*\n\s*_*\s*\n\s*+([-0-9,.]+)")
                .unwrap(),
            Regex::new(r"Quellensteuer\s+[`]*\s*([A-Z]{3})\s+([-0-9,.]+ ?-?)").unwrap(),
            Regex::new(r"Quellensteuervergütung\s+[`]*\s*([A-Z]{3})\s+([-0-9,.]+ ?-?)").unwrap(),
        ];
        static ref COMDIRECT_ACCRUALS: Vec<Regex> =
            vec![Regex::new(r"[0-9]+\s+Tage Zinsen\s+:\s*([A-Z]{3})\s+([-0-9,.]+)").unwrap(),];
        static ref AMENDMENT: Regex = Regex::new(r"Nachtragsabrechnung").unwrap();
    }

    let doc_type = parse_doc_type(text)?;
    trace!("DocType: {doc_type:?}");
    if doc_type == DocumentType::Finanzreport {
        return finanzreport::parse_report(text);
    }
    let mut asset_info = parse_asset(doc_type, text)?;
    trace!("AssetInfo: {asset_info:?}");
    let is_amendment = AMENDMENT.is_match(text);
    trace!("is_amendment: {is_amendment:?}");

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
                let currency = market.get_currency_from_str(&position[2]).await?;
                pre_tax_fee_value = Some(CashAmount { amount, currency });
                Some(german_string_to_float(&position[1])?)
            }
        };
    }

    let (pre_tax, valuta) = parse_pre_tax(text, doc_type, market).await?;
    trace!("pre_tax: {pre_tax}, valuta: {valuta}");

    if pre_tax_fee_value.is_none() {
        pre_tax_fee_value = match doc_type {
            DocumentType::Buy | DocumentType::Sell => {
                parse_amount(&TRADE_VALUE, text, market).await?
            }
            DocumentType::Dividend | DocumentType::Interest => {
                parse_amount(&DIV_PRE_TAX, text, market).await?
            }
            DocumentType::Tax => Some(pre_tax),
            DocumentType::BondPayBack => parse_amount(&BOND_PAYBACK, text, market).await?,
            DocumentType::Finanzreport => None,
        };
    }
    let pre_tax_fee_value = must_have(pre_tax_fee_value, "can't find value before taxes and fees")?;

    // Determine final value
    let total_amount = match doc_type {
        DocumentType::Sell | DocumentType::Buy => parse_amount(&TOTAL_AMOUNT, text, market).await?,
        DocumentType::Dividend | DocumentType::Interest | DocumentType::BondPayBack => {
            Some(pre_tax)
        }
        DocumentType::Tax => parse_amount(&PAID_TAX, text, market).await?,
        DocumentType::Finanzreport => None,
    };
    let total_amount = must_have(total_amount, "can't identify total payment amount")?;
    trace!("total amount: {total_amount}");
    let (fx_rate, _) = parse_fx_rate(text, market).await?;
    trace!("fx_rate: {fx_rate:?}");

    // Collect essential informations in ParsedTransactionInfo
    let tri = match doc_type {
        DocumentType::Buy | DocumentType::Sell | DocumentType::BondPayBack => {
            let sign = if doc_type == DocumentType::Buy {
                -1.0
            } else {
                1.0
            };
            if asset_info.position.is_none() {
                return Err(ReadPDFError::NotFound("position"));
            }
            let main_amount = CashAmount {
                amount: sign * pre_tax_fee_value.amount,
                currency: pre_tax_fee_value.currency,
            };
            let mut tri = ParsedTransactionInfo::new(
                doc_type,
                asset_info.asset,
                main_amount,
                total_amount,
                fx_rate,
                valuta,
            );
            tri.position = -sign * asset_info.position.unwrap();
            Some(tri)
        }
        DocumentType::Dividend => {
            if is_amendment {
                // foreign tax pay back
                let mut tri = ParsedTransactionInfo::new(
                    DocumentType::Tax,
                    asset_info.asset,
                    pre_tax,
                    total_amount,
                    fx_rate,
                    valuta,
                );
                tri.note = Some("foreign tax pay back\n".to_string());
                Some(tri)
            } else {
                Some(ParsedTransactionInfo::new(
                    doc_type,
                    asset_info.asset,
                    pre_tax_fee_value,
                    total_amount,
                    fx_rate,
                    valuta,
                ))
            }
        }
        DocumentType::Tax => Some(ParsedTransactionInfo::new(
            DocumentType::Tax,
            asset_info.asset,
            total_amount,
            total_amount,
            fx_rate,
            valuta,
        )),
        DocumentType::Interest => Some(ParsedTransactionInfo::new(
            doc_type,
            asset_info.asset,
            pre_tax_fee_value,
            total_amount,
            fx_rate,
            valuta,
        )),
        DocumentType::Finanzreport => None,
    };
    let mut tri = tri.ok_or(ReadPDFError::UnknownDocumentType)?;

    if !is_amendment {
        let sign = if tri.doc_type == DocumentType::Buy {
            -1.0
        } else {
            1.0
        };
        parse_payment_components(&mut tri.extra_fees, &COMDIRECT_FEES, text, sign, market).await?;
        if tri.doc_type != DocumentType::Tax {
            // Already in main payment included if document is of type tax
            parse_payment_components(&mut tri.extra_taxes, &COMDIRECT_TAXES, text, 1.0, market)
                .await?;
        }
        parse_payment_components(&mut tri.accruals, &COMDIRECT_ACCRUALS, text, -1.0, market)
            .await?;
    }

    Ok(Arc::new(tri))
}

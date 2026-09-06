//! Parse text files and extract asset and transaction data
//! Currently, transaction documents from comdirect bank are supported
use super::german_string_to_date;
use super::german_string_to_float;
use super::{DocumentType, ParsedTransactionInfo, ReadPDFError};
use finql::{
    datatypes::{Asset, CashAmount, CurrencyISOCode, Stock},
    Market,
};
use lazy_static::lazy_static;
use regex::{Regex, RegexSet};
use time::Date;

struct AssetInfo {
    asset: Asset,
    // reserved for later use; could also be ex-interest date
    _ex_div_day: Option<Date>,
    // interest rate of a bond, for later use
    _interest_rate: Option<f64>,
    position: Option<f64>,
}

/// Extract asset information from text file
fn parse_asset(doc_type: DocumentType, text: &str) -> Result<AssetInfo, ReadPDFError> {
    lazy_static! {
        static ref NAME_WKN_ISIN: Regex = Regex::new(
            r"(?m)WPKNR/ISIN\n(.*)\s\s\s*([A-Z0-9]{6})\s*\n\s*(.*)\s\s\s*([A-Z0-9]{12})"
        )
        .unwrap();
        // Search for asset in dividend documents
        static ref NAME_WKN_ISIN_DIV: Regex = Regex::new(
            r"(?m)WKN/ISIN\n\s*per\s+([.0-9]{10})\s+(.*)\s+([A-Z0-9]{6})\s*\n\s*STK\s+([.,0-9]*)\s+(.*)\s\s\s*([A-Z0-9]{12})"
        )
        .unwrap();
        // Search for asset in interest documents
        static ref NAME_WKN_ISIN_INT: Regex = Regex::new(
            r"(?m)WKN/ISIN\n\s*per\s+([.0-9]{10})\s+([-.,0-9]+)\s+(.*)\s+([A-Z0-9]{6})\s*\n\s*[A-Z]{3}\s+([.,0-9]*)\s+(.*)\s\s\s*([A-Z0-9]{12})"
        )
        .unwrap();
        // Search for asset in tax documents
        static ref WKN_ISIN_TAX: Regex = Regex::new(r"WKN\s*/\s*ISIN:\s+([A-Z0-9]{6})\s*/\s*([A-Z0-9]{12})").unwrap();
    }

    match doc_type {
        DocumentType::Interest | DocumentType::BondPayBack => {
            match NAME_WKN_ISIN_INT.captures(text) {
                Some(cap) => {
                    let wkn = Some(cap[4].to_string());
                    let isin = Some(cap[7].to_string());
                    let name = format!("{} {}", cap[3].trim(), cap[6].trim());
                    let ex_div_day = Some(german_string_to_date(&cap[1])?);
                    let position = Some(german_string_to_float(&cap[5])?);
                    let interest_rate = Some(german_string_to_float(&cap[2])?);
                    if true {
                        println!("Debug: Found asset in Interest or BondPayBack with wkn: {:?}, isin: {:?}, name: '{:?}', ex_div_day: {:?}, position: {:?}, inerest rate: {:?}",
                            wkn, isin, name, ex_div_day, position, interest_rate);
                    }
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
            match WKN_ISIN_TAX.captures(text) {
                // The document does not provide the full name, leave name empty and search in database by ISIN/WKN
                Some(cap) => {
                    let wkn = Some(cap[1].to_string());
                    let isin = Some(cap[2].to_string());
                    if true {
                        println!(
                            "Debug: Found asset in Tax info with wkn: {:?}, isin: {:?}",
                            wkn, isin
                        );
                    }
                    Ok(AssetInfo {
                        asset: Asset::Stock(Stock::new(None, String::new(), isin, wkn, None)),
                        _ex_div_day: None,
                        _interest_rate: None,
                        position: None,
                    })
                }
                None => Err(ReadPDFError::NotFound("asset")),
            }
        }
        DocumentType::Dividend => match NAME_WKN_ISIN_DIV.captures(text) {
            Some(cap) => {
                let wkn = Some(cap[3].to_string());
                let isin = Some(cap[6].to_string());
                let name = format!("{} {}", cap[2].trim(), cap[5].trim());
                let ex_div_day = Some(german_string_to_date(&cap[1])?);
                let position = Some(german_string_to_float(&cap[4])?);
                if true {
                    println!("Debug: Found asset in Dividend note with wkn: {:?}, isin: {:?}, name: '{:?}', ex_div_day: {:?}, position: {:?}",
                        wkn, isin, name, ex_div_day, position);
                }
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

async fn parse_amount(
    regex: &Regex,
    text: &str,
    market: &Market,
) -> Result<Option<CashAmount>, ReadPDFError> {
    match regex.captures(text) {
        None => Ok(None),
        Some(cap) => {
            let amount = german_string_to_float(&cap[2])?;
            let currency = market.get_currency_from_str(&cap[1]).await?;
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

async fn parse_fx_rate(
    text: &str,
    market: &Market,
) -> Result<(Option<f64>, Option<CashAmount>), ReadPDFError> {
    lazy_static! {
        static ref EXCHANGE_RATE: Regex = Regex::new(
            r"Umrechn. zum Dev. kurs\s+([0-9,.]*)\s+vom\s+[0-9.]*\s+:\s+([A-Z]{3})\s+([-0-9,.]+)"
        )
        .unwrap();
        static ref EXCHANGE_RATE_DIV: Regex =
            Regex::new(r"zum Devisenkurs:\s+[A-Z/]{7}\s+([0-9,.]+)\s\s+([A-Z]{3})\s+([-0-9,.]+)")
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
    println!(
        "Debug: Parsing FX-rate: {} {} {}",
        &cap[1], &cap[2], &cap[3]
    );
    let fx_rate = german_string_to_float(&cap[1])?;
    let amount = german_string_to_float(&cap[3])?;
    let currency = market
        .get_currency(CurrencyISOCode::new(&cap[2]).map_err(ReadPDFError::ParseCurrency)?)
        .await?;

    Ok((Some(fx_rate), Some(CashAmount { amount, currency })))
}

fn parse_doc_type(text: &str) -> Result<DocumentType, ReadPDFError> {
    lazy_static! {
        static ref DOC_TYPE_SET: RegexSet = RegexSet::new([
            r"(?m)^\s*Wertpapierkauf",
            r"(?m)^\s*Wertpapierverkauf",
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
) -> Result<(CashAmount, Date), ReadPDFError> {
    lazy_static! {
        static ref PRE_TAX_AMOUNT: Regex = Regex::new(
            r"(?m)Zu Ihren (?:Gunsten|Lasten) vor Steuern\s*\n.*\s*([0-9.]{10})\s*([A-Z]{3})\s*([-0-9.,]+)"
        )
        .unwrap();
        static ref PRE_TAX_AMOUNT_TAX: Regex =
            Regex::new(r"Zu Ihren Gunsten vor Steuern:\s*([A-Z]{3})\s*([-0-9.,]+)").unwrap();
        static ref VALUTA: Regex = Regex::new(r"erfolgt mit Valuta\s*([0-9.]{10})").unwrap();
        static ref VALUTA_ALT: Regex = Regex::new(r"Datum:\s+([0-9.]{10})").unwrap();
    }

    if doc_type == DocumentType::Tax {
        return match PRE_TAX_AMOUNT_TAX.captures(text) {
            None => Err(ReadPDFError::NotFound("pre-tax amount")),
            Some(cap) => {
                println!("Debug: Pre Tax amount: {} {}", &cap[2], &cap[1]);
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
        None => Err(ReadPDFError::NotFound("pre-tax amount")),
        Some(cap) => {
            println!(
                "Debug: Pre Tax amount: {} {} at {}",
                &cap[3], &cap[2], &cap[1]
            );
            let amount = german_string_to_float(&cap[3])?;
            let currency = market
                .get_currency(CurrencyISOCode::new(&cap[2]).map_err(ReadPDFError::ParseCurrency)?)
                .await?;
            let valuta = german_string_to_date(&cap[1])?;
            Ok((CashAmount { amount, currency }, valuta))
        }
    }
}

async fn add_or_append(
    payments: &mut Vec<CashAmount>,
    regex: &Regex,
    text: &str,
    factor: f64,
    market: Market,
) -> Result<(), ReadPDFError> {
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
) -> Result<(), ReadPDFError> {
    for regex in regex_vec {
        add_or_append(payments, regex, text, factor, market.clone()).await?;
    }
    Ok(())
}

/// Extract transaction information from text files
pub async fn parse_transactions(
    text: &str,
    market: &Market,
) -> Result<ParsedTransactionInfo, ReadPDFError> {
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
        static ref TOTAL_AMOUNT: Regex =
            Regex::new(r"Zu Ihren (?:Lasten|Gunsten) nach Steuern: *([A-Z]{3}) *([-0-9.,]+)")
                .unwrap();
        static ref BOND_PAYBACK: Regex =
            Regex::new(r"Kurswert Einlösung\s+([A-Z]{3}) *([-0-9.,]+)").unwrap();
        static ref PAID_TAX: Regex = Regex::new(
            r"(?m)(?:abgeführte|erstattete) Steuern\s+([A-Z]{3}).*\n.*\n\s+([-0-9,.]+ ?-?)$"
        )
        .unwrap();
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
            Regex::new(r"Mehrwertsteuer auf\s+[A-Z]{3}\s+[-0-9,.]+\s+([A-Z]{3})\s+([-0-9,.]+ ?-?)")
                .unwrap(),
            Regex::new(r"Kapitalertragsteuer\s*\(?[0-9]?\)?\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap(),
            Regex::new(r"Solidaritätszuschlag\s+([A-Z]{3})\s+([-0-9,.]+)").unwrap(),
            Regex::new(r"(?m)Kirchensteuer\s+([A-Z]{3})\s*\n\s*_*\s*\n\s*+([-0-9,.]+)").unwrap(),
            Regex::new(r"Quellensteuer\s+([A-Z]{3})\s+([-0-9,.]+ ?-?)").unwrap(),
            Regex::new(r"Quellensteuervergütung\s+([A-Z]{3})\s+([-0-9,.]+ ?-?)").unwrap(),
        ];
        static ref COMDIRECT_ACCRUALS: Vec<Regex> =
            vec![Regex::new(r"[0-9]+\s+Tage Zinsen\s+:\s*([A-Z]{3})\s+([-0-9,.]+)").unwrap(),];
        static ref AMENDMENT: Regex = Regex::new(r"Nachtragsabrechnung").unwrap();
    }

    let doc_type = parse_doc_type(text)?;
    let mut asset_info = parse_asset(doc_type, text)?;
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
                let currency = market.get_currency_from_str(&position[2]).await?;
                pre_tax_fee_value = Some(CashAmount { amount, currency });
                Some(german_string_to_float(&position[1])?)
            }
        };
    }

    let (pre_tax, valuta) = parse_pre_tax(text, doc_type, market).await?;

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
    };
    let total_amount = must_have(total_amount, "can't identify total payment amount")?;
    let (fx_rate, _) = parse_fx_rate(text, market).await?;

    // Collect essential informations in ParsedTransactionInfo
    let mut tri = match doc_type {
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
            tri
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
                tri
            } else {
                ParsedTransactionInfo::new(
                    doc_type,
                    asset_info.asset,
                    pre_tax_fee_value,
                    total_amount,
                    fx_rate,
                    valuta,
                )
            }
        }
        DocumentType::Tax => ParsedTransactionInfo::new(
            DocumentType::Tax,
            asset_info.asset,
            total_amount,
            total_amount,
            fx_rate,
            valuta,
        ),
        DocumentType::Interest => ParsedTransactionInfo::new(
            doc_type,
            asset_info.asset,
            pre_tax_fee_value,
            total_amount,
            fx_rate,
            valuta,
        ),
    };

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

    Ok(tri)
}

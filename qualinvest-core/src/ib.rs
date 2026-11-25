use ib::read::KontoauszugData;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::{io, num, string};

use thiserror::Error;

use log::{debug, info};
use sanitize_filename::sanitize;

use crate::read_pdf::ReadPDFError;
use finql::{
    Market,
    datatypes::{Asset, CurrencyError, DataError, Transaction},
};

type Result<T> = std::result::Result<T, ReadPDFError>;

use super::accounts::{Account, AccountHandler};
use crate::PdfParseParams;

pub async fn store_parsed_csv(
    hash: &str,
    file_name: &str,
    data: KontoauszugData,
    db: Arc<dyn AccountHandler + Send + Sync>,
    config: &PdfParseParams,
) -> Result<i32> {
    debug!("store_parsed_pdf called");
    Ok(0)
}

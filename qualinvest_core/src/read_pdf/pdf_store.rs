use super::ReadPDFError;
use crate::PdfParseParams;
use data_encoding::HEXUPPER;
use ring::digest::{Context, SHA256};
use std::fs::{copy, File};
use std::io::{BufReader, Read};
use std::path::Path;

pub fn sha256_hash(file: &Path) -> Result<String, ReadPDFError> {
    let input = File::open(file)?;
    let mut reader = BufReader::new(input);

    let mut context = Context::new(&SHA256);
    let mut buffer = Vec::new();
    // read the whole file
    reader.read_to_end(&mut buffer)?;
    context.update(&buffer);

    let digest = context.finish();
    let hash = HEXUPPER.encode(digest.as_ref());
    Ok(hash)
}

/// Store a copy of the pdf file in the path specified in the configuration.
/// The pdf will be stored with the given name, which is assumed to have been sanitized.
pub async fn store_pdf_as_name(
    pdf_path: &Path,
    name: &str,
    _hash: &str,
    config: &PdfParseParams,
) -> Result<(), ReadPDFError> {
    let new_path = Path::new(&config.doc_path).join(name);
    println!("Debug: new doc path: {}", new_path.display());
    match copy(pdf_path, &new_path) {
        Ok(_) => Ok(()),
        Err(e) => Err(ReadPDFError::IoError(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash() {
        let file = "LICENSE-MIT";
        let hash = sha256_hash(&Path::new("..").join(file)).unwrap();
        assert_eq!(
            hash,
            "0998C58A8B2993EA0B3AA8EBAF260606A8F84F3C1005F060A72C814199BDD0BA".to_string()
        );
    }
}

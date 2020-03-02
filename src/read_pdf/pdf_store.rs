use super::ReadPDFError;
use crate::Config;
use data_encoding::HEXUPPER;
use ring::digest::{Context, SHA256};
use std::fs;
use std::fs::File;
use std::path::Path;
use std::io::{BufReader,Read};

pub fn sha256_hash(file: &str) -> Result<String, ReadPDFError> {
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

pub fn store_pdf(pdf_file: &str, _hash: &str, config: &Config) -> Result<String,ReadPDFError> {
    let path = Path::new(pdf_file);
    let name = path.file_name()
      .ok_or(ReadPDFError::NotFound("no valid file name") )?.to_string_lossy();
    let new_path = format!("{}/{}",&config.doc_path, name);
    fs::copy(pdf_file, &new_path)?;
    Ok(new_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash() {
        let file = "Cargo.toml";
        let hash = sha256_hash(file).unwrap();
        assert_eq!(
            hash,
            "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855".to_string()
        );
    }
}

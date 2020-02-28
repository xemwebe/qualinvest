use data_encoding::HEXUPPER;
use ring::digest::{Context, SHA256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::fs;
use super::ReadPDFError;

pub fn sha256_hash(file: &str) -> Result<String, ReadPDFError> {
    let metadata = fs::metadata(file);
    let file_size = metadata?.len();
  
    let input = File::open(file)?;
    let mut reader = BufReader::new(input);

    let mut context = Context::new(&SHA256);
    let mut buffer = Vec::with_capacity(file_size as usize);

    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        context.update(&buffer[..count]);
    }

    let digest = context.finish();
    let hash = HEXUPPER.encode(digest.as_ref());
    Ok(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash() {
        let file = "Cargo.toml";
        let hash = sha256_hash(file).unwrap();
        assert_eq!(hash, "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855".to_string());
    }
}
/// Read pdf files and transform into plain text
/// This requires the extern tool `pdftotext` 
/// which is part of [XpdfReader](https://www.xpdfreader.com/pdftotext-man.html).

use std::process::Command;
use std::error::Error;
use std::fmt;
use std::string;
use std::io;

#[derive(Debug)]
pub enum ReadPDFError {
    IoError(io::Error),
    ParseError(string::FromUtf8Error)
}

impl fmt::Display for ReadPDFError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Conversion of pdf to text failed.")
    }
}

impl Error for ReadPDFError {
    fn cause(&self) -> Option<&dyn Error> {
        Some(self)
    }
}

impl From<std::string::FromUtf8Error> for ReadPDFError {
    fn from(error: string::FromUtf8Error) -> Self {
        Self::ParseError(error)
    }
}

impl From<io::Error> for ReadPDFError {
    fn from(error: io::Error) -> Self {
        Self::IoError(error)
    }
}

pub fn text_from_pdf(file: &str) -> Result<String, ReadPDFError> {
    let output = Command::new("pdftotext")
    .arg("-layout")
    .arg("-q")
    .arg(&file)
    .arg("-")
    .output()?;
    Ok(String::from_utf8(output.stdout)?)
}

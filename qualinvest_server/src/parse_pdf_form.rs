use bytes::Bytes;
use futures::stream::Stream;
use rocket::data::DataStream;

// Import multer types.
use multer::Multipart;
use std::convert::Infallible;
use futures::stream::once;
use finql::postgres_handler::PostgresDB;

struct PdfList {
    tmp_path: String,
    file_name: String,
}

pub enum PdfFormParseError {
    StoreTempFailed,
    MalFormedForm,
}

pub async fn parse_multipart(stream: DataStream, boundary: &str, out: &mut Vec<String>) -> Result<(), PdfFormParseError> {

    // Create a `Multipart` instance from that byte stream and the boundary.
    let mut multipart = Multipart::with_reader(stream, boundary);

    // Iterate over the fields, use `next_field()` to get the next field.
    while let Some(mut field) = multipart.next_field().await
        .map_err(|_| { PdfFormParseError::MalFormedForm })? {
        // Get field name.
        let name = field.name();
        // Get the field's filename if provided in "Content-Disposition" header.
        let file_name = field.file_name();


        // Process the field data chunks e.g. store them in a file.
        while let Some(_) = field.chunk().await
            .map_err(|_| { PdfFormParseError::MalFormedForm })? {
        }
    }
    Ok(())
}

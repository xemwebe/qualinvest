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

        println!("Name: {:?}, File Name: {:?}", name, file_name);

        // Process the field data chunks e.g. store them in a file.
        while let Some(chunk) = field.chunk().await
            .map_err(|_| { PdfFormParseError::MalFormedForm })? {
            // Do something with field chunk.
            println!("Chunk: {:?}", chunk);
        }
    }
    Ok(())
}

// extern crate multipart;

// use multipart::server::Multipart;
// use multipart::server::save::Entries;
// use multipart::server::save::SaveResult::*;
// use rocket;

// pub fn parse_multipart(stream: rocket::Datastream, boundary: String, out: &mut Vec<String>, db: &mut PostgresDB) {
//     match Multipart::with_body(stream, boundary).save().temp() {
//         Full(entries) => process_entries(entries, &db, &state.doc_path, &mut out),
//         _ => out.push(format!("{}{}", state.rel_path, uri!(error_msg: msg="Invalid document type"))),
//     }
// }

// /// Process the multi-part pdf upload and return a string of error messages, if any.
// fn process_entries(entries: Entries, db: &mut PostgresDB, doc_path: &String, errors: &mut Vec<String>) {
//     let mut pdf_config = qualinvest_core::PdfParseParams{
//         doc_path: doc_path.clone(),
//         warn_old: false,
//         consistency_check: false,
//         rename_asset: false,
//         default_account: None,
//     };

//     // List of documents contains tuple of file name and full path
//     let mut documents = Vec::new();
//     for (name, field) in entries.fields {
//         match &*name {
//             "default_account" => {
//                 match &field[0].data {
//                     multipart::server::save::SavedData::Text(account) => 
//                         pdf_config.default_account = account.parse::<usize>().ok(),
//                     _ => errors.push("Invalid default account setting".to_string()),
//                 }
//             },
//             "warn_old" => pdf_config.warn_old = true,
//             "consistency_check" => pdf_config.consistency_check = true,
//             "rename_asset" => pdf_config.rename_asset = true,
//             "doc_name" => {
//                 for f in &field {
//                     match &f.data {
//                         multipart::server::save::SavedData::File(path, _size) => {
//                             let doc_name = &f.headers.filename;
//                             if doc_name.is_none() || &f.headers.content_type != &Some(mime::APPLICATION_PDF) {
//                                 errors.push("Invalid pdf document".to_string());
//                             } else {
//                                 documents.push( (path.clone(), doc_name.as_ref().unwrap().clone()) );
//                             }
//                         },
//                         _ => errors.push("Invalid pdf document".to_string()),
//                     }    
//                 }
//             },
//             _ => errors.push("Unsupported field name".to_string()),
//         }
//     }

//     // call pdf parse for each pdf found
//     for (path, pdf_name) in documents {
//         let transactions = parse_and_store(&path, &pdf_name, db, &pdf_config);
//         match transactions {
//             Err(err) => {
//                 errors.push(format!("Failed to parse file {} with error {:?}", pdf_name, err));
//             }
//             Ok(count) => {
//                 errors.push(format!("{} transaction(s) stored in database.", count));
//             }
//         }
//     }
// }

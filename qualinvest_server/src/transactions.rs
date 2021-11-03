/// Viewing and editing of transactions
use tempfile::TempDir;
use rocket::State;
use rocket::response::Redirect;
use rocket_dyn_templates::Template;
use rocket::form::{Form, FromForm};
use rocket::fs::TempFile;

use qualinvest_core::accounts::{Account,AccountHandler};
use qualinvest_core::user::UserHandler;
use finql_data::{Transaction,TransactionType,TransactionHandler,AssetHandler,CashAmount,CashFlow,Currency};
use qualinvest_core::PdfParseParams;
use qualinvest_core::read_pdf::{parse_and_store};
use crate::user::UserCookie;
use crate::layout::layout;
use crate::form_types::NaiveDateForm;
use std::str::FromStr;
use super::rocket_uri_macro_login;
use super::ServerState;

#[get("/transactions?<err_msg>")]
pub async fn transactions(err_msg: Option<String>, user_opt: Option<UserCookie>, state: &State<ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(uri!(ServerState::base(), login(Some("transactions")))));
    }
    let user = user_opt.unwrap();

    let db = state.postgres_db.clone();
    let user_settings = db.get_user_settings(user.userid).await;
    let mut context = state.default_context();
    if let Ok(transactions) = db.get_transaction_view_for_accounts(&user_settings.account_ids).await {
        context.insert("transactions", &transactions);
    } else {
        if err_msg.is_none() {
            context.insert("err_msg", "Failed to get list of transactions!");
        } else {
            context.insert("err_msg", &err_msg);
        }
    }

    context.insert("user", &user);
    Ok(layout("transactions", &context.into_json()))
}


/// Structure for storing information in transaction formular
#[derive(Debug,Serialize,Deserialize,FromForm)]
pub struct TransactionForm {
    pub id: Option<usize>,
    pub asset_id: Option<usize>,
    pub position: Option<f64>,
    pub trans_type: String,
    pub cash_amount: f64,
    pub currency: String,
    pub date: NaiveDateForm,
    pub note: Option<String>,
    pub trans_ref: Option<usize>,
    pub account_id: usize,
}


impl TransactionForm {
    fn from(t: &Transaction, account: &Account) -> Result<TransactionForm,&'static str> {
        let account_id = account.id.ok_or("Invalid account")?;
        let mut tf = TransactionForm {
            id: t.id,
            note: t.note.as_ref().cloned(),
            cash_amount: t.cash_flow.amount.amount,
            date: NaiveDateForm::new(t.cash_flow.date),
            currency: t.cash_flow.amount.currency.to_string(),
            asset_id: None,
            trans_type: "".to_string(),
            trans_ref: None,
            account_id : account_id,
            position: None};
    
        match t.transaction_type {
            TransactionType::Asset{asset_id, position} => {
                tf.trans_type = "a".to_string();
                tf.asset_id = Some(asset_id);
                tf.position = Some(position);
            },
            TransactionType::Dividend{asset_id} => {
                tf.trans_type = "d".to_string();
                tf.asset_id = Some(asset_id);
            },
            TransactionType::Interest{asset_id} => {
                tf.trans_type = "i".to_string();
                tf.asset_id = Some(asset_id);
            },
            TransactionType::Fee{transaction_ref} => {
                tf.trans_type = "f".to_string();
                tf.trans_ref = transaction_ref;
            },
            TransactionType::Tax{transaction_ref} => {
                tf.trans_type = "t".to_string();
                tf.trans_ref = transaction_ref;
            },
            TransactionType::Cash => {
                tf.trans_type = "c".to_string();
            },
        }
        Ok(tf)        
    }

    fn to_transaction(&self) -> Result<Transaction,&'static str> {
        let trans_type = match self.trans_type.as_str() {
            "a" => {
                if self.asset_id.is_none() || self.position.is_none() { return Err("malformed_transaction"); }
                TransactionType::Asset{asset_id: self.asset_id.unwrap(), position: self.position.unwrap() }
            },
            "d" => TransactionType::Dividend{asset_id: self.asset_id.ok_or("malformed_transaction")?},
            "i" => TransactionType::Interest{asset_id: self.asset_id.ok_or("malformed_transaction")?},
            "f" => TransactionType::Fee{transaction_ref: self.trans_ref},
            "t" => TransactionType::Tax{transaction_ref: self.trans_ref},
            "c" => TransactionType::Cash,
            _ => {
                return Err("malformed_transaction");
            }
        };
        let currency = Currency::from_str(&self.currency)
            .map_err(|_| "malformed_transaction")?;
        let t = Transaction{
            id: self.id,
            transaction_type: trans_type,
            cash_flow: CashFlow{
                amount: CashAmount{
                    amount: self.cash_amount,
                    currency,
                },
                date: self.date.date,
            },
            note: self.note.as_ref().cloned(),
        };
        Ok(t)
    } 
}

#[get("/transactions/edit?<transaction_id>&<err_msg>")]
pub async fn edit_transaction(transaction_id: Option<usize>, err_msg: Option<String>, user: UserCookie, state: &State<ServerState>) -> Result<Template,Redirect> {
    let db = state.postgres_db.clone();
    let mut context = state.default_context();
    context.insert("user", &user);

    if err_msg.is_some() {
        context.insert("err_msg", &err_msg);
        return Ok(layout("transaction_form", &context.into_json()));
    }

    if let Some(user_accounts) = user.get_accounts(db.clone()).await {
        context.insert("valid_accounts", &user_accounts);
    } else {
        context.insert("err_msg", "You need an account before you can add transactions");
    }

    if let Some(trans_id) = transaction_id {
        if let Ok(account) = db.get_transaction_account_if_valid(trans_id, user.userid).await {
            if let Ok(transaction) = db.get_transaction_by_id(trans_id).await {
                if let Ok(transaction) = TransactionForm::from(&transaction, &account) {
                    context.insert("transaction", &transaction);
                } else {
                    context.insert("err_msg", "Failed to read transaction");
                }
            } else {
                context.insert("err_msg", "Invalid transaction ID");
            }
        } else {
            context.insert("err_msg", "Account is invalid");
        }
    }
    
    if let Ok(assets) = db.get_all_assets().await {
        context.insert("assets", &assets);
    } else {
        context.insert("err_msg", "Could not find any assets");
    }

    if let Ok(currencies) = db.get_all_currencies().await {
        context.insert("currencies", &currencies);
    } else {
        context.insert("err_msg", "No currencies have been defined yet");
    }

    Ok(layout("transaction_form", &context.into_json()))
}

#[get("/transactions/upload?<error_msg>")]
pub async fn pdf_upload_form(error_msg: Option<String>, user: UserCookie, state: &State<ServerState>) -> Result<Template,Redirect> {
    let db = state.postgres_db.clone();
    let mut context = state.default_context();   
    context.insert("user", &user);

    if let Some(message) = error_msg {
        context.insert("err_msg", &message);
        return Ok(layout("pdf_upload", &context.into_json()));
    }

    if let Some(user_accounts) = user.get_accounts(db.clone()).await {
        context.insert("accounts", &user_accounts);
    } else {
        context.insert("err_msg", "You have no accounts setup");
    }

    let default_account_id: Option<usize> = None;
    context.insert("default_account_id", &default_account_id);
    Ok(layout("pdf_upload", &context.into_json()))
}

/// Structure for storing information in transaction formular
#[derive(Debug,Serialize,Deserialize,FromForm)]
pub struct UploadForm {
    pub doc_path: String,
    pub is_directory: bool,
    pub warn_old: bool,
    pub default_account: Option<usize>,
    pub consistency_check: bool,
    pub rename_asset: bool,
}

#[derive(Debug, FromForm)]
pub struct PDFUploadFormData<'v> {
    pub warn_old: bool,
    pub consistency_check: bool,
    pub rename_asset: bool,
    pub default_account: Option<usize>,
    pub doc_name: Vec<TempFile<'v>>, 
}

/// Structure for storing pdf upload errors
#[derive(Debug, Serialize)]
pub struct UploadError {
    file_name: String,
    message: String,
}

#[post("/pdf_upload", data="<data>")]
/// Uploading pdf documents via web form
pub async fn pdf_upload(mut data: Form<PDFUploadFormData<'_>>, user: UserCookie, state: &State<ServerState>) 
-> Result<Template, Redirect> {
    let pdf_config = PdfParseParams{
        doc_path: state.doc_path.clone(),
        warn_old: data.warn_old,
        consistency_check: data.consistency_check,
        rename_asset: data.rename_asset,
        default_account: data.default_account,
    };
    if ! user.is_admin {
        return Err(Redirect::to(uri!(ServerState::base(), super::index(Some("Only admins may upload pdf files!")))));
    }
    // parse each each pdf found
    let mut errors = Vec::new();
    let tmp_dir = TempDir::new()
        .map_err(|_| Redirect::to(uri!(ServerState::base(), pdf_upload_form(Some("Failed to create tmp directory")))))?;
    for (i,doc) in data.doc_name.iter_mut().enumerate() {
        let tmp_path = tmp_dir.path().join(format!("qltmp_pdf{}",i));
        doc.persist_to(&tmp_path).await
            .map_err(|_| Redirect::to(uri!(ServerState::base(), pdf_upload_form(Some("Persisting uploaded file failed.")))))?;
        if let Some(raw_name) = doc.raw_name() {
            let file_name = raw_name.dangerous_unsafe_unsanitized_raw().html_escape().to_string();            

            if let Some(path) = doc.path() {
                let transactions = parse_and_store(&path, &file_name, state.postgres_db.clone(), &pdf_config).await;
                match transactions {
                    Err(err) => {
                        errors.push(UploadError{
                            file_name,
                            message: format!("Failed to parse file: {}", err),
                        });
                    },
                    Ok(count) => {
                        errors.push(UploadError{
                            file_name,
                            message: format!("{} transaction(s) stored in database.", count),
                        });
                    },
                }
            } else {
                errors.push(UploadError{
                    file_name,
                    message: "Failed to parse file".to_string(),
                });
            }   
        } else {
            errors.push(UploadError{
                file_name: "unknown".to_string(),
                message: "No proper filename have been provided".to_string(),
            })
        }
    }

    let mut context = state.default_context();   
    context.insert("upload_results", &errors);
    Ok(layout("pdf_upload_report", &context.into_json()))
}

#[get("/transactions/delete?<transaction_id>")]
pub async fn delete_transaction(transaction_id: usize, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {
    let db = state.postgres_db.clone();
    // remove transaction and everything related, if the user has the proper rights

    let result = db.remove_transaction(transaction_id, user.userid).await;
    let message = if result.is_err() {
        Some("Failed to delete transaction")
    } else {
        None
    };
    Ok(Redirect::to(uri!(ServerState::base(), transactions(message))))
}

#[post("/transactions/update", data="<transaction>")]
pub async fn update_transaction(user: UserCookie, transaction: Form<TransactionForm>, state: &State<ServerState>) -> Option<String>  {
    let db = state.postgres_db.clone();
    // check if trans_ref belongs to trade where the user has access to
    if let Some(ref_id) = transaction.trans_ref {
        if db.get_transaction_account_if_valid(ref_id, user.userid).await.is_err() {
            return Some("The refenrece id is invalid".to_string());
        }  
    }

    // check whether currency exists
    if let Ok(currencies) = db.get_all_currencies().await {
        if !currencies.iter().any(|&c| c.to_string()==transaction.currency) {
            return Some("Currency is unknown".to_string());
        }   
    } else {
        return Some("Found no currencies".to_string());
    }
    
    if let Ok(trans) = transaction.to_transaction() {
        if let Some(id) = transaction.id {
            // check if id is valid
            if let Ok(old_account) = db.get_transaction_account_if_valid(id, user.userid).await {
                if db.update_transaction(&trans).await.is_err() {
                    return Some("Updating transaction failed".to_string());
                }
                let old_id = old_account.id.unwrap();
                if old_id != transaction.account_id {
                    if db.change_transaction_account(id, old_id, transaction.account_id).await.is_err() {
                        return Some("Updating transaction's account failed".to_string());
                    }
                }
            }
        } else {
            // new transaction, all checks passed, write to db
            if let Ok(id) = db.insert_transaction(&trans).await {
                if db.add_transaction_to_account(transaction.account_id, id).await.is_err() {
                    return Some("Inserting transaction failed".to_string());
                }
            } else {
                return Some("Inserting new transaction failed".to_string());
            }
        }
    } else {
        return Some("Invalid transaction format".to_string());
    }

    None
}

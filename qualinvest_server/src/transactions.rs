/// Viewing and editing of transactions
use tempfile::TempDir;
use rocket::State;
use rocket::response::Redirect;
use rocket_dyn_templates::Template;
use rocket::form::{Form, FromForm};
use rocket::fs::TempFile;

use chrono::Local;
use qualinvest_core::accounts::{Account,AccountHandler};
use qualinvest_core::user::UserHandler;
use finql_data::{Transaction,TransactionType,TransactionHandler,AssetHandler,CashAmount,CashFlow,Currency};
use qualinvest_core::PdfParseParams;
use qualinvest_core::read_pdf::{parse_and_store};
use crate::user::UserCookie;
use crate::layout::layout;
use crate::form_types::NaiveDateForm;
use std::str::FromStr;
use super::rocket_uri_macro_error_msg;
use super::rocket_uri_macro_login;
use super::ServerState;

#[get("/transactions")]
pub async fn transactions(user_opt: Option<UserCookie>, state: &State<ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(format!("{}{}",state.rel_path, uri!(login(redirect=Some("transactions"))))));
    }
    let user = user_opt.unwrap();

    let db = state.postgres_db.clone();
    let user_settings = db.get_user_settings(user.userid).await;
    let transactions = db.get_transaction_view_for_accounts(&user_settings.account_ids).await
        .map_err(|e| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg=format!("Couldn't get transactions for your account, error was {}", e))))))?;

    let mut context = state.default_context();
    context.insert("transactions", &transactions);
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
    fn new(account: &Account) -> Result<TransactionForm,&'static str> {
        if account.id.is_none() {
            Err("no_account_given")
        } else {
            Ok(TransactionForm{
                id: None,
                asset_id: None,
                position: None,
                trans_type: "7a".to_string(),
                cash_amount: 0.0,
                currency: "EUR".to_string(),
                date: NaiveDateForm::new(Local::now().naive_local().date()),
                note: None,
                trans_ref: None,
                account_id: account.id.unwrap(),
            })
        }
    }

    fn from(t: &Transaction, account: &Account) -> Result<TransactionForm,&'static str> {
        let mut tf = TransactionForm::new(account)?;
        tf.id = t.id;
        tf.note = t.note.as_ref().cloned();
        tf.cash_amount = t.cash_flow.amount.amount;
        tf.date = NaiveDateForm::new(t.cash_flow.date);
        tf.currency = t.cash_flow.amount.currency.to_string();
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

#[get("/transactions/edit?<transaction_id>")]
pub async fn edit_transaction(transaction_id: Option<usize>, user: UserCookie, state: &State<ServerState>) -> Result<Template,Redirect> {
    let db = state.postgres_db.clone();
    let user_accounts = user.get_accounts(db.clone()).await;
    if user_accounts.is_none()
    {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="no_user_accounts")))));
    }
    let user_accounts = user_accounts.unwrap();

    let transaction = if let Some(trans_id) = transaction_id {
        let account = db.get_transaction_account_if_valid(trans_id, user.userid).await
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="no_valid_account")))))?;

        let transaction = db.get_transaction_by_id(trans_id).await
            .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="invalid_transaction_id")))))?;
        TransactionForm::from(&transaction, &account)
            .map_err(|e| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg=e)))))?
    } else {
        TransactionForm::new(&user_accounts[0])
            .map_err(|e| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg=e)))))?
    };    

    let assets = db.get_all_assets().await
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="no_assets")))))?;
    let currencies = db.get_all_currencies().await
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="no_assets")))))?;
    let mut context = state.default_context();
    context.insert("transaction", &transaction);
    context.insert("assets", &assets);
    context.insert("currencies", &currencies);
    context.insert("valid_accounts", &user_accounts);
    context.insert("user", &user);
    Ok(layout("transaction_form", &context.into_json()))
}

#[get("/transactions/upload")]
pub async fn pdf_upload_form(user: UserCookie, state: &State<ServerState>) -> Result<Template,Redirect> {
    let db = state.postgres_db.clone();
    let user_accounts = user.get_accounts(db.clone()).await;
    if user_accounts.is_none()
    {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="no_user_accounts")))));
    }
    let user_accounts = user_accounts.unwrap();
    let default_account_id: Option<usize> = None;

    let mut context = state.default_context();   
    context.insert("user", &user);
    context.insert("default_account_id", &default_account_id);
    context.insert("accounts", &user_accounts);
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
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="Only admins may upload pdf files!")))));
    }
    // parse each each pdf found
    let mut errors = Vec::new();
    let tmp_dir = TempDir::new()
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="Failed to create tmp directory")))))?;
    for (i,doc) in data.doc_name.iter_mut().enumerate() {
        let tmp_path = tmp_dir.path().join(format!("qltmp_pdf{}",i));
        doc.persist_to(&tmp_path).await
            .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="Persisting uploaded file failed.")))))?;
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

    db.remove_transaction(transaction_id, user.userid).await
        .map_err(|_| Redirect::to(uri!(error_msg(msg="data_access_failure_access_denied"))))?;
    Ok(Redirect::to(format!("{}{}", state.rel_path, uri!(transactions()))))
}

#[post("/transactions", data = "<form>")]
pub async fn process_transaction(form: Form<TransactionForm>, user: UserCookie, state: &State<ServerState>) -> Result<Redirect,Redirect> {
    let transaction = form.into_inner();
    let db = state.postgres_db.clone();

    // check that user has access rights to account
    let user_accounts = user.get_accounts(db.clone()).await;
    if user_accounts.is_none()
    {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="no_user_accounts")))));
    }
    let user_accounts = user_accounts.unwrap();
    if !user_accounts.iter().any(|acc| acc.id==Some(transaction.account_id)) {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="no_access_to_account")))));
    }

    // check if trans_ref belongs to trade where the user has access to
    if transaction.trans_ref.is_some() {
        let ref_id = transaction.trans_ref.unwrap();
        if db.get_transaction_account_if_valid(ref_id, user.userid).await.is_err() {
            return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="access_violation_trans_ref")))));
        }
    }

    // check whether currency exists
    let currencies = db.get_all_currencies().await
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="currency_check_failed")))))?;
    if !currencies.iter().any(|&c| c.to_string()==transaction.currency) {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="unknown_currency")))));
    }

    if let Some(id) = transaction.id {
        // check for valid id
        if let Ok(old_account) = db.get_transaction_account_if_valid(id, user.userid).await {
            let trans = &transaction.to_transaction()
                .map_err(|e| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg=e)))))?;
            db.update_transaction(trans).await
                .map_err(|_| { Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="update_of_transaction_failed"))))})?;
            let old_id = old_account.id.unwrap();
            if old_id != transaction.account_id {
                db.change_transaction_account(id, old_id, transaction.account_id).await
                    .map_err(|_| { Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="update_of_transaction_failed"))))})?;
            }
        } else {
            return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="access_violation_trans_ref")))));
        }
    } else {
        // new transaction, all checks passed, write to db
        let trans = &transaction.to_transaction()
            .map_err(|e| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg=e)))))?;
        let id = db.insert_transaction(trans).await
            .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="insert_new_transaction_failed")))))?;
        db.add_transaction_to_account(transaction.account_id, id).await
            .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg(msg="insert_new_transaction_failed")))))?;
    }

    Ok(Redirect::to(format!("{}{}", state.rel_path, uri!(transactions()))))
}

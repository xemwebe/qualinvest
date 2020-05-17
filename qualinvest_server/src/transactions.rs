use std::ops::DerefMut;

use rocket::State;
use rocket::response::Redirect;
use rocket_contrib::templates::Template;
use rocket::request::{Form,FromFormValue};
use rocket::http::RawStr;

use chrono::{Local,NaiveDate};
use finql::postgres_handler::PostgresDB;
use qualinvest_core::accounts::{Account,AccountHandler};
use qualinvest_core::user::UserHandler;
use finql::transaction::{Transaction,TransactionType};
use finql::data_handler::{TransactionHandler,AssetHandler};
use finql::{CashAmount,CashFlow,Currency};
use crate::user::UserCookie;
use super::{QlInvestDbConn,ServerState};
use crate::layout::layout;
use crate::filter;
use std::str::FromStr;
use super::rocket_uri_macro_error_msg;
use super::rocket_uri_macro_login;

#[get("/transactions?<accounts>&<start>&<end>")]
pub fn transactions(accounts: Option<String>, start: Option<String>, end: Option<String>, user_opt: Option<UserCookie>, mut qldb: QlInvestDbConn, state: State<ServerState>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(login: redirect="transactions"))));
    }
    let user = user_opt.unwrap();

    let mut db = PostgresDB{ conn: qldb.0.deref_mut() };
    let user_accounts = user.get_accounts(&mut db);
    if user_accounts.is_none() {
//        return Err(Flash::error(Redirect::to(format!("{}{}", state.rel_path, uri!(transactions: accounts, start, end), "Please ask the administrator to set up an account for you first.")));
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="no_user_account"))));
    }
    let user_accounts = user_accounts.unwrap();

    let filter = filter::FilterForm::from_query(accounts, start, end, &user, &user_accounts, &mut db)?;

    let transactions = db.get_transaction_view_for_accounts(&filter.account_ids)
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="get_transaction_view_for_accounts"))))?;

    let mut context = state.default_context();
    context.insert("transactions", &transactions);
    context.insert("valid_accounts", &user_accounts);
    context.insert("user", &user);
    context.insert("filter", &filter);
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
                trans_type: "a".to_string(),
                cash_amount: 0.0,
                currency: "EUR".to_string(),
                date: NaiveDateForm(Local::now().naive_local().date()),
                note: None,
                trans_ref: None,
                account_id: account.id.unwrap(),
            })
        }
    }

    fn from(t: &Transaction, account: &Account) -> Result<TransactionForm,&'static str> {
        let mut tf = TransactionForm::new(account)?;
        tf.id = t.id;
        tf.note = if let Some(s) = &t.note {
            Some(s.clone())
        } else {
            None
        };
        tf.cash_amount = t.cash_flow.amount.amount;
        tf.date = NaiveDateForm(t.cash_flow.date);
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
                date: self.date.0,
            },
            note: match &self.note {
                None => None,
                Some(s) => Some(s.clone(),)
            },
        };
        Ok(t)
    } 
}

#[derive(Debug,Serialize,Deserialize)]
pub struct NaiveDateForm(NaiveDate);

impl<'v> FromFormValue<'v> for NaiveDateForm {
    type Error = &'v RawStr;

    fn from_form_value(form_value: &'v RawStr) -> Result<NaiveDateForm, &'v RawStr> {
        Ok(NaiveDateForm(NaiveDate::parse_from_str(form_value.as_str(), "%Y-%m-%d")
            .map_err(|_| form_value )?))
    }
}

#[get("/transactions/edit/<trans_id>")]
pub fn edit_transaction(trans_id: usize, user: UserCookie, mut qldb: QlInvestDbConn, state: State<ServerState>) -> Result<Template,Redirect> {
    let mut db = PostgresDB{ conn: qldb.0.deref_mut() };
    let user_accounts = user.get_accounts(&mut db);
    if user_accounts.is_none()
    {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="no_user_accounts"))));
    }
    let user_accounts = user_accounts.unwrap();

    let account = db.get_transaction_account_if_valid(trans_id, user.userid)
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="no_valid_account"))) )?;

        let transaction = db.get_transaction_by_id(trans_id)
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="invalid_transaction_id"))))?;
    let transaction_form = TransactionForm::from(&transaction, &account)
        .map_err(|e| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg=e))))?;
    let assets = db.get_all_assets()
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="no_assets"))))?;
    let currencies = db.get_all_currencies()
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="no_assets"))))?;
    let mut context = state.default_context();
    context.insert("transaction", &transaction_form);
    context.insert("assets", &assets);
    context.insert("currencies", &currencies);
    context.insert("valid_accounts", &user_accounts);
    context.insert("user", &user);
    Ok(layout("edit_transaction", &context.into_json()))
}

#[get("/transactions/new")]
pub fn new_transaction(user: UserCookie, mut qldb: QlInvestDbConn, state: State<ServerState>) -> Result<Template,Redirect> {
    let mut db = PostgresDB{ conn: qldb.0.deref_mut() };
    let user_accounts = user.get_accounts(&mut db);
    if user_accounts.is_none()
    {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="no_user_accounts"))));
    }
    let user_accounts = user_accounts.unwrap();
    let transaction = TransactionForm::new(&user_accounts[0])
        .map_err(|e| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg=e))))?;
    let assets = db.get_all_assets()
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="no_assets"))))?;   
    let currencies = db.get_all_currencies()
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="no_assets"))))?;
    let mut context = state.default_context();
    context.insert("transaction", &transaction);
    context.insert("assets", &assets);
    context.insert("currencies", &currencies);
    context.insert("valid_accounts", &user_accounts);
    context.insert("user", &user);
    Ok(layout("new_transaction", &context.into_json()))
}


#[get("/transactions/delete/<trans_id>")]
pub fn delete_transaction(trans_id: usize, user: UserCookie, mut qldb: QlInvestDbConn, state: State<ServerState>) -> Result<Redirect,Redirect> {
    let mut db = PostgresDB{ conn: qldb.0.deref_mut() };
    // remove transaction and everything related, if the user has the proper rights
    db.remove_transaction(trans_id, user.userid)
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="data_access_failure_access_denied"))))?;
    Ok(Redirect::to(format!("{}{}", state.rel_path, uri!(transactions: _,_,_))))
}

#[post("/transactions", data = "<form>")]
pub fn process_transaction(form: Form<TransactionForm>, user: UserCookie, mut qldb: QlInvestDbConn, state: State<ServerState>) -> Result<Redirect,Redirect> {
    let transaction = form.into_inner();
    let mut db = PostgresDB{ conn: qldb.0.deref_mut() };

    // check that user has access rights to account
    let user_accounts = user.get_accounts(&mut db);
    if user_accounts.is_none()
    {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="no_user_accounts"))));
    }
    let user_accounts = user_accounts.unwrap();
    if !user_accounts.iter().any(|acc| acc.id==Some(transaction.account_id)) {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="no_access_to_account"))));
    }

    // check if trans_ref belongs to trade where the user has access to
    if transaction.trans_ref.is_some() {
        let ref_id = transaction.trans_ref.unwrap();
        if db.get_transaction_account_if_valid(ref_id, user.userid).is_err() {
            return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="access_violation_trans_ref"))));
        }
    }

    // check whether currency exists
    let currencies = db.get_all_currencies()
        .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="currency_check_failed"))))?;
    if !currencies.iter().any(|&c| c.to_string()==transaction.currency) {
        return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="unknown_currency"))));
    }

    if let Some(id) = transaction.id {
        // check for valid id
        if let Ok(old_account) = db.get_transaction_account_if_valid(id, user.userid) {
            let trans = &transaction.to_transaction()
                .map_err(|e| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg=e))))?;
            db.update_transaction(trans)
                .map_err(|_| { Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="update_of_transaction_failed")))})?;
            let old_id = old_account.id.unwrap();
            if old_id != transaction.account_id {
                db.change_transaction_account(id, old_id, transaction.account_id)
                    .map_err(|_| { Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="update_of_transaction_failed")))})?;
            }
        } else {
            return Err(Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="access_violation_trans_ref"))));
        }
    } else {
        // new transaction, all checks passed, write to db
        let trans = &transaction.to_transaction()
            .map_err(|e| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg=e))))?;
        let id = db.insert_transaction(trans)
            .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="insert_new_transaction_failed"))))?;
        db.add_transaction_to_account(transaction.account_id, id)
            .map_err(|_| Redirect::to(format!("{}{}", state.rel_path, uri!(error_msg: msg="insert_new_transaction_failed"))))?;
    }

    Ok(Redirect::to(format!("{}{}", state.rel_path, uri!(transactions: _,_,_))))
}

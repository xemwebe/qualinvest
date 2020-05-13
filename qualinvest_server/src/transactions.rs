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
use qualinvest_core::Config;
use finql::transaction::{Transaction,TransactionType};
use finql::data_handler::{TransactionHandler,AssetHandler};
use crate::helper;
use crate::user::UserCookie;
use super::{default_context, QlInvestDbConn};
use crate::layout::layout;

#[get("/transactions?<accounts>")]
pub fn transactions(accounts: Option<String>, user_opt: Option<UserCookie>, mut qldb: QlInvestDbConn, state: State<Config>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to("/login?redirect=transactions"));
    }

    let user = user_opt.unwrap();
    let mut db = PostgresDB{ conn: qldb.0.deref_mut() };
    let user_accounts = user.get_accounts(&mut db);
    if user_accounts.is_none() {
        return Err(Redirect::to("/err/no_user_account"));
    }
    let user_accounts = user_accounts.unwrap();

    let selected_accounts =
        if let Some(accounts) = accounts {
            let accounts = helper::parse_ids(&accounts);
            if user.is_admin {
                accounts
            } else {
                db.valid_accounts(user.userid, &accounts)
                    .map_err(|_| Redirect::to("/err/valid_accounts"))?
            }
        } else {
            let mut account_ids = Vec::new();
            for account in &user_accounts {
                account_ids.push(account.id.unwrap());
            }
            account_ids
        };

    let transactions = db.get_transaction_view_for_accounts(&selected_accounts)
        .map_err(|_| Redirect::to("/err/get_transaction_view_for_accounts"))?;

    let mut context = default_context(&state);
    context.insert("transactions", &transactions);
    context.insert("selected_accounts", &selected_accounts);
    context.insert("valid_accounts", &user_accounts);
    Ok(layout("transactions", &context.into_json()))
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
    pub account_info: String,
}


impl TransactionForm {
    fn new(account: &Account) -> Result<TransactionForm,Redirect> {
        if account.id.is_none() {
            Err(Redirect::to("/err/no_account_given"))
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
                account_info: format!("{}: {}", account.broker, account.account_name),
            })
        }
    }

    fn from(t: &Transaction, account: &Account) -> Result<TransactionForm,Redirect> {
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
}

#[get("/transactions/edit/<trans_id>")]
pub fn edit_transaction(trans_id: usize, user_opt: Option<UserCookie>, mut qldb: QlInvestDbConn, state: State<Config>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to("/login?redirect=transactions"));
    }

    let user = user_opt.unwrap();
    let mut db = PostgresDB{ conn: qldb.0.deref_mut() };
    let user_accounts = user.get_accounts(&mut db);
    if user_accounts.is_none()
    {
        return Err(Redirect::to("err/no_user_accounts"));
    }
    let user_accounts = user_accounts.unwrap();

    let account = db.get_transaction_account_if_valid(trans_id, user.userid)
        .map_err(|_| Redirect::to("err/no_valid_account") )?;

        let transaction = db.get_transaction_by_id(trans_id)
        .map_err(|_| Redirect::to("err/invalid_transaction_id"))?;
    let transaction_form = TransactionForm::from(&transaction, &account)?;
    let assets = db.get_all_assets()
        .map_err(|_| Redirect::to("err/no_assets"))?;
    let currencies = db.get_all_currencies()
        .map_err(|_| Redirect::to("err/no_assets"))?;
    let mut context = default_context(&state);
    context.insert("transaction", &transaction_form);
    context.insert("assets", &assets);
    context.insert("currencies", &currencies);
    context.insert("valid_accounts", &user_accounts);
    context.insert("user", &user);
    Ok(layout("edit_transaction", &context.into_json()))
}

#[get("/transactions/new")]
pub fn new_transaction(user_opt: Option<UserCookie>, mut qldb: QlInvestDbConn, state: State<Config>) -> Result<Template,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to("/login?redirect=transactions"));
    }

    let user = user_opt.unwrap();
    let mut db = PostgresDB{ conn: qldb.0.deref_mut() };
    let user_accounts = user.get_accounts(&mut db);
    if user_accounts.is_none()
    {
        return Err(Redirect::to("err/no_user_accounts"));
    }
    let user_accounts = user_accounts.unwrap();
    let transaction = TransactionForm::new(&user_accounts[0])?;
    let assets = db.get_all_assets()
        .map_err(|_| Redirect::to("err/no_assets"))?;   
    let currencies = db.get_all_currencies()
        .map_err(|_| Redirect::to("err/no_assets"))?;
    let mut context = default_context(&state);
    context.insert("transaction", &transaction);
    context.insert("assets", &assets);
    context.insert("currencies", &currencies);
    context.insert("valid_accounts", &user_accounts);
    context.insert("user", &user);
    Ok(layout("new_transaction", &context.into_json()))
}


#[get("/transactions/delete/<trans_id>")]
pub fn delete_transaction(trans_id: usize, user_opt: Option<UserCookie>, mut qldb: QlInvestDbConn) -> Result<Redirect,Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to("/login?redirect=transactions"));
    }

    let user = user_opt.unwrap();
    let mut db = PostgresDB{ conn: qldb.0.deref_mut() };
        
    db.remove_transaction(trans_id, user.userid)
        .map_err(|_| Redirect::to("err/data_access_failure_access_denied"))?;
    Ok(Redirect::to("/transactions"))
}

#[post("/transactions", data = "<form>")]
pub fn process_transaction(form: Form<TransactionForm>, user_opt: Option<UserCookie>, mut qldb: QlInvestDbConn, state: State<Config>) -> Result<Redirect,Redirect> {
    let transaction = form.into_inner();

    let user = user_opt.unwrap();
    let mut db = PostgresDB{ conn: qldb.0.deref_mut() };

    let user_accounts = user.get_accounts(&mut db);
    if user_accounts.is_none()
    {
        return Err(Redirect::to("err/no_user_accounts"));
    }
    let user_accounts = user_accounts.unwrap();

    Ok(Redirect::to("/transactions"))
}

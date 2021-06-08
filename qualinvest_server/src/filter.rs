use std::collections::HashMap;
use std::sync::Arc;

use rocket_dyn_templates::{Template, Engines};
use rocket_dyn_templates::tera::{self, Value};
use rocket::fairing::Fairing;
use rocket::response::Redirect;
use rocket::State;
use rocket::form::{Form,FromForm};

use num_format::{Locale, WriteFormatted};
use unicode_segmentation::UnicodeSegmentation;
use crate::helper;
use chrono::{Local,NaiveDate};
use crate::user;
use crate::form_types::NaiveDateForm;
use qualinvest_core::user::UserHandler;
use qualinvest_core::accounts::Account;
use super::ServerState;

fn format_num_precision(num: f64, precision: i32) -> String {
    let fac10 = 10_f64.powi(precision);
    let rounded_num = (num*fac10).round() as i64;
    let i_fac10 = fac10 as i64;
    let int_part = rounded_num/i_fac10;
    let decimal_part = (rounded_num-int_part*i_fac10).abs();
    let mut writer = String::new();
    writer.write_formatted(&(int_part as i64), &Locale::en).unwrap();
    format!("{int_part}.{decimal_part:0>width$}", int_part=writer, decimal_part=decimal_part, width=precision as usize)
}

fn format_num(value: &Value, _: &HashMap<String, Value>) -> tera::Result<Value> {
    if value.is_f64() {
        let num = value.as_f64().unwrap();
        Ok(tera::Value::String(format_num_precision(num, 2)))
    } else {
        Ok(value.clone())
    }
}

fn format_num4(value: &Value, _: &HashMap<String, Value>) -> tera::Result<Value> {
    if value.is_f64() {
        let num = value.as_f64().unwrap();
        Ok(tera::Value::String(format_num_precision(num, 4)))
    } else {
        Ok(value.clone())
    }
}


fn format_per_cent(value: &Value, _: &HashMap<String, Value>) -> tera::Result<Value> {
    if value.is_f64() {
        let num = value.as_f64().unwrap();
        let num_str = format!("{:.2}%", 100.*num);
        Ok(tera::Value::String(num_str))
    } else {
        Ok(value.clone())
    }
}

fn type_to_string(value: &Value, _: &HashMap<String, Value>) -> tera::Result<Value> {
    match value {
        Value::String(type_str) => Ok(Value::String( match type_str.as_str() {
            "c" => "Cash",
            "a" => "Buy or Sell",
            "d" => "Dividend",
            "i" => "Interest",
            "t" => "Tax",
            "f" => "Fee",
            _ => "Unknown",
        }.to_string() )),       
        _ => Ok(value.clone())
    }
}


fn remove_line_break(value: &Value, _: &HashMap<String, Value>) -> tera::Result<Value> {
    match value {
        Value::String(type_str) => Ok(Value::String( type_str.replace("\n","") )),       
        _ => Ok(value.clone())
    }
}

fn base_name(value: &Value, _: &HashMap<String, Value>) -> tera::Result<Value> {
    match value {
        Value::String(file) => Ok(Value::String( helper::basename(file).to_string() )),       
        _ => Ok(value.clone())
    }
}

fn print_str_slice(strs: &[&str]) -> String {
    let mut s = String::new();
    for e in strs {
        s = format!("{}{}", s, e);
    }
    s
}

fn short_text(value: &Value, _: &HashMap<String, Value>) -> tera::Result<Value> {
    match value {
        Value::String(s) =>  {
            if s.len() > 53 {
                let g = UnicodeSegmentation::graphemes(s.as_str(), true).collect::<Vec<&str>>();
                Ok(Value::String( format!("{}...", print_str_slice(&g[..51]) )))
            } else {
                Ok(value.clone())   
            }
        },
        _ => Ok(value.clone())
    }
}


pub fn set_filter() -> impl Fairing {
    Template::custom(|engines: &mut Engines| {
        engines.tera.register_filter("format_num", format_num);
        engines.tera.register_filter("format_num4", format_num4);
        engines.tera.register_filter("format_per_cent", format_per_cent);
        engines.tera.register_filter("type_to_string", type_to_string);
        engines.tera.register_filter("remove_line_break", remove_line_break);
        engines.tera.register_filter("base_name", base_name);
        engines.tera.register_filter("short_text", short_text);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_num_formatter() {
        assert_eq!(format_num_precision(12345.6789, 3), "12,345.679".to_string());
    }
}


#[derive(Debug,Serialize,Deserialize)]
pub struct PlainFilter {
    pub account_ids: Vec<usize>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

#[derive(Debug,Serialize,Deserialize,FromForm)]
pub struct FilterForm {
    pub account_ids: Vec<String>,
    pub start_date: NaiveDateForm,
    pub end_date: NaiveDateForm,
}

impl PlainFilter {
    pub async fn from_query<'a>(accounts: Option<String>, start: Option<String>, end: Option<String>, user: &'a user::UserCookie, 
        user_accounts: &Vec<Account>, rel_path: &str, db: Arc<dyn UserHandler+Send+Sync+'a>) -> Result<PlainFilter, Redirect> {
        let end_date = match end {
            Some(s) => NaiveDate::parse_from_str(s.as_str(), "%Y-%m-%d")
                .map_err(|_| Redirect::to(format!("{}{}", rel_path, "/err/invalid_date")))?,
            None => Local::now().naive_local().date()
        };
        let start_date = match start {
            Some(s) => NaiveDate::parse_from_str(s.as_str(), "%Y-%m-%d")
                .map_err(|_| Redirect::to(format!("{}{}", rel_path, "/err/invalid_date")))?,
            None => end_date
        };
        let account_ids =
            if let Some(accounts) = accounts {
                let accounts = helper::parse_ids(&accounts);
                if user.is_admin {
                    accounts
                } else {
                    db.valid_accounts(user.userid, &accounts).await
                        .map_err(|_| Redirect::to(format!("{}{}",rel_path, "/err/valid_accounts")))?
                }
        } else {
            let mut account_ids = Vec::new();
            for account in user_accounts {
                account_ids.push(account.id.unwrap());
            }
            account_ids
        };
        Ok(PlainFilter{account_ids, start_date, end_date})
    }
}

impl FilterForm {
    fn to_query(&self) -> String {
        if self.account_ids.len() == 0 {
            String::new()
        } else {
            let mut query  = format!("?accounts={}", self.account_ids[0]);
            for id in &self.account_ids[1..] {
                query = format!("{},{}", query, *id);
            }
            query = format!("{}&start={}&end={}",query
                ,self.start_date.date.format("%Y-%m-%d").to_string()
                ,self.end_date.date.format("%Y-%m-%d").to_string()
            );
            query
        }
    }
}

#[post("/filter/<view>", data="<form>")]
pub fn process_filter(view: String, form: Form<FilterForm>, state: &State<ServerState>) -> Redirect {
    let filter_form = form.into_inner();
    let query_string = format!("/{}/{}{}", state.rel_path, view, filter_form.to_query());
    Redirect::to(query_string)
}


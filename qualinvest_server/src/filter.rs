use std::collections::HashMap;

use rocket_contrib::templates::{Template, Engines};
use rocket_contrib::templates::tera::{self, Value};
use rocket::request::{Form, FormItems, FormItem, FromForm};
use rocket::fairing::Fairing;
use rocket::response::Redirect;
use num_format::{Locale, WriteFormatted};
use lazy_static::lazy_static;
use regex::Regex;
use crate::helper;

fn format_num_precision(num: f64, precision: i32) -> String {
    let mut writer = String::new();
    let int_part = num.floor();
    let decimal_part = ((num-int_part)*10_f64.powi(precision)).round();
    writer.write_formatted(&(int_part as i64), &Locale::en).unwrap();
    format!("{}.{}", writer, decimal_part)
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
            "a" => "Buy/Sell",
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
        Value::String(type_str) => Ok(Value::String( helper::basename(type_str).to_string() )),       
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


#[derive(Debug)]
pub struct FilterForm {
    account_ids: Vec<usize>,
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
            query
        }
    }
}

impl<'f> FromForm<'f> for FilterForm {
    type Error = &'static str;
    
    fn from_form(form_items: &mut FormItems<'f>, _strict: bool) -> Result<Self, Self::Error> {
        lazy_static! {
            static ref ACCOUNT_ID: Regex = Regex::new(r"accid([0-9]*)").unwrap();
        }
        
        let mut account_ids = Vec::new();
        for FormItem { key, .. } in form_items {
            match ACCOUNT_ID.captures(key.as_str()) {
                Some(account) =>  { account_ids.push( account[1].parse::<usize>().unwrap()); },
                None => { return Err("Invalid form parameter found"); }
            }
        }

        Ok(
            FilterForm {
                account_ids
            }
        )
    }
}

#[post("/filter/<view>", data="<form>")]
pub fn process_filter(view: String, form: Form<FilterForm>) -> Redirect {
    let filter_form = form.into_inner();
    let query_string = format!("/{}{}", view, filter_form.to_query());
    Redirect::to(query_string) 
}


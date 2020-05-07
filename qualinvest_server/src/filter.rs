use std::collections::HashMap;

use rocket_contrib::templates::{Template, Engines};
use rocket_contrib::templates::tera::{self, Value};
use rocket::fairing::Fairing;
use num_format::{Locale, WriteFormatted};

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

pub fn set_filter() -> impl Fairing {
    Template::custom(|engines: &mut Engines| {
        engines.tera.register_filter("format_num", format_num);
        engines.tera.register_filter("format_num4", format_num4);
        engines.tera.register_filter("format_per_cent", format_per_cent);
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
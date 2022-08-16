use std::collections::HashMap;

use rocket::fairing::Fairing;
use rocket_dyn_templates::tera::{self, Value};
use rocket_dyn_templates::{Engines, Template};

use crate::helper;
use num_format::{Locale, WriteFormatted};
use unicode_segmentation::UnicodeSegmentation;

fn format_num_precision(num: f64, precision: i32) -> String {
    let fac10 = 10_f64.powi(precision);
    let rounded_num = (num * fac10).round() as i64;
    let i_fac10 = fac10 as i64;
    let int_part = rounded_num / i_fac10;
    let decimal_part = (rounded_num - int_part * i_fac10).abs();
    let mut writer = String::new();
    writer
        .write_formatted(&(int_part as i64), &Locale::en)
        .unwrap();
    format!(
        "{int_part}.{decimal_part:0>width$}",
        int_part = writer,
        decimal_part = decimal_part,
        width = precision as usize
    )
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
        let num_str = format!("{:.2}%", 100. * num);
        Ok(tera::Value::String(num_str))
    } else {
        Ok(value.clone())
    }
}

fn type_to_string(value: &Value, _: &HashMap<String, Value>) -> tera::Result<Value> {
    match value {
        Value::String(type_str) => Ok(Value::String(
            match type_str.as_str() {
                "c" => "Cash",
                "a" => "Buy or Sell",
                "d" => "Dividend",
                "i" => "Interest",
                "t" => "Tax",
                "f" => "Fee",
                _ => "Unknown",
            }
            .to_string(),
        )),
        _ => Ok(value.clone()),
    }
}

fn remove_line_break(value: &Value, _: &HashMap<String, Value>) -> tera::Result<Value> {
    match value {
        Value::String(type_str) => Ok(Value::String(type_str.replace('\n', ""))),
        _ => Ok(value.clone()),
    }
}

fn base_name(value: &Value, _: &HashMap<String, Value>) -> tera::Result<Value> {
    match value {
        Value::String(file) => Ok(Value::String(helper::basename(file).to_string())),
        _ => Ok(value.clone()),
    }
}

fn print_str_slice(strs: &[&str]) -> String {
    let mut s = String::new();
    for e in strs {
        s = format!("{s}{e}");
    }
    s
}

fn short_text(value: &Value, _: &HashMap<String, Value>) -> tera::Result<Value> {
    match value {
        Value::String(s) => {
            if s.len() > 53 {
                let g = UnicodeSegmentation::graphemes(s.as_str(), true).collect::<Vec<&str>>();
                Ok(Value::String(format!("{}...", print_str_slice(&g[..51]))))
            } else {
                Ok(value.clone())
            }
        }
        _ => Ok(value.clone()),
    }
}

pub fn set_filter() -> impl Fairing {
    Template::custom(|engines: &mut Engines| {
        engines.tera.register_filter("format_num", format_num);
        engines.tera.register_filter("format_num4", format_num4);
        engines
            .tera
            .register_filter("format_per_cent", format_per_cent);
        engines
            .tera
            .register_filter("type_to_string", type_to_string);
        engines
            .tera
            .register_filter("remove_line_break", remove_line_break);
        engines.tera.register_filter("base_name", base_name);
        engines.tera.register_filter("short_text", short_text);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_num_formatter() {
        assert_eq!(
            format_num_precision(12345.6789, 3),
            "12,345.679".to_string()
        );
    }
}

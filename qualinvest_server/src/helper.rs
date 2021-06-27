use std::borrow::Cow;
use lazy_static::lazy_static;
use regex::Regex;
use chrono::{NaiveDate};
use rocket::response::Redirect;


pub fn parse_ids(s: &str) -> Vec<usize> {
    lazy_static! {
        static ref RANGE: Regex = Regex::new(r"\s*([0-9]*)\s*-\s*([0-9]*)\s*").unwrap();
        static ref NUM: Regex = Regex::new(r"\s*([0-9]*)\s*").unwrap();
    }

    let mut ids = Vec::new();
    for sub in s.split(",") {
        match RANGE.captures(sub) {
            Some(range) => {
                let start = range[1].parse::<usize>();
                let end = range[2].parse::<usize>();
                if start.is_ok() && end.is_ok() {
                    for i in start.unwrap()..end.unwrap()+1 {
                        ids.push(i);
                    }
                }
            },
            None => {
                match NUM.captures(sub) {
                    Some(num) => {
                        let num = num[1].parse::<usize>();
                        if let Ok(num) = num {
                            ids.push(num);
                        }
                    },
                    None => {},
                };
            },
        }
    }
    ids
}


pub fn basename<'a>(path: &'a str) -> Cow<'a, str> {
    let mut pieces = path.rsplitn(2, |c| c == '/' || c == '\\');
    match pieces.next() {
        Some(p) => p.into(),
        None => path.into(),
    }
}

pub fn date_from_string(date_str: &str, rel_path: &str) -> Result<NaiveDate, Redirect> {
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .map_err(|_| Redirect::to(format!("{}{}", rel_path, "/err/invalid_date")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ids() {
        assert_eq!(parse_ids(" 2, 7-9, 6, 3-4 "), vec![2,7,8,9,6,3,4]);
    }
    
    #[test]
    fn test_basename() {
     assert_eq!(basename("c:\\users\\fakeUser\\myfile.txt"), "myfile.txt");   
     assert_eq!(basename("/home/fakeUser/myfile.txt"), "myfile.txt");   
    }
}
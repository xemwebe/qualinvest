use super::ReadPDFError;
use finql::datatypes::CashAmount;
use time::{macros::format_description, Date};

type Result<T> = std::result::Result<T, ReadPDFError>;

/// Convert a string with German number convention
/// (e.g. '.' as thousands separator and ',' as decimal separator)
pub fn german_string_to_float(num_string: &str) -> Result<f64> {
    let sign_less_string = num_string.replace('-', "");
    let positive = sign_less_string == num_string;
    let result = sign_less_string
        .trim()
        .replace('.', "")
        .replace(',', ".")
        .parse()
        .map_err(ReadPDFError::ParseFloat);
    match result {
        Ok(num) => {
            if positive {
                Ok(num)
            } else {
                Ok(-num)
            }
        }
        Err(err) => Err(err),
    }
}

/// Converts strings in German data convention to Date
pub fn german_string_to_date(date_string: &str) -> Result<Date> {
    let format = format_description!("[day].[month].[year]");
    Date::parse(date_string, &format).map_err(|_| ReadPDFError::ParseDate)
}

pub fn add_by_currency(
    new_amount: &CashAmount,
    base_amount: &mut CashAmount,
    foreign_amount: &mut CashAmount,
) {
    if new_amount.currency == base_amount.currency {
        base_amount.amount += new_amount.amount;
    } else {
        foreign_amount.amount += new_amount.amount;
    }
}

pub fn rounded_equal(x: f64, y: f64, precision: i32) -> bool {
    let factor = 10.0_f64.powi(precision);
    ((x * factor).round() - (y * factor).round()).abs() < 1.0
}

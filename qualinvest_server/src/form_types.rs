use rocket::form;
use chrono::NaiveDate;

#[derive(Debug,Serialize,Deserialize)]
pub struct OptionalNaiveDateForm {
    pub date: Option<NaiveDate>
}

#[rocket::async_trait]
impl<'r> form::FromFormField<'r> for OptionalNaiveDateForm {
    fn from_value(field: form::ValueField<'r>) -> form::Result<'r, Self> {
        match NaiveDate::parse_from_str(field.value, "%Y-%m-%d") {
            Ok(date) => Ok(OptionalNaiveDateForm{ date: Some(date) }),
            Err(_) => Ok(OptionalNaiveDateForm{ date: None })
        }
    }
}

#[derive(Debug,Serialize,Deserialize)]
pub struct NaiveDateForm {
    pub date: NaiveDate
}

impl NaiveDateForm {
    pub fn new(date: NaiveDate) -> NaiveDateForm {
        NaiveDateForm{ date }
    }
}

#[rocket::async_trait]
impl<'r> form::FromFormField<'r> for NaiveDateForm {
    fn from_value(field: form::ValueField<'r>) -> form::Result<'r, Self> {
        match NaiveDate::parse_from_str(field.value, "%Y-%m-%d") {
            Ok(date) => Ok(NaiveDateForm{ date }),
            Err(err) => Err(rocket::form::Errors::from(form::Error::validation(err.to_string())))
        }
    }
}

use rocket::form;
use chrono::NaiveDate;

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
        match NaiveDate::parse_from_str(field.as_str(), "%Y-%m-%d") {
            Ok(date) => Ok(NaiveDateForm{ date }),
            Err(err) => Err(form::Error::validation("is no valid date"))
        }
    }
}

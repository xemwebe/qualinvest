use rocket::form;
use time::{macros::format_description, Date};

#[derive(Debug, Serialize, Deserialize)]
pub struct OptionalDateForm {
    pub date: Option<Date>,
}

#[rocket::async_trait]
impl<'r> form::FromFormField<'r> for OptionalDateForm {
    fn from_value(field: form::ValueField<'r>) -> form::Result<'r, Self> {
        let format = format_description!("[year]-[month]-[day]");
        match Date::parse(field.value, &format) {
            Ok(date) => Ok(OptionalDateForm { date: Some(date) }),
            Err(_) => Ok(OptionalDateForm { date: None }),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DateForm {
    pub date: Date,
}

impl DateForm {
    pub fn new(date: Date) -> DateForm {
        DateForm { date }
    }
}

#[rocket::async_trait]
impl<'r> form::FromFormField<'r> for DateForm {
    fn from_value(field: form::ValueField<'r>) -> form::Result<'r, Self> {
        let format = format_description!("[year]-[month]-[day]");
        match Date::parse(field.value, &format) {
            Ok(date) => Ok(DateForm { date }),
            Err(err) => Err(rocket::form::Errors::from(form::Error::validation(
                err.to_string(),
            ))),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AssetListItem {
    pub id: i32,
    pub name: String,
}

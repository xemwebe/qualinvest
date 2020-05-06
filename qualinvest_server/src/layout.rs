use rocket_contrib::templates::Template;
use tera;
use super::auth::sanitization::*;

pub fn layout(template: &'static str, context: &serde_json::Value) -> Template {
    Template::render(template, context)
}


use rocket_contrib::templates::Template;

pub fn layout(template: &'static str, context: &serde_json::Value) -> Template {
    Template::render(template, context)
}


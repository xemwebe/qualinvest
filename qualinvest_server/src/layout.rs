use rocket_dyn_templates::Template;

pub fn layout(template: &'static str, context: &serde_json::Value) -> Template {
    Template::render(template, context)
}

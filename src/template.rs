use minijinja::{Environment, UndefinedBehavior};
use serde::Serialize;

pub(crate) fn render_template<T>(template: &str, context: T) -> String
where
    T: Serialize,
{
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.set_trim_blocks(true);
    env.set_lstrip_blocks(true);
    env.add_template("template", template)
        .expect("static template should parse");
    env.get_template("template")
        .expect("template should be registered")
        .render(context)
        .expect("static template should render")
}

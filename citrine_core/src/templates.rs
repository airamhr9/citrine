use log::{debug, error};
use once_cell::sync::OnceCell;
use serde::Serialize;
use tera::{Context, Tera, Value};

use crate::configuration;

static TEMPLATES: OnceCell<Tera> = OnceCell::new();
//only for reloading on debug
static CALLBACK: OnceCell<fn(Tera) -> Tera> = OnceCell::new();

pub fn init_templates(configure_tera: fn(Tera) -> Tera) -> Result<(), tera::Error>
{
    //only for reloading on debug
    if cfg!(debug_assertions) && CALLBACK.set(configure_tera).is_err() {
        error!("Could not save templates configuration for template reload. Custom template functions may not work");
    }

    let mut tera = load_tera();

    for template in tera.get_template_names() {
        debug!("Loaded template {}", template);
    }

    tera = configure_tera(tera);

    debug!("Tera templates initialized");

    if TEMPLATES.set(tera).is_err() {
        Err(tera::Error::msg(
            "Could not initialize template engine configuration",
        ))
    } else {
        Ok(())
    }
}

fn load_tera() -> Tera {
    let mut template_folder = configuration::templates_folder_or_default();
    template_folder.push_str("/**/*");
    let mut tera = match Tera::new(&template_folder) {
        Ok(t) => t,
        Err(e) => {
            error!("Error intializing tera {}", e);
            Tera::default()
        }
    };
    tera.autoescape_on(vec![".html"]);
    tera
}

pub fn render_view(template_name: &str, data: &impl Serialize) -> Result<String, tera::Error> {
    let value = serde_json::to_value(data)?;
    if let Value::Array(_) = value {
        let msg = "Can't build a template context from a top level array. Make sure the data can be serialized as a JSON Object";
        error!("{}", msg);
        return Err(tera::Error::msg(msg));
    }
    render_view_with_context(template_name, &Context::from_value(value)?)
}

pub fn render_view_with_context(
    template_name: &str,
    context: &Context,
) -> Result<String, tera::Error> {
    if cfg!(debug_assertions) {
        //reload tera on debug mode to make development more bearable
        let mut tera = load_tera();
        if CALLBACK.get().is_some() {
            tera = CALLBACK.get().unwrap()(tera);
        }
        tera.render(template_name, context)
    } else {
        if TEMPLATES.get().is_none() {
            panic!("Tera template engine not initialized")
        }
        TEMPLATES.get().unwrap().render(template_name, context)
    }
}

use log::{debug, error};
use once_cell::sync::OnceCell;
use serde::Serialize;
use tera::{Context, Tera, Value};

static TEMPLATES: OnceCell<Tera> = OnceCell::new();
//only for reloading on debug
static CALLBACK: OnceCell<fn(Tera) -> Tera> = OnceCell::new();

pub fn init_templates(configure_tera: fn(Tera) -> Tera) -> Result<(), tera::Error>
{
    //only for reloading on debug
    if cfg!(debug_assertions) {
        if let Err(_) = CALLBACK.set(configure_tera) {
            error!("Could not save templates configuration for template reload. Custom template functions may not work");
        }
    }

    let mut tera = load_tera();

    for template in tera.get_template_names() {
        debug!("Loaded template {}", template);
    }

    tera = configure_tera(tera);

    debug!("Tera templates initialized");

    if let Err(_) = TEMPLATES.set(tera) {
        Err(tera::Error::msg(
            "Could not initialize template engine configuration",
        ))
    } else {
        Ok(())
    }
}

fn load_tera() -> Tera {
    // get this path from env variables
    let mut tera = match Tera::new("templates/**/*") {
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
    let value = serde_json::to_value(&data)?;
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
        // This should never happen as init_templates() is called on App initialization before
        // any request can be handled. Maybe can be removed
        if TEMPLATES.get().is_none() {
            panic!("Tera template engine not initialized")
        }
        TEMPLATES.get().unwrap().render(template_name, context)
    }
}
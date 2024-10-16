use log::{debug, error};
use serde::Serialize;
use tera::{Context, Tera, Value};

lazy_static! {
    static ref TEMPLATES: Tera = {
        //todo use an env variable for this and set the current value as default
        let mut tera = match Tera::new("templates/**/*") {
            Ok(t) => t,
            Err(e) => {
                error!("Error intializing tera {}", e);
                Tera::default()
            }
        };
        tera.autoescape_on(vec![".html"]);
        
        for template in tera.get_template_names() {
            debug!("Loaded template {}", template);
        }

        debug!("Tera templates initialized");

        tera
    };
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

pub fn render_view_with_context(template_name: &str, context: &Context) -> Result<String, tera::Error> {
    TEMPLATES.render(template_name, context)
}

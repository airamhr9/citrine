use std::env;

pub fn port_or_default() -> u16 {
    if let Ok(var) = env::var("CITRINE_PORT") {
        let parse_res = var.parse::<u16>();
        if let Err(e) = parse_res {
            panic!("Invalid value for citrine.port: {}", e);
        }
        parse_res.unwrap()
    } else {
        8080
    }
}

pub fn application_name_or_default() -> String {
    env::var("CITRINE_APP_NAME")
        .unwrap_or(env::var("CARGO_PKG_NAME").unwrap_or("Citrine Application".to_string()))
}

pub fn version() -> String {
    env::var("CARGO_PKG_VERSION").unwrap_or("0.0.1".to_string())
}

pub fn templates_enabled_or_default() -> bool {
    if let Ok(var) = env::var("CITRINE_TEMPLATES_ENABLED") {
        match var.to_lowercase().as_str() {
            "true" => return true,
            "false" => return false,
            _ => panic!("Invalid value for citrine.templates.enabled: {}", var),
        }
    }

    false
}

pub fn templates_folder_or_default() -> String {
    env::var("CITRINE_TEMPLATES_FOLDER").unwrap_or("templates".to_string())
}

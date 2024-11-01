use std::{env, fs, path::Path};

use log::debug;

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
            _ => panic!("Invalid value for CITRINE_TEMPLATES_ENABLED: {}", var),
        }
    }

    false
}

pub fn templates_folder_or_default() -> String {
    env::var("CITRINE_TEMPLATES_FOLDER").unwrap_or("templates".to_string())
}

pub fn banner_enabled() -> bool {
    if let Ok(var) = env::var("CITRINE_BANNER_ENABLED") {
        match var.to_lowercase().as_str() {
            "true" => return true,
            "false" => return false,
            _ => panic!("Invalid value for CITRINE_BANNER_ENABLED {}", var),
        }
    }

    true
}

pub fn banner() -> String {
    let banner_path = "./banner.txt";
    if !Path::new(banner_path).is_file() {
        default_banner().to_string()
    } else {
        let read_res = fs::read_to_string(banner_path);
        if let Err(e) = read_res {
            debug!("bannet.txt detected but it could not be read: {}", e);
            default_banner().to_string()
        } else {
            read_res.unwrap()
        }
    }
}

fn default_banner() -> &'static str {
    r"   ___  _  _          _
  / __\(_)| |_  _ __ (_) _ __    ___
 / /   | || __|| '__|| || '_ \  / _ \
/ /___ | || |_ | |   | || | | ||  __/
\____/ |_| \__||_|   |_||_| |_| \___|
"
}

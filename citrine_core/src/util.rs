use std::{sync::mpsc::{self, TryRecvError}, thread, time::Duration};

use log::debug;

pub fn get_max_age_from_reqwest(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let cache_control_header = headers.get("Cache-Control");
    if cache_control_header.is_none() {
        debug!("No Cache-Control header present.");
        return None;
    }
    let cache_control_header_str = cache_control_header.unwrap().to_str();
    if let Err(e) = cache_control_header_str {
        debug!("Could not read Cache-Control header {}", e);
        return None;
    }

    let tokens: Vec<&str> = cache_control_header_str.unwrap().split(',').collect();
    for token in tokens {
        let key_value: Vec<&str> = token.split('=').map(str::trim).collect();
        let key = key_value.first().unwrap();
        let val = key_value.get(1);

        if String::from("max-age").eq(&key.to_lowercase()) {
            match val {
                Some(value) => {
                    let max_age_num_value: Result<u64, _> = value.parse();
                    if max_age_num_value.is_err() {
                        debug!(
                            "Could not parse max_age value in Cache-Control header, value was {}",
                            key
                        );
                        return None;
                    }

                    return Some(Duration::from_secs(max_age_num_value.unwrap()));
                }
                None => return None,
            }
        }
    }

    debug!("Cache-Control header present but no max-age value was found");
    None
}

type Delay = Duration;
type Cancel = Box<dyn Fn() + Send>;

pub fn use_repeating_job<F>(job: F) -> Cancel
where
    F: Fn() -> Delay,
    F: Send + 'static,
{
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    thread::spawn(move || loop {
        let delay = job();
        thread::sleep(delay);

        if let Ok(_) | Err(TryRecvError::Disconnected) = shutdown_rx.try_recv() {
            break;
        }
    });

    Box::new(move || {
        println!("Stopping...");
        let _ = shutdown_tx.send("stop");
    })
}

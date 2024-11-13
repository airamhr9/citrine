use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};

use derive_more::derive::Display;
use hyper::Uri;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use log::debug;
use serde::Deserialize;
use tokio::task;

use crate::{
    security::security_configuration::{AuthClaims, AuthResult},
    util,
};

pub struct OIDCConfiguration {
    jwk_url: String,
    audience: HashSet<String>,
    issuers: HashSet<String>,
    jwks: Arc<RwLock<FetchJwkResult>>,
    cleanup: Mutex<Box<dyn Fn() + Send>>,
}

impl Drop for OIDCConfiguration {
    fn drop(&mut self) {
        // Stop the update thread when the updater is destructed
        let cleanup_fn = self.cleanup.lock().unwrap();
        cleanup_fn();
    }
}

impl OIDCConfiguration {
    pub async fn new(issuers: HashSet<Uri>, jwk_url: Uri, audience: HashSet<String>) -> Self {
        let jwk_url = jwk_url.to_string();
        let closure_jwk_url = jwk_url.clone();
        let fetch_jwks_res = task::spawn_blocking(move || Self::get_jwks(&closure_jwk_url)).await;
        if let Err(e) = fetch_jwks_res {
            panic!("Error fetching JWK {}", e);
        }
        let fetch_jwks_res = fetch_jwks_res.unwrap();
        if let Err(e) = fetch_jwks_res {
            panic!("Error fetching JWK {}", e);
        }
        let jwks = fetch_jwks_res.unwrap();
        let issuers = issuers.iter().map(|iss| iss.to_string()).collect();

        let mut config = OIDCConfiguration {
            jwks: Arc::new(RwLock::new(jwks)),
            jwk_url,
            audience,
            issuers,
            cleanup: Mutex::new(Box::new(|| {})),
        };

        config.periodic_update();
        config
    }

    fn periodic_update(&mut self) {
        let shared_jwks = self.jwks.clone();
        let jwk_url = self.jwk_url.clone();

        let stop = util::use_repeating_job(move || {
            debug!("Updating JWKs");
            match Self::get_jwks(&jwk_url) {
                Ok(jwks) => {
                    let mut current_jwks = shared_jwks.write().unwrap();
                    current_jwks.keys = jwks.keys;
                    current_jwks.validity = jwks.validity;

                    current_jwks.validity
                }
                Err(_) => Duration::from_secs(1000),
            }
        });

        let mut cleanup = self.cleanup.lock().unwrap();
        *cleanup = stop;
    }

    fn get_jwks(jwk_url: &str) -> Result<FetchJwkResult, FetchJwkError> {
        let jwk_url = reqwest::Url::parse(jwk_url).unwrap();
        let jwk_result = reqwest::blocking::get(jwk_url);
        if let Err(e) = jwk_result {
            debug!("Could not get JWK {}", e);
            return Err(FetchJwkError::new(&e));
        }
        let jwk_response = jwk_result.unwrap();
        let headers = jwk_response.headers();
        // todo configure default
        let validity = util::get_max_age_from_reqwest(headers).unwrap_or(Duration::from_secs(300));
        let jwk_parse_res = jwk_response.json::<JwkResponse>();
        if let Err(e) = jwk_parse_res {
            debug!("Could not parse JWK response {}", e);
            return Err(FetchJwkError::new(&e));
        }
        let mut keys_map = HashMap::new();
        for key in jwk_parse_res.unwrap().keys {
            keys_map.insert(String::clone(&key.kid), key);
        }
        Ok(FetchJwkResult {
            keys: keys_map,
            validity,
        })
    }

    pub fn authenticate(&self, token: &str) -> AuthResult {
        debug!("Using OIDC Authenticator");
        let split_token = token.split(" ");
        let token = split_token.last().unwrap_or("");

        let header_res = jsonwebtoken::decode_header(token);
        if let Err(e) = header_res {
            debug!("Error decoding token header: {}", e);
            return AuthResult::Denied;
        }
        let header = header_res.unwrap();
        if header.kid.is_none() {
            debug!("No KID found in header");
            return AuthResult::Denied;
        }
        let kid = header.kid.unwrap();
        let jwks = self.jwks.read().unwrap();
        let key_opt = jwks.keys.get(&kid);
        if key_opt.is_none() {
            debug!("No matching JWK key for token kid");
            return AuthResult::Denied;
        }
        let key = key_opt.unwrap();

        let algorithm_res = Algorithm::from_str(&key.alg);
        if let Err(e) = algorithm_res {
            debug!("Invalid token algorithm {}", e);
            return AuthResult::Denied;
        }
        let mut validation = Validation::new(algorithm_res.unwrap());
        validation.iss = Some(self.issuers.clone());
        validation.aud = Some(self.audience.clone());

        let decoding_key_res = DecodingKey::from_rsa_components(&key.n, &key.e);
        if let Err(e) = decoding_key_res {
            debug!("Could not build decoding key {}", e);
            return AuthResult::Denied;
        }

        let token_data =
            jsonwebtoken::decode::<AuthClaims>(token, &decoding_key_res.unwrap(), &validation);

        if token_data.is_err() {
            debug!("Error getting token data {:?}", token_data.err());
            AuthResult::Denied
        } else {
            debug!("Request allowed");
            AuthResult::Authenticated(token_data.unwrap().claims)
        }
    }
}

#[derive(Debug, Deserialize)]
struct JwkResponse {
    keys: Vec<JwkKey>,
}

#[derive(Debug, Deserialize)]
struct FetchJwkResult {
    keys: HashMap<String, JwkKey>,
    validity: Duration,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
struct JwkKey {
    pub e: String,
    pub alg: String,
    pub kty: String,
    pub kid: String,
    pub n: String,
}

#[derive(Debug, Display)]
struct FetchJwkError {
    cause: String,
}

impl FetchJwkError {
    pub fn new(e: &dyn std::error::Error) -> Self {
        FetchJwkError {
            cause: e.to_string(),
        }
    }
}

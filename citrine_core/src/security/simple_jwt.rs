use base64::Engine;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use log::debug;

use crate::security::security_configuration::AuthClaims;

use super::security_configuration::AuthResult;

pub enum JWTSecret {
    Plain(String),
    Base64(String),
}

impl JWTSecret {
    pub fn plain(secret: &str) -> Self {
        Self::Plain(secret.to_string())
    }

    pub fn base64_encoded(secret: &str) -> Self {
        Self::Base64(secret.to_string())
    }
}

#[derive(Debug)]
pub struct JWTConfiguration {
    secret: String,
    algorithm: Algorithm,
}

impl JWTConfiguration {
    pub fn new(secret: JWTSecret, algorithm: Algorithm) -> Self {
        let secret = match secret {
            JWTSecret::Plain(plain) => plain,
            JWTSecret::Base64(base64_encoded) => {
                let bytes_res = base64::prelude::BASE64_STANDARD.decode(base64_encoded);
                if let Err(e) = bytes_res {
                    panic!("Invalid Base64 JWT Secret {}", e);
                }
                let string_res = String::from_utf8(bytes_res.unwrap());
                if let Err(e) = string_res {
                    panic!("Invalid Base64 JWT Secret {}", e);
                }
                string_res.unwrap()
            }
        };

        JWTConfiguration { secret, algorithm }
    }

    pub fn authenticate(&self, token: &str) -> AuthResult {
        debug!("Using JWT Authenticator");
        let validation = Validation::new(self.algorithm);
        let split_token = token.split(" ");
        let token = split_token.last().unwrap_or("");

        let token_data = jsonwebtoken::decode::<AuthClaims>(
            token,
            &DecodingKey::from_secret(self.secret.as_ref()),
            &validation,
        );

        if token_data.is_err() {
            debug!("Error getting token data {:?}", token_data.err());
            AuthResult::Denied
        } else {
            debug!("Request allowed");
            AuthResult::Authenticated(token_data.unwrap().claims)
        }
    }
}

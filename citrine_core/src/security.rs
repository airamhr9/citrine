use hyper::header::{HeaderValue, AUTHORIZATION};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use log::debug;
use serde::{Deserialize, Serialize};

use crate::{
    request::RequestMetadata,
    request_matcher::{MethodMatcher, RequestMatcher},
};

pub struct SecurityConfiguration {
    rules: Vec<SecurityRule>,
}

impl SecurityConfiguration {
    pub fn new() -> Self {
        SecurityConfiguration { rules: vec![] }
    }

    pub fn add_rule(
        mut self,
        method_matcher: MethodMatcher,
        path_regex: &str,
        action: SecurityAction,
    ) -> Self {
        self.rules.push(SecurityRule::new(
            RequestMatcher::new(path_regex, method_matcher),
            action,
        ));
        self
    }

    pub fn authorize(&self, request: &RequestMetadata) -> AuthResult {
        debug!("Authorizing request {} {}", request.method, request.uri);
        for rule in self.rules.iter() {
            if rule.matches(request) {
                debug!("Found matching rule");
                return rule.get_auth_result(request);
            }
        }

        debug!("No matching rule, allowing request");
        AuthResult::Allowed
    }
}

impl Default for SecurityConfiguration {
    fn default() -> Self {
        Self::new()
    }
}

struct SecurityRule {
    request_matcher: RequestMatcher,
    action: SecurityAction,
}

impl SecurityRule {
    pub fn new(request_matcher: RequestMatcher, action: SecurityAction) -> Self {
        SecurityRule {
            request_matcher,
            action,
        }
    }

    pub fn matches(&self, request: &RequestMetadata) -> bool {
        self.request_matcher.matches(&request.method, &request.uri)
    }

    pub fn get_auth_result(&self, request: &RequestMetadata) -> AuthResult {
        self.action.apply(request)
    }
}

pub enum SecurityAction {
    Deny,
    Allow,
    Authenticate(Authenticator),
}

impl SecurityAction {
    pub fn apply(&self, request: &RequestMetadata) -> AuthResult {
        match self {
            Self::Deny => AuthResult::Denied,
            Self::Allow => AuthResult::Allowed,
            Self::Authenticate(authenticator) => authenticator.authenticate(request),
        }
    }
}

pub enum Authenticator {
    //todo add SAML
    JWT(JWTConfiguration),
    // This will receive a function that has the Authorization header as a param and returns
    // whether the request is allowed.
    Custom(fn(&HeaderValue) -> AuthResult),
}

impl Authenticator {
    pub fn authenticate(&self, request: &RequestMetadata) -> AuthResult {
        let authorization_header = request.headers.get(AUTHORIZATION);
        if authorization_header.is_none() {
            debug!("No Authorization header provided. Denying request");
            return AuthResult::Denied;
        }
        let authorization_header_str = authorization_header.unwrap().to_str();
        if authorization_header_str.is_err() {
            debug!("Invalid Authorization header provided. Denying request");
            return AuthResult::Denied;
        }

        match self {
            Authenticator::JWT(config) => config.authenticate(authorization_header_str.unwrap()),
            Authenticator::Custom(custom_auth_function) => {
                custom_auth_function(authorization_header.unwrap())
            }
        }
    }
}

pub struct JWTConfiguration {
    secret: String,
    algorithm: Algorithm,
}

impl JWTConfiguration {
    pub fn new(secret: &str, algorithm: Algorithm) -> Self {
        JWTConfiguration {
            secret: secret.to_string(),
            algorithm,
        }
    }

    fn authenticate(&self, token: &str) -> AuthResult {
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
            AuthResult::JWTAuthenticated(token_data.unwrap().claims)
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AuthClaims {
    pub sub: Option<String>,
    pub name: Option<String>,
    pub iat: Option<usize>,
    pub admin: Option<bool>,
    pub exp: Option<usize>,
    pub iss: Option<String>,
    pub nbf: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AuthResult {
    Denied,
    Allowed,
    JWTAuthenticated(AuthClaims),
    CustomAuthenticated(String),
}

impl AuthResult {
    pub fn get_claims(&self) -> Option<&AuthClaims> {
        match self {
            AuthResult::JWTAuthenticated(claims) => Some(claims),
            _ => None,
        }
    }
}

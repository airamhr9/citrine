use core::panic;
use hyper::{
    header::{HeaderValue, AUTHORIZATION},
    Method,
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use log::debug;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::request::Request;

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

    pub fn authorize(&self, request: &Request) -> AuthResult {
        debug!("Authorizing request {} {}", request.method, request.uri);
        for rule in self.rules.iter() {
            if rule.matches(request)  {
                debug!("Found matching rule");
                return rule.get_auth_result(request);
            }
        }

        debug!("No matching rule, allowing request");
        AuthResult::Allowed
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

    pub fn matches(&self, request: &Request) -> bool {
        self.request_matcher.matches(request)
    }

    pub fn get_auth_result(&self, request: &Request) -> AuthResult {
        self.action.apply(request)
    }
}

pub enum SecurityAction {
    Deny,
    Allow,
    Authenticate(Authenticator),
}

impl SecurityAction {
    pub fn apply(&self, request: &Request) -> AuthResult {
        match self {
            Self::Deny => AuthResult::Denied,
            Self::Allow => AuthResult::Allowed,
            Self::Authenticate(authenticator) => authenticator.authenticate(request),
        }
    }
}

pub enum MethodMatcher {
    One(Method),
    Multiple(Vec<Method>),
    All,
}

struct RequestMatcher {
    path_regex: Regex,
    method_matcher: MethodMatcher,
}

impl RequestMatcher {
    pub fn new(path_regex: &str, method_matcher: MethodMatcher) -> Self {
        let regex_res = Regex::new(path_regex);
        if let Err(e) = regex_res {
            panic!("Malformed request matcher in security configuration: {}", e);
        }
        RequestMatcher {
            path_regex: regex_res.unwrap(),
            method_matcher,
        }
    }

    pub fn matches(&self, request: &Request) -> bool {
        self.matches_method(&request.method) && self.path_regex.is_match(request.uri.path())
    }

    fn matches_method(&self, method: &Method) -> bool {
        match &self.method_matcher {
            MethodMatcher::All => true,
            MethodMatcher::One(m) => method == m,
            MethodMatcher::Multiple(methods) => methods.contains(&method),
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
    pub fn authenticate(&self, request: &Request) -> AuthResult {
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
    algorithm: Algorithm
}

impl JWTConfiguration {
    pub fn new(secret: &str, algorithm: Algorithm) -> Self {
        JWTConfiguration { secret: secret.to_string(), algorithm }
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
    CustomAuthenticated(String)
}

impl AuthResult {
    pub fn get_claims(&self) -> Option<&AuthClaims> {
        match self {
            AuthResult::JWTAuthenticated(claims) => Some(claims),
            _ => None
        }
    }
}

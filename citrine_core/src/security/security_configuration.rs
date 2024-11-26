use std::{collections::HashMap, fmt::Display};

use hyper::header::{HeaderValue, AUTHORIZATION};
use log::debug;

use crate::{
    request::RequestMetadata,
    request_matcher::{MethodMatcher, RequestMatcher},
};

use super::{oidc::OIDCConfiguration, simple_jwt::JWTConfiguration};

pub struct SecurityConfiguration {
    rules: Vec<SecurityRule>,
}

impl SecurityConfiguration {
    pub fn new() -> Self {
        SecurityConfiguration { rules: vec![] }
    }

    pub fn add_rule(
        mut self,
        rule: SecurityRule
    ) -> Self {
        self.rules.push(rule);
        self
    }

    pub fn authorize(&self, request: &RequestMetadata) -> AuthResult {
        debug!("Authorizing request {} {}", request.method, request.uri);
        for rule in self.rules.iter() {
            if rule.matches(request) {
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

pub struct SecurityRule {
    request_matchers: Vec<RequestMatcher>,
    action: SecurityAction,
}

impl Default for SecurityRule {
    fn default() -> Self {
        SecurityRule {
            request_matchers: vec![],
            action: SecurityAction::Allow,
        }
    }
}

impl SecurityRule {
    pub fn new() -> Self {
        SecurityRule::default()
    }

    pub fn add_matcher(mut self, method_matcher: MethodMatcher, path_regex: &str) -> Self {
        self.request_matchers
            .push(RequestMatcher::new(path_regex, method_matcher));
        self
    }

    pub fn execute_action(mut self, action: SecurityAction) -> Self {
        self.action = action;
        self
    }


    pub fn matches(&self, request: &RequestMetadata) -> bool {
        for request_matcher in self.request_matchers.iter() {
            if request_matcher.matches(&request.method, &request.uri) {
                debug!(
                    "Found matching rule with matcher: {} | {}",
                    request_matcher, self.action
                );
                return true;
            }
        }
        false
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

impl Display for SecurityAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Deny => write!(f, "Deny"),
            Self::Allow => write!(f, "Allow"),
            Self::Authenticate(authenticator) => write!(f, "Authenticate with {}", authenticator),
        }
    }
}

pub type AuthClaims = HashMap<String, serde_json::Value>;

#[derive(Debug, Clone, PartialEq)]
pub enum AuthResult {
    Denied,
    Allowed,
    Authenticated(AuthClaims),
    CustomAuthenticated(String),
}

impl AuthResult {
    pub fn get_claims(&self) -> Option<&AuthClaims> {
        match self {
            AuthResult::Authenticated(claims) => Some(claims),
            _ => None,
        }
    }
}

pub enum Authenticator {
    OIDC(OIDCConfiguration),
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
            Authenticator::OIDC(config) => config.authenticate(authorization_header_str.unwrap()),
            Authenticator::Custom(custom_auth_function) => {
                custom_auth_function(authorization_header.unwrap())
            }
        }
    }
}

impl Display for Authenticator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JWT(_) => write!(f, "JWT"),
            Self::OIDC(_) => write!(f, "OIDC"),
            Self::Custom(_) => write!(f, "Custom"),
        }
    }
}

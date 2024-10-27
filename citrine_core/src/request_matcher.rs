use std::fmt::Display;

use hyper::{Method, Uri};
use regex::Regex;

pub enum MethodMatcher {
    One(Method),
    Multiple(Vec<Method>),
    All,
}

pub struct RequestMatcher {
    path_regex: Regex,
    method_matcher: MethodMatcher,
}

impl RequestMatcher {
    pub fn new(path_regex: &str, method_matcher: MethodMatcher) -> Self {
        let regex_res = Regex::new(path_regex);
        if let Err(e) = regex_res {
            panic!("Malformed request matcher: {}", e);
        }
        RequestMatcher {
            path_regex: regex_res.unwrap(),
            method_matcher,
        }
    }

    fn matches_method(&self, method: &Method) -> bool {
        match &self.method_matcher {
            MethodMatcher::All => true,
            MethodMatcher::One(m) => method == m,
            MethodMatcher::Multiple(methods) => methods.contains(method),
        }
    }

    pub fn matches(&self, method: &Method, uri: &Uri) -> bool {
        self.matches_method(method) && self.path_regex.is_match(uri.path())
    }

}

impl Display for RequestMatcher {
fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{} {}", self.method_matcher, self.path_regex)
}
}

impl Display for MethodMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "All HTTP methods"),
            Self::One(method) => write!(f, "{}", method),
            Self::Multiple(methods) => write!(f, "{:?}", methods),
        }
    }
}

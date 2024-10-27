use crate::{
    request::Request,
    request_matcher::{MethodMatcher, RequestMatcher},
};

#[derive(Default)]
pub struct RequestMiddleware {
    functions: Vec<Middleware>,
}

struct Middleware {
    request_matcher: RequestMatcher,
    function: fn(Request) -> Request,
}

impl RequestMiddleware {
    pub fn new() -> Self {
        RequestMiddleware { functions: vec![] }
    }

    pub fn add_middleware(
        mut self,
        method_matcher: MethodMatcher,
        path_regex: &str,
        middleware: fn(Request) -> Request,
    ) -> Self {
        self.functions.push(Middleware::new(
            RequestMatcher::new(path_regex, method_matcher),
            middleware,
        ));
        self
    }

    pub fn process(&self, request: Request) -> Request {
        for middleware in self.functions.iter() {
            if middleware
                .request_matcher
                .matches(&request.method, &request.uri)
            {
                return (middleware.function)(request);
            }
        }
        request
    }
}

impl Middleware {
    fn new(request_matcher: RequestMatcher, function: fn(Request) -> Request) -> Self {
        Middleware {
            request_matcher,
            function,
        }
    }
}

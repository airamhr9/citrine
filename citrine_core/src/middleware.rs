use crate::request::Request;

#[derive(Default)]
pub struct RequestMiddleware {
    functions: Vec<fn(Request) -> Request>
}

impl RequestMiddleware {
    pub fn new(middleware: fn(Request) -> Request) -> Self {
        RequestMiddleware { functions: vec![middleware] }
    }

    pub fn then(mut self, middleware: fn(Request) -> Request) -> Self {
        self.functions.push(middleware);
        self
    }

    pub fn process(&self, mut request: Request) -> Request {
        for middleware in self.functions.iter() {
            request = middleware(request);
        }
        request
    }
}

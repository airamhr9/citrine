use hyper::body::Incoming;
use hyper::Method;
use hyper_staticfile::Static;
use log::debug;
use std::collections::HashMap;
use std::sync::Arc;

use crate::error::ErrorType;
use crate::error::RequestError;
use crate::error::ServerError;
use crate::request::Request;
use crate::response::Response;

pub type RequestHandler<T> = fn(Arc<T>, Request) -> Response;

#[derive(Clone)]
pub struct StaticFileServer {
    pub url_base_path: String,
    pub server: Option<Static>
}

impl StaticFileServer {
    pub fn new(url_base_path: &str, server: Static) -> Self {
        StaticFileServer {
            url_base_path: url_base_path.to_string(),
            server: Some(server)
        }
    }

    pub fn can_serve_request(&self, request: &hyper::Request<Incoming>) -> bool {
        self.server.is_some() && request.method() == Method::GET
            && request.uri().path().starts_with(&self.url_base_path)
    }
}

impl Default for StaticFileServer {
    fn default() -> Self {
        StaticFileServer {
            url_base_path: String::new(),
            server: None
        }
    }
}

pub struct Router<T: Send + Sync + 'static> {
    pub base_path: String,
    pub routes: Vec<Route<T>>,
}

pub struct Route<T: Send + Sync + 'static> {
    pub method: Method,
    pub path: String,
    pub handler: RequestHandler<T>,
}

impl<T> Router<T>
where
    T: Send + Sync + 'static,
{
    pub fn new() -> Self {
        return Router {
            base_path: String::new(),
            routes: Vec::new(),
        };
    }

    pub fn add_router(mut self, nested: Router<T>) -> Self {
        for route in nested.routes.iter() {
            self = self.add_route(route.method.clone(), &route.path, route.handler);
        }

        self
    }

    pub fn base_path(base_path: &str) -> Self {
        return Router {
            base_path: base_path.to_string(),
            routes: Vec::new(),
        };
    }

    pub fn add_route(mut self, method: Method, path: &str, handler: RequestHandler<T>) -> Self {
        let mut real_path = format!("{}{}", self.base_path, path);
        if real_path == "" {
            real_path = "/".to_string();
        }
        self.routes.push(Route {
            method,
            path: real_path,
            handler,
        });
        self
    }
}

pub struct InternalRouter<T: Send + Sync + 'static> {
    routes: HashMap<Method, HashMap<String, RouterNode<T>>>,
    state: Arc<T>,
}

pub struct RouterNode<T: Send + Sync + 'static> {
    routes: HashMap<String, RouterNode<T>>,
    handler: Option<RequestHandler<T>>,
    variable: Option<String>,
}

impl<T> InternalRouter<T>
where
    T: Send + Sync + 'static,
{
    pub fn new_stateful(state: T) -> InternalRouter<T> {
        InternalRouter {
            routes: HashMap::new(),
            state: Arc::new(state),
        }
    }

    pub fn from(router: Router<T>, state: T) -> Result<InternalRouter<T>, ServerError> {
        let mut internal_router = InternalRouter::new_stateful(state);

        for route in router.routes {
            if let Err(e) = internal_router.add_route(route.method, &route.path, route.handler)
            {
                return Err(e);
            }
        }

        Ok(internal_router)
    }

    pub fn add_route(
        &mut self,
        method: Method,
        route: &str,
        handler: RequestHandler<T>,
    ) -> Result<(), ServerError> {
        debug!("Binding route {} {}", method, route);
        let routes: Vec<String> = route.split("/").map(|s| s.to_string()).collect();

        let method_map = self.routes.get(&method);
        if method_map.is_none() {
            self.routes
                .insert(method.clone(), HashMap::<String, RouterNode<T>>::new());
        }
        let mut current = self.routes.get_mut(&method).unwrap();

        for (i, elem) in routes.iter().enumerate() {
            let key: String;
            let variable: Option<String>;

            if elem.starts_with(":") {
                // variable path element
                if elem.len() <= 1 {
                    return Err(ServerError::from(format!(
                        "Malformed path: Variable without name in path {}",
                        route
                    )));
                }

                //todo optimize this
                key = "VARIABLE".to_string();
                variable = Some(elem[1..].to_string());
            } else {
                // normal path element
                key = elem.to_string();
                variable = None;
            }

            if !current.contains_key(&key) {
                let node = RouterNode {
                    routes: HashMap::new(),
                    handler: None,
                    variable,
                };
                current.insert(key.clone(), node);
                if i == routes.len() - 1 {
                    let inserted_node = current.get_mut(&key).unwrap();
                    inserted_node.handler = Some(handler);
                    break;
                }
                current = &mut current.get_mut(&key).unwrap().routes;
            } else {
                let node = current.get_mut(&key).unwrap();
                if i == routes.len() - 1 {
                    if node.handler.is_some() {
                        return Err(ServerError::from(format!(
                            "{} {} is already already defined",
                            method, route
                        )));
                    }
                    node.handler = Some(handler);
                    break;
                }
                current = &mut node.routes;
            }
        }

        Ok(())
    }

    // All request errors are turned into responses in the caller function
    //
    // The point of returning here as an error is to both avoid calling the response interceptor
    // in the case of an error and to give the flexibility to later on add a global error handler
    pub fn run(&self, mut req: Request) -> Result<(Request, Response), RequestError> {
        let mut path_variables = HashMap::<String, String>::new();

        let method_map = self.routes.get(&req.method);
        if method_map.is_none() {
            return Err(RequestError::with_message(
                ErrorType::MethodNotAllowed,
                &format!("{} {}", req.method, req.uri.path()),
            ));
        }

        let routes: Vec<String> = req.uri.path().split("/").map(|s| s.to_string()).collect();
        let mut current = self.routes.get(&req.method).unwrap();
        for (i, elem) in routes.iter().enumerate() {
            let mut opt_node = current.get(elem);
            //no match for this node
            if opt_node.is_none() {
                //let's try to match a variable
                opt_node = current.get("VARIABLE");

                //can't match this route
                if opt_node.is_none() {
                    return Err(RequestError::with_message(
                        ErrorType::NotFound,
                        req.uri.path(),
                    ));
                }
            }
            let node = opt_node.unwrap();
            if node.variable.is_some() {
                // can this be optimized?
                path_variables.insert(node.variable.clone().unwrap(), elem.clone());
            }
            if i == routes.len() - 1 {
                if let Some(function) = node.handler.as_ref() {
                    req.set_path_variables(path_variables);
                    //optimize this
                    return Ok((req.clone(), function(self.state.clone(), req)));
                } else {
                    return Err(RequestError::with_message(
                        ErrorType::NotFound,
                        req.uri.path(),
                    ));
                }
            }
            current = &node.routes;
        }

        return Err(RequestError::with_message(
            ErrorType::NotFound,
            req.uri.path(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use hyper::{HeaderMap, StatusCode, Uri};

    use super::*;

    struct StateTest {
    }

    impl Default for StateTest {
        fn default() -> Self {
            StateTest { }
        }
    }

    #[test]
    fn router_test() {
        let mut router = InternalRouter::new_stateful(StateTest::default());
        if let Err(e) = router.add_route(Method::GET, "/hello", |state, _| {
            return Response::new(StatusCode::OK).json("Hello world");
        }) {
            panic!("{}", e)
        }

        if let Err(e) = router.add_route(Method::POST, "/hello/other", |state, _| {
            return Response::new(StatusCode::OK).json("Hello world");
        }) {
            panic!("{}", e)
        }

        if let Err(e) = router.add_route(Method::GET, "/hi/other", |state, _| {
            return Response::new(StatusCode::OK).json("Hello world");
        }) {
            panic!("{}", e)
        }

        let tabs = 0;
        for (key, value) in &router.routes {
            println!("{}", key);
            print(value, tabs + 1);
        }

        let uri1 = Uri::from_static("http://domain.com/hello");
        let req1: Request = Request::new(Method::GET, uri1, "Body".to_string(), HeaderMap::new());
        let uri2 = Uri::from_static("http://domain.com/hello/other");
        let req2: Request = Request::new(Method::POST, uri2, "Body".to_string(), HeaderMap::new());
        let uri3 = Uri::from_static("http://domain.com/hi/other");
        let req3: Request = Request::new(Method::GET, uri3, "Body".to_string(), HeaderMap::new());
        let uri4 = Uri::from_static("http://domain.com/hi/other");
        let req4: Request = Request::new(Method::PUT, uri4, "Body".to_string(), HeaderMap::new());

        let _ = router.run(req1);
        let _ = router.run(req2);
        let _ = router.run(req3);
        let _ = router.run(req4);
    }

    fn print(map: &HashMap<String, RouterNode<StateTest>>, tabs: usize) {
        for (key2, value2) in map {
            println!(
                "{} {}: {:#?}",
                "  ".repeat(tabs),
                key2,
                value2.handler.is_some()
            );
            if !value2.routes.is_empty() {
                print(&value2.routes, tabs + 1);
            }
        }
    }
}

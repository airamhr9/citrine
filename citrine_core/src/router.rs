use hyper::header::CONTENT_TYPE;
use hyper::Method;
use log::debug;
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::Arc;

use crate::error::ErrorType;
use crate::error::RequestError;
use crate::error::ServerError;
use crate::request::ContentType;
use crate::request::Request;
use crate::response::Response;

pub type RequestHandler<T> = fn(Arc<T>, Request) -> Response;

pub struct Router<T: Send + Sync + 'static> {
    pub base_path: String,
    pub routes: Vec<Route<T>>,
}

pub struct Route<T: Send + Sync + 'static> {
    pub method: Method,
    pub path: String,
    pub handler: RequestHandler<T>,
    pub accepts_type: Accepts,
}

#[derive(Clone, Debug)]
pub enum Accepts {
    None,
    One(ContentType),
    Multiple(Vec<ContentType>),
}

impl Accepts {
    pub fn get_matching(&self, req: &Request) -> Option<ContentType> {
        if let Some(content_type) = req.headers.get(CONTENT_TYPE) {
            let content_type = content_type.to_str().unwrap();
            return match self {
                Accepts::One(encoding) => {
                    if encoding.is_valid(content_type) {
                        Some(*encoding)
                    } else {
                        None
                    }
                }
                Accepts::Multiple(encodings) => {
                    for encoding in encodings {
                        if encoding.is_valid(content_type) {
                            return Some(*encoding);
                        }
                    }
                    None
                }
                Accepts::None => None,
            };
        }

        None
    }
}

impl Display for Accepts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::One(content_type) => write!(f, "Accepts: {}", content_type.as_header_value()),
            Self::Multiple(types) => write!(
                f,
                "Accepts any: {}",
                types
                    .iter()
                    .map(|t| t.as_header_value())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
        }
    }
}

impl<T> Router<T>
where
    T: Send + Sync + 'static,
{
    pub fn new() -> Self {
        Router {
            base_path: String::new(),
            routes: Vec::new(),
        }
    }

    pub fn add_router(mut self, nested: Router<T>) -> Self {
        for route in nested.routes.iter() {
            self = self.add_route(
                route.method.clone(),
                &route.path,
                route.handler,
                route.accepts_type.clone(),
            );
        }

        self
    }

    pub fn base_path(base_path: &str) -> Self {
        Router {
            base_path: base_path.to_string(),
            routes: Vec::new(),
        }
    }

    pub fn add_route(
        mut self,
        method: Method,
        path: &str,
        handler: RequestHandler<T>,
        accepts_type: Accepts,
    ) -> Self {
        let mut real_path = format!("{}{}", self.base_path, path);
        if real_path.is_empty() {
            real_path = "/".to_string();
        }
        self.routes.push(Route {
            method,
            path: real_path,
            handler,
            accepts_type,
        });
        self
    }

    pub fn get(self, path: &str, handler: RequestHandler<T>) -> Self {
        self.add_route(Method::GET, path, handler, Accepts::None)
    }

    pub fn post(self, path: &str, handler: RequestHandler<T>) -> Self {
        self.add_route(Method::POST, path, handler, Accepts::One(ContentType::Json))
    }

    pub fn put(self, path: &str, handler: RequestHandler<T>) -> Self {
        self.add_route(Method::PUT, path, handler, Accepts::One(ContentType::Json))
    }

    pub fn patch(self, path: &str, handler: RequestHandler<T>) -> Self {
        self.add_route(
            Method::PATCH,
            path,
            handler,
            Accepts::One(ContentType::Json),
        )
    }

    pub fn delete(self, path: &str, handler: RequestHandler<T>) -> Self {
        self.add_route(
            Method::DELETE,
            path,
            handler,
            Accepts::One(ContentType::Json),
        )
    }
}

impl<T> Default for Router<T>
where
    T: Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

pub struct InternalRouter<T: Send + Sync + 'static> {
    routes: HashMap<Method, HashMap<String, RouterNode<T>>>,
}

pub struct RouterNode<T: Send + Sync + 'static> {
    routes: HashMap<String, RouterNode<T>>,
    handler: Option<RequestHandler<T>>,
    variable: Option<String>,
    accepts_type: Accepts,
}

impl<T> InternalRouter<T>
where
    T: Send + Sync + 'static,
{
    pub fn new() -> InternalRouter<T> {
        InternalRouter {
            routes: HashMap::new(),
        }
    }

    pub fn from(router: Router<T>) -> Result<InternalRouter<T>, ServerError> {
        let mut internal_router = InternalRouter::new();

        for route in router.routes {
            internal_router.add_route(route)?;
        }

        Ok(internal_router)
    }

    pub fn add_route(&mut self, route: Route<T>) -> Result<(), ServerError> {
        debug!("Binding route {} {}", route.method, route.path);
        let routes: Vec<String> = route.path.split("/").map(|s| s.to_string()).collect();

        let method_map = self.routes.get(&route.method);
        if method_map.is_none() {
            self.routes.insert(
                route.method.clone(),
                HashMap::<String, RouterNode<T>>::new(),
            );
        }
        let mut current = self.routes.get_mut(&route.method).unwrap();

        for (i, elem) in routes.iter().enumerate() {
            let key: String;
            let variable: Option<String>;

            if let Some(variable_name) = elem.strip_prefix(":") {
                if elem.len() <= 1 {
                    return Err(ServerError::from(format!(
                        "Malformed path: Variable without name in path {}",
                        route.path
                    )));
                }

                //todo optimize this
                key = "VARIABLE".to_string();
                variable = Some(variable_name.to_string())
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
                    accepts_type: Accepts::None,
                };
                current.insert(key.clone(), node);
                if i == routes.len() - 1 {
                    // Node with handler is inserted
                    let inserted_node = current.get_mut(&key).unwrap();
                    inserted_node.handler = Some(route.handler);
                    inserted_node.accepts_type = route.accepts_type;
                    break;
                }
                current = &mut current.get_mut(&key).unwrap().routes;
            } else {
                let node = current.get_mut(&key).unwrap();
                if i == routes.len() - 1 {
                    if node.handler.is_some() {
                        return Err(ServerError::from(format!(
                            "{} {} is already already defined",
                            route.method, route.path
                        )));
                    }
                    node.handler = Some(route.handler);
                    break;
                }
                current = &mut node.routes;
            }
        }

        Ok(())
    }

    pub fn run(&self, mut req: Request, context: Arc<T>) -> (Request, Response) {
        let mut path_variables = HashMap::<String, String>::new();

        let method_map = self.routes.get(&req.method);
        if method_map.is_none() {
            let path = req.uri.path().to_owned();
            let method = req.method.clone();
            return (
                req,
                RequestError::with_message(
                    ErrorType::MethodNotAllowed,
                    &format!("{} {}", method, &path),
                )
                .into(),
            );
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
                    let path = req.uri.path().to_owned();
                    return (
                        req,
                        RequestError::with_message(ErrorType::NotFound, &path).into(),
                    );
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

                    let content_type_opt = node.accepts_type.get_matching(&req);
                    // If we have a GET or don't have a body ignore this
                    if req.get_body_raw().is_some() {
                        // Matches if request Content-Type is compatible with the route
                        if let Some(content_type) = content_type_opt {
                            req.set_content_type(content_type);
                        } else {
                            return (
                                req,
                                RequestError::with_message(
                                    ErrorType::UnsupportedMediaType,
                                    &node.accepts_type.to_string(),
                                )
                                .into(),
                            );
                        }
                    }
                    // The handler has found a valid route
                    return (req.clone(), function(context.clone(), req));
                } else {
                    let path = req.uri.path().to_owned();
                    return (
                        req,
                        RequestError::with_message(ErrorType::NotFound, &path).into(),
                    );
                }
            }
            current = &node.routes;
        }

        let path = req.uri.path().to_owned();
        (
            req,
            RequestError::with_message(ErrorType::NotFound, &path).into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use hyper::{HeaderMap, StatusCode, Uri};

    use crate::security::AuthResult;

    use super::*;

    struct ContextTest {}

    impl Default for ContextTest {
        fn default() -> Self {
            ContextTest {}
        }
    }

    #[test]
    fn router_test() {
        let mut router = InternalRouter::new();
        let route = Route {
            method: Method::GET,
            path: "/hello".to_string(),
            handler: |_, _| {
                return Response::new(StatusCode::OK).json("Hello world");
            },
            accepts_type: Accepts::None,
        };
        if let Err(e) = router.add_route(route) {
            panic!("{}", e)
        }
        let route = Route {
            method: Method::POST,
            path: "/hello/other".to_string(),
            handler: |_, _| {
                return Response::new(StatusCode::OK).json("Hello world");
            },
            accepts_type: Accepts::One(ContentType::Json),
        };
        if let Err(e) = router.add_route(route) {
            panic!("{}", e)
        }
        let route = Route {
            method: Method::GET,
            path: "/hi/other".to_string(),
            handler: |_, _| {
                return Response::new(StatusCode::OK).json("Hello world");
            },
            accepts_type: Accepts::None,
        };
        if let Err(e) = router.add_route(route) {
            panic!("{}", e)
        }

        let tabs = 0;
        for (key, value) in &router.routes {
            println!("{}", key);
            print(value, tabs + 1);
        }

        let uri1 = Uri::from_static("http://domain.com/hello");
        let req1: Request = Request::new(
            Method::GET,
            uri1,
            "Body".to_string(),
            HeaderMap::new(),
            AuthResult::Allowed,
        );
        let uri2 = Uri::from_static("http://domain.com/hello/other");
        let req2: Request = Request::new(
            Method::POST,
            uri2,
            "Body".to_string(),
            HeaderMap::new(),
            AuthResult::Allowed,
        );
        let uri3 = Uri::from_static("http://domain.com/hi/other");
        let req3: Request = Request::new(
            Method::GET,
            uri3,
            "Body".to_string(),
            HeaderMap::new(),
            AuthResult::Allowed,
        );
        let uri4 = Uri::from_static("http://domain.com/hi/other");
        let req4: Request = Request::new(
            Method::PUT,
            uri4,
            "Body".to_string(),
            HeaderMap::new(),
            AuthResult::Allowed,
        );

        let context = Arc::new(ContextTest {});

        let _ = router.run(req1, context.clone());
        let _ = router.run(req2, context.clone());
        let _ = router.run(req3, context.clone());
        let _ = router.run(req4, context.clone());
    }

    fn print(map: &HashMap<String, RouterNode<ContextTest>>, tabs: usize) {
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

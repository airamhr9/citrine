use log::info;

use crate::{
    error::ServerError, request::Request, response::Response, router::{InternalRouter, Router}
};

struct Application<T: Send + Sync + 'static> {
    name: String,
    version: String,
    port: u16,
    interceptor: Option<fn(&Request, &Response)>,
    router: InternalRouter<T>,
}

impl<T> Application<T>
where
    T: Send + Sync + 'static,
{
    pub async fn start(self) -> Result<(), ServerError> {
        info!(
            "Starting application {} v{} (via Framework)",
            self.name, self.version
        );

        crate::server::start(
            self.port,
            self.interceptor,
            self.router,
        )
        .await;

        Result::Ok(())
    }
}

pub struct ApplicationBuilder<T: Send + Sync + 'static> {
    name: String,
    version: String,
    port: u16,
    interceptor: Option<fn(&Request, &Response)>,
    state: T,
    routes: Vec<Router<T>>,
}

impl<T> ApplicationBuilder<T>
where
    T: Send + Sync + 'static,
{
    pub fn new() -> ApplicationBuilder<T>
    where
        T: Default,
    {
        ApplicationBuilder::default()
    }

    pub fn name(mut self, name: &str) -> ApplicationBuilder<T> {
        self.name = name.to_string();
        self
    }

    pub fn version(mut self, version: &str) -> ApplicationBuilder<T> {
        self.version = version.to_string();
        self
    }

    pub fn port(mut self, port: u16) -> ApplicationBuilder<T> {
        self.port = port;
        self
    }

    pub fn interceptor(
        mut self,
        interceptor: fn(&Request, &Response),
    ) -> ApplicationBuilder<T> {
        self.interceptor = Some(interceptor);
        self
    }

    pub fn state(mut self, state: T) -> ApplicationBuilder<T> {
        self.state = state;
        self
    }

    pub fn add_routes(mut self, router: Router<T>) -> ApplicationBuilder<T> {
        self.routes.push(router);
        self
    }

    pub async fn start(self) -> Result<(), ServerError> {
        let internal_router_res = InternalRouter::from(self.routes, self.state);
        if let Err(e) = internal_router_res {
            return Err(ServerError::from(e));
        }
        Application {
            name: self.name,
            version: self.version,
            port: self.port,
            interceptor: self.interceptor,
            router: internal_router_res.unwrap(),
        }
        .start()
        .await
    }
}

impl<T: Default> Default for ApplicationBuilder<T>
where
    T: Send + Sync + 'static,
{
    fn default() -> ApplicationBuilder<T> {
        ApplicationBuilder {
            name: "Framework Application".to_string(),
            version: "0.0.1".to_string(),
            port: 8080,
            interceptor: None,
            routes: vec![],
            state: T::default(),
        }
    }
}

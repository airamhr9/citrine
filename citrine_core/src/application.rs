use log::info;
use tera::Tera;

use crate::{
    configuration,
    error::ServerError,
    middleware::RequestMiddleware,
    request::Request,
    response::Response,
    router::{InternalRouter, Router},
    security::security_configuration::SecurityConfiguration,
    server::RequestPipelineConfiguration,
    static_file_server::StaticFileServer,
    templates,
};

pub struct Application<T: Send + Sync + 'static> {
    name: String,
    version: String,
    port: u16,
    context: T,
    request_middleware: RequestMiddleware,
    response_interceptor: fn(&Request, &Response),
    router: InternalRouter<T>,
    load_templates: bool,
    configure_tera: fn(Tera) -> Tera,
    security_configuration: SecurityConfiguration,
    static_file_server: StaticFileServer,
}

impl<T> Application<T>
where
    T: Send + Sync + 'static,
{
    pub fn builder() -> ApplicationBuilder<T>
    where
        T: Default,
    {
        ApplicationBuilder::default()
    }

    pub async fn start(self) -> Result<(), ServerError> {
        if self.load_templates {
            if let Err(e) = templates::init_templates(self.configure_tera) {
                panic!("Error loading templates: {}", e);
            }
        }

        if configuration::banner_enabled() {
            println!("{}", configuration::banner());
        }
        info!(
            "Started application {} v{} (via Citrine)",
            self.name, self.version
        );

        crate::server::start(
            self.port,
            RequestPipelineConfiguration::new(
                self.response_interceptor,
                self.router,
                self.security_configuration,
                self.static_file_server,
                self.request_middleware,
                self.context,
            ),
        )
        .await;

        Result::Ok(())
    }
}

pub struct ApplicationBuilder<T: Send + Sync + 'static> {
    name: String,
    version: String,
    port: u16,
    context: T,
    request_middleware: RequestMiddleware,
    response_interceptor: fn(&Request, &Response),
    router: Router<T>,
    load_templates: bool,
    configure_tera: fn(Tera) -> Tera,
    security_configuration: SecurityConfiguration,
    static_file_server: StaticFileServer,
}

impl<T> ApplicationBuilder<T>
where
    T: Send + Sync + 'static,
{
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

    pub fn response_interceptor(
        mut self,
        response_interceptor: fn(&Request, &Response),
    ) -> ApplicationBuilder<T> {
        self.response_interceptor = response_interceptor;
        self
    }

    pub fn context(mut self, context: T) -> ApplicationBuilder<T> {
        self.context = context;
        self
    }

    pub fn security_configuration(
        mut self,
        security_configuration: SecurityConfiguration,
    ) -> ApplicationBuilder<T> {
        self.security_configuration = security_configuration;
        self
    }

    pub fn router(mut self, router: Router<T>) -> ApplicationBuilder<T> {
        self.router = router;
        self
    }

    /*
     * Tera will need to be configured when not in debug mode.
     * As of now, to make development easier, tera is reloaded in every template request
     * when running with debug_assertions to reflect changes in template code, but this will not
     * be the case when running in production mode
     */
    pub fn configure_tera(mut self, configuration: fn(Tera) -> Tera) -> Self {
        self.configure_tera = configuration;
        // doesn't make sense to configure tera and not enable it
        self.load_templates = true;
        self
    }

    pub fn serve_static_files(mut self, static_file_server: StaticFileServer) -> Self {
        self.static_file_server = static_file_server;
        self
    }

    pub fn load_templates(mut self) -> Self {
        self.load_templates = true;
        self
    }

    pub fn request_middleware(mut self, request_middleware: RequestMiddleware) -> Self {
        self.request_middleware = request_middleware;
        self
    }

    pub async fn start(self) -> Result<(), ServerError> {
        let internal_router_res = InternalRouter::from(self.router);
        if let Err(e) = internal_router_res {
            return Err(ServerError::from(e));
        }
        Application {
            name: self.name,
            version: self.version,
            port: self.port,
            context: self.context,
            request_middleware: self.request_middleware,
            response_interceptor: self.response_interceptor,
            router: internal_router_res.unwrap(),
            load_templates: self.load_templates,
            configure_tera: self.configure_tera,
            security_configuration: self.security_configuration,
            static_file_server: self.static_file_server,
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
            name: configuration::application_name_or_default(),
            version: configuration::version(),
            port: configuration::port_or_default(),
            context: T::default(),
            request_middleware: RequestMiddleware::default(),
            response_interceptor: |_, _| {},
            router: Router::new(),
            load_templates: configuration::templates_enabled_or_default(),
            configure_tera: |t| t,
            security_configuration: SecurityConfiguration::new(),
            static_file_server: StaticFileServer::default(),
        }
    }
}

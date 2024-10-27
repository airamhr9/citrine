use http_body_util::Full;
use hyper::service::service_fn;
use hyper::{body::Bytes, server::conn::http1};
use hyper_util::rt::TokioIo;
use hyper_util::server::graceful::GracefulShutdown;
use log::{error, info};
use std::net::SocketAddr;
use std::process::exit;
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::error::{ErrorType, RequestError, ServerError};
use crate::request::{Request, RequestMetadata}; 
use crate::response::Response;
use crate::router::InternalRouter;
use crate::security::{AuthResult, SecurityConfiguration};
use crate::static_file_server::StaticFileServer;

pub struct RequestPipelineConfiguration<T: 'static + Send + Sync> {
    interceptor: fn(&Request, &Response),
    router: InternalRouter<T>,
    security_configuration: SecurityConfiguration,
    static_file_server: StaticFileServer,
    context: Arc<T> 
}

impl<T> RequestPipelineConfiguration<T>
where
    T: 'static + Send + Sync,
{
    pub fn new(
        interceptor: fn(&Request, &Response),
        router: InternalRouter<T>,
        security_configuration: SecurityConfiguration,
        static_file_server: StaticFileServer,
        context: T
    ) -> Self {
        RequestPipelineConfiguration {
            interceptor,
            router,
            security_configuration,
            static_file_server,
            context: Arc::new(context)
        }
    }
}

pub async fn start<T>(port: u16, config: RequestPipelineConfiguration<T>)
where
    T: 'static + Sync + Send,
{
    let listener: TcpListener;
    match TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))).await {
        Ok(tcp_listener) => listener = tcp_listener,
        Err(_) => {
            error!("Error binding port {}", port);
            exit(1)
        }
    }
    info!("Listening in port {}", port);

    let http = http1::Builder::new();

    let graceful_shutdown = GracefulShutdown::new();

    let config = Arc::new(config);

    let mut signal = std::pin::pin!(shutdown_signal());

    loop {
        tokio::select! {
            Ok((stream, _addr)) = listener.accept() => {
                let io = TokioIo::new(stream);

                //Check if we can avoid the double cloning
                let request_config = config.clone();
                let svc = service_fn(move |request| {
                    handle_request(request, request_config.clone())
                });

                let conn = http.serve_connection(io, svc);

                let fut = graceful_shutdown.watch(conn);

                tokio::spawn(async move {
                    if let Err(e) = fut.await {
                        error!("Error handling request {:?}", e);
                    }
                });
            },

            _ = &mut signal => {
                info!("Shutting down gracefully");
                break;
            }
        }
    }

    tokio::select! {
        _ = graceful_shutdown.shutdown() => {
        },
        _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {
            eprintln!("Timed out wait for all connections to close");
        }
    }
}

async fn shutdown_signal() {
    // Wait for the CTRL+C signal
    let result = tokio::signal::ctrl_c().await;
    if result.is_err() {
        error!(
            "Could not instantiate CTRL+C signal: {}",
            result.err().unwrap()
        );
    }
}

async fn handle_request<T: Send + Sync + 'static>(
    request: hyper::Request<hyper::body::Incoming>,
    config: Arc<RequestPipelineConfiguration<T>>,
) -> Result<hyper::Response<Full<Bytes>>, ServerError> {
    let request_metadata: RequestMetadata = request.into();

    // First, we check if the request is authorized
    let auth_result = config.security_configuration.authorize(&request_metadata);
    if auth_result == AuthResult::Denied {
        return RequestError::with_message(ErrorType::Unauthorized, request_metadata.uri.path())
            .to_response()
            .try_into();
    }

    // Second, we try to serve the request as a static file request
    // If that fails, we go on normally to fulfill the request with our router
    // Consider adding support for logging this types of requests
    if let Some(response) = config.static_file_server.try_serve(&request_metadata).await {
        return Ok(response);
    }

    // Third, map the request_metadata into the request object that will be user visible
    let internal_request_res = Request::from_metadata_and_auth(request_metadata, auth_result).await;
    if let Err(e) = internal_request_res {
        return RequestError::with_message(ErrorType::RequestBodyUnreadable, &e.to_string())
            .to_response()
            .try_into();
    }
    let internal_request = internal_request_res.unwrap();

    // Fourth, use the router to get the REST request result
    let router_result = config.router.run(internal_request, config.context.clone());
    if let Err(e) = router_result {
        return e.to_response().try_into();
    }
    // we return the request from the run function because it will be different from the one we
    // input, as the path variables are matched inside.
    let (internal_request, response) = router_result.unwrap();

    // Lastly, execute the configured interceptor
    (config.interceptor)(&internal_request, &response);

    response.try_into()
}

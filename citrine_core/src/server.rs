use http_body_util::{BodyExt, Full};
use hyper::body::{Buf, Incoming};
use hyper::service::service_fn;
use hyper::{body::Bytes, server::conn::http1};
use hyper::{Method, StatusCode};
use hyper_util::rt::TokioIo;
use hyper_util::server::graceful::GracefulShutdown;
use log::{error, info};
use std::io::Read;
use std::net::SocketAddr;
use std::process::exit;
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::error::{ErrorType, RequestError, ServerError};
use crate::request::Request;
use crate::response::Response;
use crate::router::{InternalRouter, StaticFileServer};
use crate::security::{AuthResult, SecurityConfiguration};

pub struct RequestPipelineConfiguration<T: 'static + Send + Sync> {
    interceptor: fn(&Request, &Response),
    router: InternalRouter<T>,
    security_configuration: SecurityConfiguration,
    static_file_server: StaticFileServer,
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
    ) -> Self {
        RequestPipelineConfiguration {
            interceptor,
            router,
            security_configuration,
            static_file_server,
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
    mut request: hyper::Request<hyper::body::Incoming>,
    config: Arc<RequestPipelineConfiguration<T>>,
) -> Result<hyper::Response<Full<Bytes>>, ServerError> {
    let uri = request.uri().clone();
    let method = request.method().clone();
    let headers = request.headers().clone();

    // return default error response if request body cant be read
    // check if the map_response error should be explicitly handled
    let req_body_res = request.body_mut().collect().await;
    if let Err(e) = req_body_res {
        return Ok(map_response(
            RequestError::with_message(ErrorType::RequestBodyUnreadable, &e.to_string())
                .to_response(),
        )?);
    }
    let mut body_string = String::new();
    if let Err(e) = req_body_res
        .unwrap()
        .aggregate()
        .reader()
        .read_to_string(&mut body_string)
    {
        return Ok(map_response(
            RequestError::with_message(ErrorType::RequestBodyUnreadable, &e.to_string())
                .to_response(),
        )?);
    }

    let mut internal_request = Request::new(method, uri, body_string, headers);

    let auth_result = config.security_configuration.authorize(&internal_request);
    if auth_result == AuthResult::Denied {
        return Ok(map_response(
            RequestError::with_message(ErrorType::Unauthorized, internal_request.uri.path())
                .to_response(),
        )?);
    }
    internal_request.auth_result = auth_result;

    // Try to get a response as a static file if enabled.
    // If that fails, we go on normally to fulfill the request with our router
    if config.static_file_server.can_serve_request(&request) {
        let static_file_response = serve_static_file(&config.static_file_server, &request).await;
        if static_file_response.is_some() {
            return Ok(static_file_response.unwrap());
        }
    }

    //todo check if this clone can or should be removed
    let router_result = config.router.run(internal_request.clone());
    if let Err(e) = router_result {
        return Ok(map_response(e.to_response())?);
    }

    // we return the request from the run function because it will be different from the one in the
    // input, because the path variables are matched inside.
    // This should be improved
    let (complete_request, response) = router_result.unwrap();

    (config.interceptor)(&complete_request, &response);

    Ok(map_response(response)?)
}

async fn serve_static_file(
    static_file_server: &StaticFileServer,
    request: &hyper::Request<Incoming>,
) -> Option<hyper::Response<Full<Bytes>>> {
    let server = static_file_server.server.clone().unwrap();

    let new_uri = hyper::Uri::builder()
        .path_and_query(
            request
                .uri()
                .path()
                .strip_prefix(&static_file_server.url_base_path)
                .unwrap_or(""),
        )
        .build();
    if let Err(_) = new_uri {
        return None;
    }

    let static_file_request = hyper::Request::builder()
        .method(Method::GET)
        .uri(new_uri.unwrap())
        .body(());
    if let Err(_) = static_file_request {
        return None;
    }

    let static_file_result = server.serve(static_file_request.unwrap()).await;
    if let Err(_) = static_file_result {
        return None;
    }
    let static_file_response = static_file_result.unwrap();
    let (parts, body) = static_file_response.into_parts();

    if parts.status != StatusCode::OK {
        return None;
    }

    // Convert the body to Bytes
    let body_bytes_res = body.collect().await;
    if body_bytes_res.is_err() {
        return None;
    }
    let body_bytes = body_bytes_res.unwrap();

    // Convert the Bytes into a Full<Bytes>
    let full_body = Full::from(body_bytes.to_bytes());

    Some(hyper::Response::from_parts(parts, full_body))
}

// map our internal "user friendly" response to hyper's response
fn map_response(response: Response) -> Result<hyper::Response<Full<Bytes>>, ServerError> {
    let status_response = response.get_status();
    let mut response_builder = hyper::Response::builder().status(status_response);

    for (key, value) in response.get_headers().iter() {
        response_builder = response_builder.header(key, value);
    }

    let response_body = response
        .get_body_with_ownership()
        .unwrap_or(Full::new(Bytes::new()));

    match response_builder.body(response_body) {
        Ok(response) => Ok(response),
        Err(e) => Err(ServerError::from(e)),
    }
}


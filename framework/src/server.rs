use http_body_util::{BodyExt, Full};
use hyper::body::Buf;
use hyper::service::service_fn;
use hyper::{body::Bytes, server::conn::http1};
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
use crate::router::InternalRouter;

pub async fn start<T>(
    port: u16,
    interceptor: Option<fn(&Request, &Response)>,
    router: InternalRouter<T>,
) where
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

    let mut signal = std::pin::pin!(shutdown_signal());

    let interceptor = interceptor.unwrap_or(|_, _| {});

    let router = Arc::new(router);

    loop {
        tokio::select! {
            Ok((stream, _addr)) = listener.accept() => {
                let io = TokioIo::new(stream);

                //todo check how to avoid this double cloning
                let req_router = router.clone();
                let svc = service_fn(move |request| {
                    handle_request(interceptor, request, req_router.clone())
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
    interceptor: fn(&Request, &Response),
    request: hyper::Request<hyper::body::Incoming>,
    router: Arc<InternalRouter<T>>,
) -> Result<hyper::Response<Full<Bytes>>, ServerError> {
    let uri = request.uri().clone();
    let method = request.method().clone();

    // return default error response if request body cant be read
    // check if the map_response error should be explicitly handled
    let req_body_res = request.into_body().collect().await;
    if let Err(e) = req_body_res {
        return Ok(map_response(
            RequestError::with_message(ErrorType::RequestBodyUnreadable, &e.to_string()).to_response(),
        )?);
    }
    let mut body_string = String::new();
    if let Err(e) = req_body_res.unwrap().aggregate().reader().read_to_string(&mut body_string) {
        return Ok(map_response(
            RequestError::with_message(ErrorType::RequestBodyUnreadable, &e.to_string()).to_response(),
        )?);
    }

    let internal_request = Request::new(method, uri, body_string);

    //todo check if this clone can or should be removed
    let router_result = router.run(internal_request.clone());
    if let Err(e) = router_result {
        return Ok(map_response(e.to_response())?);
    }

    // we return the request from the run function because it will be different from the one in the
    // input, because the path variables are matched inside.  
    // This should be improved
    let (complete_request, response) = router_result.unwrap();

    interceptor(&complete_request, &response);

    Ok(map_response(response)?)
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

    match  response_builder.body(response_body) {
        Ok(response) => Ok(response),
        Err(e) => Err(ServerError::from(e))
    }
}

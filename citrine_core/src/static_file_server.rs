use http_body_util::{BodyExt, Full};
use hyper::{body::Bytes, Method, StatusCode};
use hyper_staticfile::Static;

use crate::request::RequestMetadata;

#[derive(Default, Clone)]
pub struct StaticFileServer {
    pub url_base_path: String,
    server: Option<Static>,
}

impl StaticFileServer {
    pub fn new(url_base_path: &str, server: Static) -> Self {
        StaticFileServer {
            url_base_path: url_base_path.to_string(),
            server: Some(server),
        }
    }

    pub async fn try_serve(&self, request: &RequestMetadata) -> Option<hyper::Response<Full<Bytes>>> {
        if self.server.is_none()
            || request.method != Method::GET
            || !request.uri.path().starts_with(&self.url_base_path)
        {
            return None;
        }

        let server = self.server.clone().unwrap();

        let new_uri = hyper::Uri::builder()
            .path_and_query(
                request
                    .uri
                    .path()
                    .strip_prefix(&self.url_base_path)
                    .unwrap_or(""),
            )
            .build();
        if new_uri.is_err() {
            return None;
        }

        let static_file_request = hyper::Request::builder()
            .method(Method::GET)
            .uri(new_uri.unwrap())
            .body(());
        if static_file_request.is_err() {
            return None;
        }

        let static_file_result = server.serve(static_file_request.unwrap()).await;
        if static_file_result.is_err() {
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
}

use std::path::PathBuf;

use http_body_util::{BodyExt, Full};
use hyper::{body::Bytes, Method, StatusCode};
use hyper_staticfile::Static;

use crate::request::RequestMetadata;

/// Contains a map of folders, with the key being the base_url and 
#[derive(Default, Clone)]
pub struct StaticFileServer {
    folders: Vec<ServedFolder>
}

impl StaticFileServer {
    pub fn new() -> Self {
        StaticFileServer { folders: vec![] }
    }

    pub fn serve_folder(mut self, url_base_path: &str, folder: PathBuf) -> Self {
        self.folders.push(ServedFolder::new(url_base_path, folder));
        self
    }

    pub async fn try_serve(&self, request: &RequestMetadata) -> Option<hyper::Response<Full<Bytes>>> {
        if request.method != Method::GET {
            return None;
        }

        for folder in self.folders.iter() {
            if request.uri.path().starts_with(&folder.url_base_path) {
                if let Some(response) = folder.try_serve(request).await {
                    return Some(response);
                }
            }
        }

        None
    }
}

#[derive(Clone)]
pub struct ServedFolder {
    url_base_path: String,
    server: Static
}

impl ServedFolder {
    pub fn new(url_base_path: &str, folder: PathBuf) -> Self {
        ServedFolder { url_base_path: url_base_path.to_string(), server: Static::new(folder) }
    }

    pub async fn try_serve(&self, request: &RequestMetadata) -> Option<hyper::Response<Full<Bytes>>> {
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

        let static_file_result = self.server.clone().serve(static_file_request.unwrap()).await;
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


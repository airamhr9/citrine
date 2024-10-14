use http_body_util::Full;
use hyper::{body::Bytes, HeaderMap, StatusCode};
use hyper::header::{HeaderName, HeaderValue, CONTENT_TYPE};
use serde::Serialize;

pub struct Response {
    pub status: StatusCode,
    pub body: Option<Full<Bytes>>,
    headers: HeaderMap,
}

impl Response {
    pub fn new(status: StatusCode) -> Self {
        Response {
            status,
            body: None,
            headers: HeaderMap::new(),
        }
    }

    pub fn add_header(mut self, key: HeaderName, value: &str) -> Self {
        let value = HeaderValue::from_str(value).unwrap();
        self.headers.insert(key, value);

        self
    }

    pub fn json(mut self, body: impl Serialize) -> Self {
        //todo check how to better handle serialization errors
        let body_bytes = serde_json::to_string(&body).unwrap();

        self.body = Some(Full::new(body_bytes.into()));

        self.headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        self
    }

    pub fn body(mut self, body: String) -> Self {
        //todo check how to better handle serialization errors
        self.body = Some(Full::new(body.into()));

        self
    }

    pub fn get_status(&self) -> StatusCode {
        self.status.clone()
    }

    pub fn get_body_with_ownership(self) -> Option<Full<Bytes>> {
        self.body
    }

    pub fn get_body(&self) -> &Option<Full<Bytes>> {
        &self.body
    }

    pub fn get_headers(&self) -> &HeaderMap {
        &self.headers
    }
}

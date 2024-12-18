use http_body_util::Full;
use hyper::header::{HeaderName, HeaderValue, CONTENT_TYPE};
use hyper::{body::Bytes, HeaderMap, StatusCode};
use serde::Serialize;
use tera::Context;

use crate::{templates, DefaultErrorResponseBody};

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

    pub fn static_template(template_name: &str) -> Result<Self, tera::Error> {
        let mut response = Self::new(StatusCode::OK).body(templates::render_view_with_context(
            template_name,
            &Context::new(),
        )?);

        response.headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static(mime::TEXT_HTML_UTF_8.essence_str()),
        );

        Ok(response)
    }

    pub fn template(template_name: &str, data: &impl Serialize) -> Result<Self, tera::Error> {
        let mut response =
            Self::new(StatusCode::OK).body(templates::render_view(template_name, data)?);

        response.headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static(mime::TEXT_HTML_UTF_8.essence_str()),
        );

        Ok(response)
    }

    pub fn template_from_context(
        template_name: &str,
        context: &Context,
    ) -> Result<Self, tera::Error> {
        let mut response = Self::new(StatusCode::OK)
            .body(templates::render_view_with_context(template_name, context)?);

        response.headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static(mime::TEXT_HTML_UTF_8.essence_str()),
        );

        Ok(response)
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

        self.headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_JSON.essence_str()),
        );

        self
    }

    pub fn default_error(e: &dyn std::error::Error) -> Self {
        Response::new(StatusCode::INTERNAL_SERVER_ERROR).json(DefaultErrorResponseBody::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
        ))

    }

    pub fn body(mut self, body: String) -> Self {
        //todo check how to better handle serialization errors
        self.body = Some(Full::new(body.into()));

        self
    }

    pub fn get_status(&self) -> StatusCode {
        self.status
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

impl TryFrom<Response> for hyper::Response<Full<Bytes>> {
    type Error = crate::ServerError;

    fn try_from(response: Response) -> Result<Self, Self::Error> {
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
            Err(e) => Err(crate::ServerError::from(e)),
        }
    }
}

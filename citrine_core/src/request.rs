use std::{collections::HashMap, io::Read};

use http_body_util::BodyExt;
use hyper::{
    body::{Buf, Incoming},
    HeaderMap, Method, Uri,
};
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::{
    error::{ErrorType, RequestError},
    security::AuthResult,
};

pub struct RequestMetadata {
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub original_request: hyper::Request<hyper::body::Incoming>,
}

impl From<hyper::Request<Incoming>> for RequestMetadata {
    fn from(req: hyper::Request<Incoming>) -> Self {
        RequestMetadata {
            method: req.method().clone(),
            uri: req.uri().clone(),
            headers: req.headers().clone(),
            original_request: req,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    pub uri: Uri,
    body: Option<String>,
    pub path_variables: HashMap<String, String>,
    pub headers: HeaderMap,
    pub auth_result: AuthResult,
}

impl Request {
    pub fn new(
        method: Method,
        uri: Uri,
        body: String,
        headers: HeaderMap,
        auth_result: AuthResult,
    ) -> Self {
        let body = if method != Method::GET {
            Some(body)
        } else {
            None
        };
        Request {
            method,
            uri,
            body,
            path_variables: HashMap::new(),
            headers,
            auth_result,
        }
    }

    pub async fn from_metadata_and_auth(
        mut metadata: RequestMetadata,
        auth_result: AuthResult,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let req_body = metadata.original_request.body_mut().collect().await?;

        let mut body_string = String::new();
        req_body
            .aggregate()
            .reader()
            .read_to_string(&mut body_string)?;

        Ok(Request::new(
            metadata.method,
            metadata.uri,
            body_string,
            metadata.headers,
            auth_result,
        ))
    }

    pub fn set_path_variables(&mut self, path_variables: HashMap<String, String>) {
        self.path_variables = path_variables;
    }

    pub fn get_body_raw(&self) -> &Option<String> {
        &self.body
    }

    //todo make deserialization dependant on request Content-Type. Use Accept-Type in request
    pub fn get_body<T>(&self) -> Result<T, RequestError>
    where
        T: DeserializeOwned,
    {
        if self.body.is_none() {
            return Err(RequestError::default(ErrorType::MissingBody));
        }

        let body_res: Result<T, _> = serde_json::from_str(self.body.as_ref().unwrap());

        if let Err(e) = body_res {
            return Err(RequestError::with_message(
                ErrorType::RequestBodyUnreadable,
                &e.to_string(),
            ));
        }

        Ok(body_res.unwrap())
    }

    pub fn get_body_validated<T>(&self) -> Result<T, RequestError>
    where
        T: DeserializeOwned + Validate,
    {
        if self.body.is_none() {
            return Err(RequestError::default(ErrorType::MissingBody));
        }

        let body_res: Result<T, _> = serde_json::from_str(self.body.as_ref().unwrap());

        if let Err(e) = body_res {
            return Err(RequestError::with_message(
                ErrorType::RequestBodyUnreadable,
                &e.to_string(),
            ));
        }

        let body = body_res.unwrap();

        if let Err(e) = body.validate() {
            return Err(RequestError::default(ErrorType::FailedValidation(e)));
        }

        Ok(body)
    }
}

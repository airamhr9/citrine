use std::{collections::HashMap, io::Read};

use http_body_util::BodyExt;
use hyper::{
    body::{Buf, Incoming},
    header::CONTENT_TYPE,
    HeaderMap, Method, Uri,
};
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::{
    error::{DeserializationError, ErrorType, RequestError},
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

    pub fn get_json_body<T>(&self) -> Result<T, RequestError>
    where
        T: DeserializeOwned,
    {
        self.get_body(AcceptType::One(BodyEncoding::Json))
    }

    pub fn get_json_body_validated<T>(&self) -> Result<T, RequestError>
    where
        T: DeserializeOwned + Validate,
    {
        self.get_body_validated(AcceptType::One(BodyEncoding::Json))
    }

    pub fn get_body<T>(&self, accept_type: AcceptType) -> Result<T, RequestError>
    where
        T: DeserializeOwned,
    {
        if self.body.is_none() {
            return Err(RequestError::default(ErrorType::MissingBody));
        }

        let body_res: Result<T, DeserializationError> = accept_type.parse_body(self);
        if let Err(e) = body_res {
            return Err(e.into());
        }

        Ok(body_res.unwrap())
    }

    pub fn get_body_validated<T>(&self, accept_type: AcceptType) -> Result<T, RequestError>
    where
        T: DeserializeOwned + Validate,
    {
        if self.body.is_none() {
            return Err(RequestError::default(ErrorType::MissingBody));
        }

        let body_res: Result<T, DeserializationError> = accept_type.parse_body(self);
        if let Err(e) = body_res {
            return Err(e.into());
        }

        let body = body_res.unwrap();

        if let Err(e) = body.validate() {
            return Err(RequestError::default(ErrorType::FailedValidation(e)));
        }

        Ok(body)
    }
}

pub enum AcceptType {
    One(BodyEncoding),
    Multiple(Vec<BodyEncoding>),
}

impl AcceptType {
    fn get_matching(&self, req: &Request) -> Option<BodyEncoding> {
        if let Some(content_type) = req.headers.get(CONTENT_TYPE) {
            let content_type = content_type.to_str().unwrap();
            return match self {
                AcceptType::One(encoding) => {
                    if encoding.is_valid(content_type) {
                        Some(encoding.clone())
                    } else {
                        None
                    }
                }
                AcceptType::Multiple(encodings) => {
                    for encoding in encodings {
                        if encoding.is_valid(content_type) {
                            return Some(encoding.clone());
                        }
                    }
                    None
                }
            };
        }

        None
    }

    fn parse_body<T>(&self, req: &Request) -> Result<T, DeserializationError>
    where
        T: DeserializeOwned,
    {
        let matching_encoding = self.get_matching(req);
        if matching_encoding.is_none() {
            return Err(DeserializationError::InvalidContentType);
        }

        matching_encoding.unwrap().parse(req)
    }
}

#[derive(Debug, Clone)]
pub enum BodyEncoding {
    Json,
    FormUrlEncoded,
}

impl BodyEncoding {
    fn is_valid(&self, content_type: &str) -> bool {
        content_type
            == match self {
                Self::Json => mime::APPLICATION_JSON.to_string(),
                Self::FormUrlEncoded => mime::APPLICATION_WWW_FORM_URLENCODED.to_string(),
            }
    }

    fn parse<T>(&self, req: &Request) -> Result<T, DeserializationError>
    where
        T: DeserializeOwned,
    {
        let body_str = req.body.as_ref().unwrap();
        match self {
            BodyEncoding::Json => {
                let res: Result<T, _> = serde_json::from_str(body_str);
                if let Err(e) = res {
                    Err(e.into())
                } else {
                    Ok(res.unwrap())
                }
            }
            BodyEncoding::FormUrlEncoded => {
                let res: Result<T, _> = serde_html_form::from_str(body_str);
                if let Err(e) = res {
                    Err(e.into())
                } else {
                    Ok(res.unwrap())
                }
            }
        }
    }
}

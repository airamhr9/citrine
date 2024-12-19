use std::{collections::HashMap, io::Read};

use http_body_util::BodyExt;
use hyper::{
    body::{Buf, Incoming},
    HeaderMap, Method, Uri,
};
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::{
    error::{DeserializationError, ErrorType, RequestError},
    security::security_configuration::AuthResult,
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
    path_variables: HashMap<String, String>,
    pub headers: HeaderMap,
    pub auth_result: AuthResult,
    content_type: Option<ContentType>,
}

impl Request {
    pub fn new(
        method: Method,
        uri: Uri,
        body: String,
        headers: HeaderMap,
        auth_result: AuthResult,
    ) -> Self {
        let body = if method == Method::GET || body.is_empty() {
            None
        } else {
            Some(body)
        };
        Request {
            method,
            uri,
            body,
            path_variables: HashMap::new(),
            headers,
            auth_result,
            content_type: None,
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

    pub fn get_path_variables(&self ) -> &HashMap<String, String> {
        &self.path_variables
    }

    pub fn set_content_type(&mut self, content_type: ContentType) {
        self.content_type = Some(content_type);
    }

    pub fn get_body_raw(&self) -> &Option<String> {
        &self.body
    }

    pub fn get_body<T>(&self) -> Result<T, RequestError>
    where
        T: DeserializeOwned,
    {
        if self.body.is_none() || self.content_type.is_none() {
            return Err(RequestError::default(ErrorType::MissingBody));
        }

        let body_res: Result<T, DeserializationError> = self.content_type.unwrap().parse(&self.body);
        if let Err(e) = body_res {
            return Err(e.into());
        }

        Ok(body_res.unwrap())
    }

    pub fn get_body_validated<T>(&self) -> Result<T, RequestError>
    where
        T: DeserializeOwned + Validate,
    {
        let body: T = self.get_body()?;

        if let Err(e) = body.validate() {
            return Err(RequestError::default(ErrorType::FailedValidation(e)));
        }

        Ok(body)
    }
}


#[derive(Debug, Clone, Copy)]
pub enum ContentType {
    Json,
    FormUrlEncoded,
}

impl ContentType {
    pub fn is_valid(&self, content_type: &str) -> bool {
        content_type == self.as_header_value()
    }

    pub fn as_header_value(&self) -> String {
        match self {
            Self::Json => mime::APPLICATION_JSON.to_string(),
            Self::FormUrlEncoded => mime::APPLICATION_WWW_FORM_URLENCODED.to_string(),
        }
    }

    pub fn parse<T>(&self, body: &Option<String>) -> Result<T, DeserializationError>
    where
        T: DeserializeOwned,
    {
        let body_str = body.as_ref().unwrap();
        match self {
            ContentType::Json => {
                let res: Result<T, _> = serde_json::from_str(body_str);
                if let Err(e) = res {
                    Err(e.into())
                } else {
                    Ok(res.unwrap())
                }
            }
            ContentType::FormUrlEncoded => {
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

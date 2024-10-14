use std::collections::HashMap;

use hyper::{Method, Uri};
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::error::{ErrorType, RequestError};

#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    pub uri: Uri,
    body: Option<String>,
    pub path_variables: HashMap<String, String>,
}

impl Request {
    pub fn new(method: Method, uri: Uri, body: String) -> Self {
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
        }
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
            return Err(RequestError::default(ErrorType::FailedValidation(e)))
        }


        Ok(body)
    }
}

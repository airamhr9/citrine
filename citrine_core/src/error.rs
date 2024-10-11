use std::fmt::Debug;

use chrono::{NaiveDateTime, Utc};
use derive_more::derive::{Display, Error};
use hyper::StatusCode;
use log::error;
use serde::Serialize;

use crate::response::Response;

pub type ServerError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone, Display)]
pub enum ErrorType {
    RequestBodyUnreadable,
    NotFound,
    MethodNotAllowed,
    Internal,
    MissingBody,
}

impl ErrorType {
    pub fn default_message(&self) -> &'static str {
        match self {
            ErrorType::NotFound => "Request not found",
            ErrorType::MethodNotAllowed => "Method not allowed",
            ErrorType::RequestBodyUnreadable => "Could not parse request body",
            ErrorType::Internal => "There was an error handling the request",
            ErrorType::MissingBody => "Request body is missing",
        }
    }
}

#[derive(Debug, Clone, Error, Display)]
#[display("{}{}", error_type, if cause.is_some() { format!(". Cause: {}", cause.clone().unwrap()) } else { "".to_owned() } )]
pub struct RequestError {
    error_type: ErrorType,
    cause: Option<String>,
}

impl RequestError {
    pub fn with_message(error_type: ErrorType, cause: &str) -> Self {
        RequestError {
            error_type,
            cause: Some(cause.to_string()),
        }
    }

    pub fn default(error_type: ErrorType) -> Self {
        RequestError {
            error_type,
            cause: None,
        }
    }

    pub fn to_response(self) -> Response {
        let status_code = match self.error_type {
            ErrorType::NotFound => StatusCode::NOT_FOUND,
            ErrorType::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            ErrorType::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorType::RequestBodyUnreadable | ErrorType::MissingBody => StatusCode::BAD_REQUEST,
        };
        let cause = self
            .cause
            .unwrap_or(self.error_type.default_message().to_string());

        if log::log_enabled!(log::Level::Debug) {
            error!("Response status: {} cause: {}", status_code, cause);
        }

        Response::new(status_code).json(DefaultErrorResponseBody::new(status_code, cause))
    }
}

#[derive(Serialize)]
pub struct DefaultErrorResponseBody {
    status: String,
    cause: String,
    date: NaiveDateTime,
}

impl DefaultErrorResponseBody {
    pub fn new(status: StatusCode, cause: String) -> Self {
        let status_message = if status.canonical_reason().is_some() {
            format!("{} {}", status.as_str(), status.canonical_reason().unwrap())
        } else {
            "500 Internal Server Error".to_string()
        };
        DefaultErrorResponseBody {
            status: status_message,
            cause,
            date: Utc::now().naive_local(),
        }
    }
}

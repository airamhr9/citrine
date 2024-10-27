use std::fmt::Debug;

use chrono::{NaiveDateTime, Utc};
use derive_more::derive::{Display, Error};
use hyper::StatusCode;
use log::error;
use serde::{Deserialize, Serialize};
use validator::ValidationErrors;

use crate::response::Response;

pub type ServerError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone, Display)]
pub enum ErrorType {
    RequestBodyUnreadable,
    NotFound,
    MethodNotAllowed,
    Internal,
    MissingBody,
    FailedValidation(ValidationErrors),
    Unauthorized,
    UnsupportedMediaType,
}

impl ErrorType {
    pub fn default_message(&self) -> &'static str {
        match self {
            ErrorType::NotFound => "Request not found",
            ErrorType::MethodNotAllowed => "Method not allowed",
            ErrorType::RequestBodyUnreadable => "Could not parse request body",
            ErrorType::Internal => "There was an error handling the request",
            ErrorType::MissingBody => "Request body is missing",
            ErrorType::FailedValidation(_) => "Request body failed validation",
            ErrorType::Unauthorized => "Unauthorized",
            ErrorType::UnsupportedMediaType => "Unsupported Media Type",
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
            ErrorType::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorType::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ErrorType::RequestBodyUnreadable
            | ErrorType::MissingBody
            | ErrorType::FailedValidation(_) => StatusCode::BAD_REQUEST,
        };
        let cause = self
            .cause
            .unwrap_or(self.error_type.default_message().to_string());

        if log::log_enabled!(log::Level::Debug) {
            error!("Response status: {} cause: {}", status_code, cause);
        }

        let status_message = if status_code.canonical_reason().is_some() {
            format!(
                "{} {}",
                status_code.as_str(),
                status_code.canonical_reason().unwrap()
            )
        } else {
            "500 Internal Server Error".to_string()
        };

        let validation_errors =
            if let ErrorType::FailedValidation(validation_errors) = self.error_type {
                Some(validation_errors)
            } else {
                None
            };

        let response_body = DefaultErrorResponseBody {
            status: status_message,
            cause,
            date: Utc::now().naive_local(),
            validation_errors,
        };

        Response::new(status_code).json(response_body)
    }
}

#[derive(Serialize)]
pub struct DefaultErrorResponseBody {
    status: String,
    cause: String,
    date: NaiveDateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    validation_errors: Option<ValidationErrors>,
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
            validation_errors: None,
        }
    }
}

impl From<DeserializationError> for RequestError {
    fn from(error: DeserializationError) -> Self {
        match error {
            DeserializationError::MalformedBody(cause) => {
                RequestError::with_message(ErrorType::RequestBodyUnreadable, &cause)
            }
            DeserializationError::InvalidContentType => {
                RequestError::default(ErrorType::UnsupportedMediaType)
            }
        }
    }
}

#[derive(Debug, Deserialize, Display)]
pub enum DeserializationError {
    MalformedBody(String),
    InvalidContentType,
}

impl DeserializationError {
    pub fn malformed_body(e: &dyn std::error::Error) -> Self {
        DeserializationError::MalformedBody(e.to_string())
    }
}

impl From<serde_json::Error> for DeserializationError {
    fn from(value: serde_json::Error) -> Self {
        DeserializationError::malformed_body(&value)
    }
}

impl From<serde_html_form::de::Error> for DeserializationError {
    fn from(value: serde_html_form::de::Error) -> Self {
        DeserializationError::malformed_body(&value)
    }
}

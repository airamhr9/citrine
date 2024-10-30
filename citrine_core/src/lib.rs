// exports to avoid having to add necessary libraries as dependencies on the app
pub use tokio;
pub use tera;
pub use jsonwebtoken;
pub use hyper::{body::Bytes, Method, Uri, StatusCode, header};

pub use error::{ServerError, RequestError, DefaultErrorResponseBody};
pub use router::{Router, Route, Accepts};

mod server;
mod router;
mod error;
mod views;
pub mod security;
pub mod middleware;
pub mod request;
pub mod response;
pub mod application;
pub mod static_file_server;
pub mod request_matcher;

extern crate lazy_static;

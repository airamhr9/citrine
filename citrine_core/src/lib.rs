// exports to avoid having to add necessary libraries as dependencies on the app
pub use tokio;
pub use hyper::{body::Bytes, Method, Uri, StatusCode, header};

pub use error::{ServerError, RequestError, DefaultErrorResponseBody};
pub use router::{Router, Route};

mod server;
mod router;
mod error;
pub mod request;
pub mod response;
pub mod application;
pub mod views;

#[macro_use]
extern crate lazy_static;

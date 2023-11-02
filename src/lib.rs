pub mod gh;
pub mod models;
pub mod schema;
pub mod service;

use diesel::result::Error;
use rocket::response::status::BadRequest;

pub struct RiirViewError {
    pub detail: String,
}

impl RiirViewError {
    pub fn new(detail: String) -> Self {
        RiirViewError { detail }
    }
}

impl From<Error> for RiirViewError {
    fn from(error: Error) -> Self {
        RiirViewError::new(error.to_string())
    }
}

impl Into<BadRequest<String>> for RiirViewError {
    fn into(self) -> BadRequest<String> {
        BadRequest(Some(self.detail))
    }
}

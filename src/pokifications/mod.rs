extern crate hyper;
extern crate futures;
extern crate serde_json;

use self::futures::future::Future;
use self::futures::Stream;

use self::hyper::StatusCode;
use self::hyper::server::{Request, Response, Service};

pub mod entities;

pub struct Pokifications;

impl Pokifications {
    /// Dispatchs the call, if possible
    fn map_body(chunks: Vec<u8>) -> Response {
        //convert chunks to String
        match (String::from_utf8(chunks).map_err(|e| format!("Unable to convert request body to string: {}", e)))
            //convert request to struct Request
            .and_then(|body| serde_json::from_str::<entities::Request>(&body).map_err(|e| format!("Syntax error on json request: {}", e)))
            .and_then(|ref _request|
                //TODO: dispatch request
                Ok("test")
            ) {
            Ok(out) => Response::new().with_status(StatusCode::Ok).with_body(out),
            Err(e) => Response::new().with_status(StatusCode::InternalServerError).with_body(e),
        }
    }
}

/// Hyper Service implementation
impl Service for Pokifications {
    // boilerplate hooking up hyper's server types
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    // The future representing the eventual Response your call will
    // resolve to. This can change to whatever Future you need.
    type Future = Box<Future<Item=Self::Response, Error=Self::Error>>;

    fn call(&self, req: Request) -> Self::Future {
        //concat every request's body chunk
        Box::new(req.body().concat2().map(move |chunks| Pokifications::map_body(chunks.to_vec())))
    }
}

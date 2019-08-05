#![deny(warnings)]
#![deny(missing_docs)]
#![recursion_limit="128"]

//! # pokefications
//!
//! RocketMap webhook clients
//!
//! A notifications daemon alternative to PokeAlarm

mod entities;
mod db;
mod config;
mod bot;
mod lists;

use hyper::{Body, Request, Response, Server};
use hyper::service::service_fn_ok;

use tokio::{run, spawn};
use tokio::prelude::{Future, Stream};

use log::{info, error};

/// Launch service according to config
fn main() {
    //log4rs::init_file("log4rs.yaml", Default::default()).expect("Unable to init logging");
    env_logger::init();

    //retrieve address and port, defaulting if not configured
    let addr = format!(
            "{}:{}",
            config::CONFIG.service.address.as_ref().map(|s| s.as_str()).unwrap_or_else(|| "0.0.0.0"),
            config::CONFIG.service.port.unwrap_or_else(|| 80)
        ).parse().expect("Error parsing webserver address");

    //basic service function
    let make_service = || {
        service_fn_ok(|req: Request<Body>| {
            //spawn an independent future to parse the stream
            spawn(req.into_body()
                    .concat2()
                    .map_err(|e| error!("concat error: {}", e))
                    .and_then(|chunks| String::from_utf8(chunks.to_vec()).map_err(|e| error!("encoding error: {}", e)))
                    .and_then(|s| serde_json::from_str(&s).map_err(|e| error!("deserialize error: {}\n{}", e, s)))
                    .and_then(bot::BotConfigs::submit)
                );
            //always reply empty 200 OK
            Response::new(Body::empty())
        })
    };

    info!("Starting webserver at {}", addr);//debug
    //bind and serve...
    run(Server::bind(&addr).serve(make_service).map_err(|e| {
        error!("server error: {}", e);
    }));
}

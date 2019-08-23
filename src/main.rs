#![deny(warnings)]
#![deny(missing_docs)]

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
use hyper::service::{make_service_fn, service_fn};

use futures_util::TryStreamExt;

use tokio::spawn;
use tokio::runtime::{Runtime, Builder};

use chrono::{DateTime, Local};

use log::{info, error};

async fn parse(now: DateTime<Local>, req: Request<Body>) -> Result<(), ()> {
    let chunks = req.into_body().try_concat().await.map_err(|e| error!("concat error: {}", e))?;
    let body = String::from_utf8(chunks.to_vec()).map_err(|e| error!("encoding error: {}", e))?;
    let configs = serde_json::from_str(&body).map_err(|e| error!("deserialize error: {}\n{}", e, body))?;
    bot::BotConfigs::submit(now, configs).await;
    Ok(())
}

async fn service(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let now = Local::now();

    //spawn an independent future to parse the stream
    spawn(async move {
        parse(now, req).await.ok();
    });

    //always reply empty 200 OK
    Ok(Response::new(Body::empty()))
}

/// Launch service according to config
fn main() -> Result<(), ()> {
    //log4rs::init_file("log4rs.yaml", Default::default()).expect("Unable to init logging");
    env_logger::init();

    //retrieve address and port, defaulting if not configured
    let addr = format!(
            "{}:{}",
            config::CONFIG.service.address.as_ref().map(|s| s.as_str()).unwrap_or_else(|| "0.0.0.0"),
            config::CONFIG.service.port.unwrap_or_else(|| 80)
        ).parse().map_err(|e| error!("Error parsing webserver address: {}", e))?;

    //basic service function
    let service = make_service_fn(|_| {
        async {
            Ok::<_, hyper::Error>(service_fn(service))
        }
    });

    info!("Starting webserver at {}", addr);//debug

    let runtime = match config::CONFIG.threads {
        Some(ref threads) => {
            let mut runtime_builder = Builder::new();
            runtime_builder.core_threads(threads.min)
                .blocking_threads(threads.max)
                .build()
                .map_err(|e| error!("Threadpool build error: {}", e))?
        },
        None => Runtime::new().map_err(|e| error!("Runtime build error: {}", e))?,
    };

    runtime.spawn(async move {
        // bind and serve...
        Server::bind(&addr).serve(service).await.map_err(|e| {
            error!("server error: {}", e);
        }).ok();
    });

    // wait for completion
    runtime.shutdown_on_idle();

    Ok(())
}

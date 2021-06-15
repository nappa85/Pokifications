#![deny(warnings)]
#![allow(where_clauses_object_safety)]
#![deny(missing_docs)]

//! # pokifications
//!
//! RocketMap webhook clients
//!
//! A notifications daemon alternative to PokeAlarm

mod entities;
mod db;
mod config;
mod bot;
mod lists;
mod telegram;
mod alerts;

use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};

use futures_util::TryStreamExt;

use mysql_async::prelude::Queryable;

use tokio::spawn;
// use tokio::fs::File;
// use tokio::prelude::*;

use chrono::{DateTime, Local};

use serde_json::value::Value;

use log::{info, error};

use crate::db::MYSQL;

// async fn log_webhook(bytes: &[u8]) -> Result<(), ()> {
//     let filename = format!("{}log/{}.log", config::CONFIG.images.bot, Local::now().format("%Y%m%d%H%M%S%f").to_string());
//     let mut file = File::create(&filename).await.map_err(|e| error!("logfile {} create error: {}", filename, e))?;
//     file.write_all(bytes).await.map_err(|e| error!("logfile {} write error: {}", filename, e))?;
//     Ok(())
// }

async fn parse(now: DateTime<Local>, bytes: Vec<u8>) -> Result<(), ()> {
    // let bytes2 = bytes.clone();

    // spawn(async move {
    //     log_webhook(&bytes2).await.ok();
    // });

    let body = String::from_utf8(bytes).map_err(|e| error!("encoding error: {}", e))?;
    // split the serialization in two passes, this way a single error doesn't break the entire block
    let configs: Vec<Value> = serde_json::from_str(&body).map_err(|e| error!("deserialize error: {}\n{}", e, body))?;

    let count = configs.len();
    spawn(async move {
        if let Ok(mut conn) = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e)) {
            conn.query_drop(format!("INSERT INTO bot_stats (day, events) VALUES (CURDATE(), {0}) ON DUPLICATE KEY UPDATE events = events + {0}", count)).await
                .map_err(|e| error!("MySQL update bot stats error: {}", e))
                .ok();
        }
    });

    bot::BotConfigs::submit(
        now, 
        configs.into_iter()
            .map(|v|
                // this is a bit of a waste of memory, but there is no other way around
                serde_json::from_value(v.clone())
                    .map_err(|e| error!("deserialize error: {}\n{}", e, v))
            )
            .filter(Result::is_ok)
            .map(Result::unwrap)
            .collect()
        ).await;
    Ok(())
}

async fn service(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    if config::CONFIG.service.safeword.is_none() || Some(req.uri().path().trim_matches('/')) == config::CONFIG.service.safeword.as_deref() {
        let now = Local::now();
        let bytes = req.into_body()
            .map_ok(|c| c.to_vec())
            .try_concat()
            .await
            .map_err(|e| {
                error!("concat error: {}", e);
                e
            })?;

        //spawn an independent future to parse the stream
        spawn(async move {
            parse(now, bytes).await.ok();
        });
    }

    //always reply empty 200 OK
    Ok(Response::new(Body::empty()))
}

/// Launch service according to config
#[tokio::main]
async fn main() -> Result<(), ()> {
    env_logger::init();

    //retrieve address and port, defaulting if not configured
    let addr = format!(
            "{}:{}",
            config::CONFIG.service.address.as_deref().unwrap_or("0.0.0.0"),
            config::CONFIG.service.port.unwrap_or(80)
        ).parse().map_err(|e| error!("Error parsing webserver address: {}", e))?;

    //basic service function
    let service = make_service_fn(|_| {
        async {
            Ok::<_, hyper::Error>(service_fn(service))
        }
    });

    // telegram::init();
    lists::init().await;
    alerts::init();
    if bot::BotConfigs::init().await.is_ok() {
        info!("Starting webserver at {}", addr);//debug

        // bind and serve...
        Server::bind(&addr).serve(service).await.map_err(|e| {
            error!("server error: {}", e);
        }).ok();
    }

    Ok(())
}

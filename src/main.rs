#![deny(warnings)]
#![allow(where_clauses_object_safety)]
#![deny(missing_docs)]

//! # pokifications
//!
//! RocketMap webhook clients
//!
//! A notifications daemon alternative to PokeAlarm

mod alerts;
mod bot;
mod cleanup;
mod config;
mod db;
mod lists;
mod telegram;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};

use futures_util::TryStreamExt;

use mysql_async::prelude::Queryable;

use tokio::spawn;
// use tokio::fs::File;
// use tokio::prelude::*;

use chrono::{DateTime, Utc};

use serde_json::value::Value;

use tracing::{debug, error, info};

use crate::db::MYSQL;

// async fn log_webhook(bytes: &[u8]) -> Result<(), ()> {
//     let filename = format!("{}log/{}.log", config::CONFIG.images.bot, Utc::now().with_timezone(&Rome).format("%Y%m%d%H%M%S%f").to_string());
//     let mut file = File::create(&filename).await.map_err(|e| error!("logfile {} create error: {}", filename, e))?;
//     file.write_all(bytes).await.map_err(|e| error!("logfile {} write error: {}", filename, e))?;
//     Ok(())
// }

async fn parse(now: DateTime<Utc>, bytes: Vec<u8>, platform: Platform) -> Result<(), ()> {
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
        configs.into_iter().filter_map(|v| {
            // this is a bit of a waste of memory, but there is no other way around
            debug!("Received {:?} webhook {}", platform, v);
            serde_json::from_value(v.clone()).map_err(|e| error!("deserialize error: {}\n{}", e, v)).ok()
        }),
        platform,
    )
    .await;
    Ok(())
}

#[derive(Copy, Clone, Debug)]
/// Scan platform
pub enum Platform {
    /// Unknown
    Unknown,
    /// ReadDeviceMap
    Rdm,
    /// Map-a-Droid
    Mad,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Platform::Unknown => "",
                Platform::Rdm => " su iOS",
                Platform::Mad => " su Android",
            }
        )
    }
}

fn check_safeword(req: &Request<Body>) -> Option<Platform> {
    if config::CONFIG.service.safeword.is_none()
        && config::CONFIG.service.rdm_safeword.is_none()
        && config::CONFIG.service.rdm_safeword.is_none()
    {
        None
    } else {
        let path = Some(req.uri().path().trim_matches('/'));
        if path == config::CONFIG.service.safeword.as_deref() {
            Some(Platform::Unknown)
        } else if path == config::CONFIG.service.rdm_safeword.as_deref() {
            Some(Platform::Rdm)
        } else if path == config::CONFIG.service.mad_safeword.as_deref() {
            Some(Platform::Mad)
        } else {
            None
        }
    }
}

async fn service(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    if let Some(platform) = check_safeword(&req) {
        let now = Utc::now();
        let bytes = req.into_body().map_ok(|c| c.to_vec()).try_concat().await.map_err(|e| {
            error!("concat error: {}", e);
            e
        })?;

        //spawn an independent future to parse the stream
        spawn(async move {
            parse(now, bytes, platform).await.ok();
        });
    }

    //always reply empty 200 OK
    Ok(Response::new(Body::empty()))
}

/// Launch service according to config
#[tokio::main]
async fn main() -> Result<(), ()> {
    tracing_subscriber::fmt::init();

    rocketmap_entities::gamemaster::init();

    cleanup::init();

    //retrieve address and port, defaulting if not configured
    let addr = format!(
        "{}:{}",
        config::CONFIG.service.address.as_deref().unwrap_or("0.0.0.0"),
        config::CONFIG.service.port.unwrap_or(80)
    )
    .parse()
    .map_err(|e| error!("Error parsing webserver address: {}", e))?;

    //basic service function
    let service = make_service_fn(|_| async { Ok::<_, hyper::Error>(service_fn(service)) });

    // telegram::init();
    lists::init().await;
    alerts::init();
    if bot::BotConfigs::init().await.is_ok() {
        info!("Starting webserver at {}", addr); //debug

        // bind and serve...
        Server::bind(&addr)
            .serve(service)
            .await
            .map_err(|e| {
                error!("server error: {}", e);
            })
            .ok();
    }

    Ok(())
}

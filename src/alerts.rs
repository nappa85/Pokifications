use std::time::Duration;

use future_parking_lot::rwlock::FutureReadable;

use futures_util::stream::StreamExt;
use futures_util::try_stream::TryStreamExt;

use tokio::future::FutureExt;
use tokio::timer::Interval;
use tokio::spawn;

use chrono::{Local, Timelike, TimeZone};

use hyper::{Client, Request};
use hyper_tls::HttpsConnector;

use serde_json::json;

use log::error;

use crate::config::CONFIG;
use crate::lists::CITIES;

pub fn init() {
    spawn(async {
        if let (Some(bot_token), Some(chat_id)) = (CONFIG.telegram.alert_bot_token.as_ref(), CONFIG.telegram.alert_chat.as_ref()) {
            Interval::new_interval(Duration::from_secs(300))
                .for_each(|_| async {
                    let now = Local::now();
                    let five_minutes_ago = now.timestamp() - 300;

                    let mut alerts = Vec::new();
                    for (_, city) in CITIES.iter() {
                        let lock = city.stats.future_read().await;

                        check_timestamp(&lock.last_pokemon, five_minutes_ago, &city.name, "Pokémom", &mut alerts);
                        check_timestamp(&lock.last_iv, five_minutes_ago, &city.name, "IV", &mut alerts);
                        if now.hour() >= 6 && now.hour() <= 21 {
                            check_timestamp(&lock.last_raid, five_minutes_ago, &city.name, "Raid", &mut alerts);
                        }
                        check_timestamp(&lock.last_quest, now.timestamp() - 86400, &city.name, "Quest", &mut alerts);
                        check_timestamp(&lock.last_invasion, five_minutes_ago, &city.name, "Invasioni", &mut alerts);
                    }

                    if !alerts.is_empty() {
                        send_alerts(&alerts, bot_token, chat_id).await.ok();
                    }
                }).await
        }
    });
}

fn check_timestamp(var: &Option<i64>, check: i64, city: &str, descr: &str, alerts: &mut Vec<String>) {
    if let Some(timestamp) = var {
        if timestamp < &check {
            alerts.push(format!("La zona {} non ha scansioni {} da {}", city, descr, Local.timestamp(*timestamp, 0).format("%d-%m-%Y %R").to_string()));
        }
    }
    else {
        alerts.push(format!("La zona {} non ha MAI avuto scansioni {} dall'ultimo avvio del bot", city, descr));
    }
}

async fn send_alerts(alerts: &[String], bot_token: &str, chat_id: &str) -> Result<(), ()> {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let body = json!({
        "chat_id": chat_id,
        "message": alerts.join("\n")
    });

    let req = Request::builder()
        .method("POST")
        .uri(&url)
        .body(body.to_string().into())
        .map_err(|e| error!("error building Telegram request: {}", e))?;

    let https = HttpsConnector::new().unwrap();
    let future = Client::builder().build::<_, hyper::Body>(https).request(req);
    let res = match CONFIG.telegram.timeout {
        Some(timeout) => future.timeout(Duration::from_secs(timeout)).await.map_err(|e| error!("timeout calling Telegram: {}", e))?,
        None => future.await,
    }.map_err(|e| error!("error calling Telegram: {}", e))?;

    if !res.status().is_success() {
        let debug = format!("error response from Telegram Alert: {:?}", res);

        let chunks = res.into_body().try_concat().await.map_err(|e| error!("error while reading {}: {}", debug, e))?;

        let body = String::from_utf8(chunks.to_vec()).map_err(|e| error!("error while encoding {}: {}", debug, e))?;

        error!("{}\n{}", debug, body);
    }

    Ok(())
}

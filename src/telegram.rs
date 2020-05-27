use std::sync::atomic::{AtomicUsize, Ordering};

use reqwest::{Body, Client, Method, Url, RequestBuilder, multipart::{Form, Part}};

use serde_json::{json, value::Value};

use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;

use futures_util::stream::StreamExt;

use tokio::{spawn, time::{Duration, Instant, interval_at, delay_for}};

use chrono::offset::Local;

use once_cell::sync::Lazy;

use log::{error, warn};

use crate::config::CONFIG;

pub static COUNT: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

async fn wall() {
    let mut delays = 0;
    // Telegram accepts only 30 messages per second
    while COUNT.fetch_add(1, Ordering::Relaxed) > 30 {
        delays += 1;
        let now = Local::now();
        delay_for(Duration::from_nanos(1_000_000_000_u64 - (now.timestamp_subsec_nanos() as u64))).await;
    }
    if delays > 0 {
        warn!("Too many Telegram messages, message delayed {} times", delays);
    }
}

pub enum CallResult {
    Body((u16, String)),
    Empty,
}

#[derive(Clone, Debug)]
pub enum Image {
    FileId(String),
    Bytes(Vec<u8>),
}

pub async fn call_telegram(req: RequestBuilder) -> Result<String, CallResult> {
    wall().await;

    let res = if let Some(t) = CONFIG.telegram.timeout {
            req.timeout(Duration::from_secs(t))
        }
        else {
            req
        }
        .send()
        .await
        .map_err(|e| {
            error!("error calling Telegram: {}", e);
            CallResult::Empty
        })?;

    let success = res.status().is_success();
    let status = res.status().as_u16();

    let debug = format!("response from Telegram: {:?}", res);

    let body = res.text()
        .await
        .map_err(|e| {
            error!("error while encoding {}: {}", debug, e);
            CallResult::Empty
        })?;

    if success {
        Ok(body)
    }
    else {
        error!("error {}\n{}", debug, body);
        Err(CallResult::Body((status, body)))
    }
}

pub async fn send_message(bot_token: &str, chat_id: &str, text: &str, parse_mode: Option<&str>, disable_web_page_preview: Option<bool>, disable_notification: Option<bool>, reply_to_message_id: Option<i64>, reply_markup: Option<Value>) -> Result<String, CallResult> {
    let url = Url::parse(&format!("https://api.telegram.org/bot{}/sendMessage", bot_token)).map_err(|e| {
        error!("error building Telegram URL: {}", e);
        CallResult::Empty
    })?;
    let mut body = json!({
        "chat_id": chat_id,
        "text": text
    });

    if let Some(v) = parse_mode {
        body["parse_mode"] = Value::from(v);
    }

    if let Some(v) = disable_web_page_preview {
        body["disable_web_page_preview"] = Value::from(v);
    }

    if let Some(v) = disable_notification {
        body["disable_notification"] = Value::from(v);
    }

    if let Some(v) = reply_to_message_id {
        body["reply_to_message_id"] = Value::from(v);
    }

    if let Some(v) = reply_markup {
        body["reply_markup"] = v;
    }

    let client = Client::new();
    let req = client.request(Method::POST, url)
        .header("Content-Type", "application/json")
        .json(&body);
    call_telegram(req).await
}


pub async fn send_photo(bot_token: &str, chat_id: &str, photo: Image, caption: Option<&str>, parse_mode: Option<&str>, disable_notification: Option<bool>, reply_to_message_id: Option<i64>, reply_markup: Option<Value>) -> Result<String, CallResult> {
    let url = Url::parse(&format!("https://api.telegram.org/bot{}/sendPhoto", bot_token)).map_err(|e| {
        error!("error building Telegram URL: {}", e);
        CallResult::Empty
    })?;
    let boundary: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .collect();

    let mut form = Form::new()
        .text("chat_id", chat_id.to_owned());

    if let Some(v) = caption {
        form = form.text("caption", v.to_owned());
    }
    if let Some(v) = parse_mode {
        form = form.text("parse_mode", v.to_owned());
    }
    if let Some(v) = disable_notification {
        form = form.text("disable_notification", v.to_string());
    }
    if let Some(v) = reply_to_message_id {
        form = form.text("reply_to_message_id", v.to_string());
    }
    if let Some(v) = reply_markup {
        form = form.text("reply_markup", v.to_string());
    }

    match photo {
        Image::FileId(file_id) => {
            form = form.text("photo", file_id);
        },
        Image::Bytes(bytes) => {
            form = form.part("photo", Part::stream(Body::from(bytes)).file_name("image.png").mime_str("image/png").map_err(|e| {
                error!("error writing multipart mime: {}", e);
                CallResult::Empty
            })?);
        },
    }

    let client = Client::new();
    let req = client.request(Method::POST, url)
        .header("Content-Type", &format!("multipart/form-data; boundary={}", boundary))
        .multipart(form);
    call_telegram(req).await
}

pub fn init() {
    spawn(async {
        // start next leap second
        let now = Local::now();
        interval_at(Instant::now() + Duration::from_nanos(1_000_000_000_u64 - (now.timestamp_subsec_nanos() as u64)), Duration::from_secs(1))
            .for_each(|_| async {
                COUNT.store(0, Ordering::Relaxed);
            }).await;
    });
}

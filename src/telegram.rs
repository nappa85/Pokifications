use std::time::Duration;
use std::io::Write;

use futures_util::try_stream::TryStreamExt;

use tokio::future::FutureExt;

use hyper::{Body, Client, Request};
use hyper_tls::HttpsConnector;

use serde_json::{json, value::Value};

use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;

use log::error;

use crate::config::CONFIG;

pub enum CallResult {
    Body((u16, String)),
    Empty,
}

#[derive(Clone, Debug)]
pub enum Image {
    FileId(String),
    Bytes(Vec<u8>),
}

async fn call_telegram(req: Request<Body>) -> Result<String, CallResult> {
    let https = HttpsConnector::new().unwrap();
    let future = Client::builder().build::<_, Body>(https).request(req);
    let res = match CONFIG.telegram.timeout {
        Some(timeout) => future.timeout(Duration::from_secs(timeout)).await.map_err(|e| {
            error!("timeout calling Telegram: {}", e);
            CallResult::Empty
        })?,
        None => future.await,
    }.map_err(|e| {
        error!("error calling Telegram: {}", e);
        CallResult::Empty
    })?;

    let success = res.status().is_success();
    let status = res.status().as_u16();

    let debug = format!("error response from Telegram: {:?}", res);

    let chunks = res.into_body().try_concat().await.map_err(|e| {
        error!("error while reading {}: {}", debug, e);
        CallResult::Empty
    })?;

    let body = String::from_utf8(chunks.to_vec()).map_err(|e| {
        error!("error while encoding {}: {}", debug, e);
        CallResult::Empty
    })?;

    if success {
        Ok(body)
    }
    else {
        error!("{}\n{}", debug, body);
        Err(CallResult::Body((status, body)))
    }
}

pub async fn send_message(bot_token: &str, chat_id: &str, text: &str, parse_mode: Option<&str>, disable_web_page_preview: Option<bool>, disable_notification: Option<bool>, reply_to_message_id: Option<i64>, reply_markup: Option<Value>) -> Result<String, CallResult> {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
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

    let req = Request::builder()
        .method("POST")
        .header("Content-Type", "application/json")
        .uri(&url)
        .body(body.to_string().into())
        .map_err(|e| {
            error!("error building Telegram request: {}", e);
            CallResult::Empty
        })?;

    call_telegram(req).await
}


pub async fn send_photo(bot_token: &str, chat_id: &str, mut photo: Image, caption: Option<&str>, parse_mode: Option<&str>, disable_notification: Option<bool>, reply_to_message_id: Option<i64>, reply_markup: Option<Value>) -> Result<String, CallResult> {
    let url = format!("https://api.telegram.org/bot{}/sendPhoto", bot_token);
    let boundary: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .collect();

    let mut data = Vec::new();
    write!(&mut data, "--{}\r\nContent-Disposition: form-data; name=\"chat_id\"\r\n\r\n{}\r\n", boundary, chat_id)
        .map_err(|e| {
            error!("error writing chat_id multipart: {}", e);
            CallResult::Empty
        })?;

    if let Some(v) = caption {
        write!(&mut data, "--{}\r\nContent-Disposition: form-data; name=\"caption\"\r\n\r\n{}\r\n", boundary, v)
            .map_err(|e| {
                error!("error writing caption multipart: {}", e);
                CallResult::Empty
            })?;
    }
    if let Some(v) = parse_mode {
        write!(&mut data, "--{}\r\nContent-Disposition: form-data; name=\"parse_mode\"\r\n\r\n{}\r\n", boundary, v)
            .map_err(|e| {
                error!("error writing parse_mode multipart: {}", e);
                CallResult::Empty
            })?;
    }
    if let Some(v) = disable_notification {
        write!(&mut data, "--{}\r\nContent-Disposition: form-data; name=\"disable_notification\"\r\n\r\n{}\r\n", boundary, v)
            .map_err(|e| {
                error!("error writing disable_notification multipart: {}", e);
                CallResult::Empty
            })?;
    }
    if let Some(v) = reply_to_message_id {
        write!(&mut data, "--{}\r\nContent-Disposition: form-data; name=\"reply_to_message_id\"\r\n\r\n{}\r\n", boundary, v)
            .map_err(|e| {
                error!("error writing reply_to_message_id multipart: {}", e);
                CallResult::Empty
            })?;
    }
    if let Some(v) = reply_markup {
        write!(&mut data, "--{}\r\nContent-Disposition: form-data; name=\"reply_markup\"\r\n\r\n{}\r\n", boundary, v)
            .map_err(|e| {
                error!("error writing reply_markup multipart: {}", e);
                CallResult::Empty
            })?;
    }

    match photo {
        Image::FileId(file_id) => {
            write!(&mut data, "--{}\r\nContent-Disposition: form-data; name=\"photo\"\r\n\r\n{}", boundary, file_id)
                .map_err(|e| {
                    error!("error writing photo multipart: {}", e);
                    CallResult::Empty
                })?;
        },
        Image::Bytes(ref mut bytes) => {
            write!(&mut data, "--{}\r\nContent-Disposition: form-data; name=\"photo\"; filename=\"image.png\"\r\nContent-Type: image/png\r\n\r\n", boundary)
                .map_err(|e| {
                    error!("error writing photo multipart: {}", e);
                    CallResult::Empty
                })?;

            data.append(bytes);
        },
    }

    write!(&mut data, "\r\n--{}--\r\n", boundary)
        .map_err(|e| {
            error!("error closing multipart: {}", e);
            CallResult::Empty
        })?;

    let req = Request::builder()
        .method("POST")
        .header("Content-Type", &format!("multipart/form-data; boundary={}", boundary))
        .uri(&url)
        .body(data.into())
        .map_err(|e| {
            error!("error building Telegram request: {}", e);
            CallResult::Empty
        })?;

    call_telegram(req).await
}

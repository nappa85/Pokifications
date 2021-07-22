use std::time::Duration;

use reqwest::{Body, Client, Method, Url, RequestBuilder, multipart::{Form, Part}};

use serde_json::{json, value::Value};

use rand::{thread_rng, Rng, distributions::Alphanumeric};

use log::error;

use crate::config::CONFIG;

// pub static RATE_LIMITER: Lazy<RwLock<(usize, Vec<String>)>> = Lazy::new(|| RwLock::new((0, Vec::new())));

// const TELEGRAM_MESSAGES_PER_SECOND: usize = 30;

// /// Telegram accepts only 30 messages per second global, and only 1 message per chat per second
// async fn wall(chat_id: String) {
//     let mut delays: usize = 0;
//     loop {
//         // check with read lock, to make a write lock only if necessary
//         let mut skip = {
//             let rt = RATE_LIMITER.read().await;
//             rt.0 >= TELEGRAM_MESSAGES_PER_SECOND || rt.1.contains(&chat_id)
//         };
//         if !skip {
//             let mut rt = RATE_LIMITER.write().await;
//             if rt.0 >= TELEGRAM_MESSAGES_PER_SECOND || rt.1.contains(&chat_id) {
//                 skip = true;
//             }
//             else {
//                 rt.0 += 1;
//                 rt.1.push(chat_id.clone());
//             }
//         }
//         if skip {
//             delays += 1;
//             let now = Local::now();
//             sleep(Duration::from_nanos(1_000_000_000_u64 - (now.timestamp_subsec_nanos() as u64))).await;
//         }
//         else {
//             break;
//         }
//     }
//     if delays > 0 {
//         warn!("Too many Telegram messages for user {}, message delayed {} times", chat_id, delays);
//     }
// }

pub enum CallResult {
    Body((u16, String)),
    Empty,
}

#[derive(Clone, Debug)]
pub enum Image {
    // FileId(String),
    FileUrl(String),
    Bytes(Vec<u8>),
}

pub async fn call_telegram(/*chat_id: String, */req: RequestBuilder) -> Result<String, CallResult> {
    // wall(chat_id).await;

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

pub struct SendMessage<'a> {
    bot_token: &'a str,
    chat_id: &'a str,
    text: &'a str,
    parse_mode: Option<&'a str>,
    disable_web_page_preview: Option<bool>,
    disable_notification: Option<bool>,
    reply_to_message_id: Option<i64>,
    reply_markup: Option<Value>
}

impl<'a> SendMessage<'a> {
    pub fn set_parse_mode(mut self, parse_mode: &'a str) -> Self {
        self.parse_mode = Some(parse_mode);
        self
    }
    // pub fn set_disable_web_page_preview(mut self, disable_web_page_preview: bool) -> Self {
    //     self.disable_web_page_preview = Some(disable_web_page_preview);
    //     self
    // }
    // pub fn set_disable_notification(mut self, disable_notification: bool) -> Self {
    //     self.disable_notification = Some(disable_notification);
    //     self
    // }
    // pub fn set_reply_to_message_id(mut self, reply_to_message_id: i64) -> Self {
    //     self.reply_to_message_id = Some(reply_to_message_id);
    //     self
    // }
    // pub fn set_reply_markup(mut self, reply_markup: Value) -> Self {
    //     self.reply_markup = Some(reply_markup);
    //     self
    // }
    pub async fn send(self) -> Result<String, CallResult> {
        let url = Url::parse(&format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token)).map_err(|e| {
            error!("error building Telegram URL: {}", e);
            CallResult::Empty
        })?;
        let mut body = json!({
            "chat_id": self.chat_id,
            "text": self.text
        });
    
        if let Some(v) = self.parse_mode {
            body["parse_mode"] = Value::from(v);
        }
    
        if let Some(v) = self.disable_web_page_preview {
            body["disable_web_page_preview"] = Value::from(v);
        }
    
        if let Some(v) = self.disable_notification {
            body["disable_notification"] = Value::from(v);
        }
    
        if let Some(v) = self.reply_to_message_id {
            body["reply_to_message_id"] = Value::from(v);
        }
    
        if let Some(v) = self.reply_markup {
            body["reply_markup"] = v;
        }
    
        let client = Client::new();
        let req = client.request(Method::POST, url)
            .header("Content-Type", "application/json")
            .json(&body);
        call_telegram(req).await
    }
}

pub fn send_message<'a>(bot_token: &'a str, chat_id: &'a str, text: &'a str) -> SendMessage<'a> {
    SendMessage {
        bot_token,
        chat_id,
        text,
        parse_mode: None,
        disable_web_page_preview: None,
        disable_notification: None,
        reply_to_message_id: None,
        reply_markup: None,
    }
}

pub struct SendPhoto<'a> {
    bot_token: &'a str,
    chat_id: &'a str,
    photo: Image,
    caption: Option<&'a str>,
    parse_mode: Option<&'a str>,
    disable_notification: Option<bool>,
    reply_to_message_id: Option<i64>,
    reply_markup: Option<Value>
}

impl<'a> SendPhoto<'a> {
    pub fn set_caption(mut self, caption: &'a str) -> Self {
        self.caption = Some(caption);
        self
    }
    // pub fn set_parse_mode(mut self, parse_mode: &'a str) -> Self {
    //     self.parse_mode = Some(parse_mode);
    //     self
    // }
    // pub fn set_disable_notification(mut self, disable_notification: bool) -> Self {
    //     self.disable_notification = Some(disable_notification);
    //     self
    // }
    // pub fn set_reply_to_message_id(mut self, reply_to_message_id: i64) -> Self {
    //     self.reply_to_message_id = Some(reply_to_message_id);
    //     self
    // }
    pub fn set_reply_markup(mut self, reply_markup: Value) -> Self {
        self.reply_markup = Some(reply_markup);
        self
    }
    pub async fn send(self) -> Result<String, CallResult> {
        let url = Url::parse(&format!("https://api.telegram.org/bot{}/sendPhoto", self.bot_token)).map_err(|e| {
            error!("error building Telegram URL: {}", e);
            CallResult::Empty
        })?;
        let boundary: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();
    
        let mut form = Form::new()
            .text("chat_id", self.chat_id.to_owned());
    
        if let Some(v) = self.caption {
            form = form.text("caption", v.to_owned());
        }
        if let Some(v) = self.parse_mode {
            form = form.text("parse_mode", v.to_owned());
        }
        if let Some(v) = self.disable_notification {
            form = form.text("disable_notification", v.to_string());
        }
        if let Some(v) = self.reply_to_message_id {
            form = form.text("reply_to_message_id", v.to_string());
        }
        if let Some(v) = self.reply_markup {
            form = form.text("reply_markup", v.to_string());
        }
    
        match self.photo {
            // Image::FileId(file_id) => {
            //     form = form.text("photo", file_id);
            // },
            Image::FileUrl(url) => {
                form = form.text("photo", url);
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
}

pub fn send_photo<'a>(bot_token: &'a str, chat_id: &'a str, photo: Image) -> SendPhoto<'a> {
    SendPhoto {
        bot_token,
        chat_id,
        photo,
        caption: None,
        parse_mode: None,
        disable_notification: None,
        reply_to_message_id: None,
        reply_markup: None,
    }
}

// pub fn init() {
//     spawn(async {
//         // start next leap second
//         let now = Local::now();
//         let mut interval = interval_at(Instant::now() + Duration::from_nanos(1_000_000_000_u64 - (now.timestamp_subsec_nanos() as u64)), Duration::from_secs(1));
//         loop {
//             interval.tick().await;

//             let mut rt = RATE_LIMITER.write().await;
//             rt.0 = 0;
//             rt.1.clear();
//         }
//     });
// }

use std::time::Duration;

use future_parking_lot::rwlock::FutureReadable;

use futures_util::stream::StreamExt;

use tokio::timer::Interval;
use tokio::spawn;

use chrono::{Local, Timelike, TimeZone};

use crate::config::CONFIG;
use crate::lists::CITIES;
use crate::telegram::send_message;

pub fn init() {
    spawn(async {
        if let (Some(bot_token), Some(chat_id)) = (CONFIG.telegram.alert_bot_token.as_ref(), CONFIG.telegram.alert_chat.as_ref()) {
            Interval::new_interval(Duration::from_secs(300))
                .for_each(|_| async {
                    let now = Local::now();
                    let half_an_hour_ago = now.timestamp() - 1650;

                    let mut alerts = Vec::new();
                    for (_, city) in CITIES.iter() {
                        let lock = city.stats.future_read().await;

                        check_timestamp(&lock.last_pokemon, half_an_hour_ago, &city.name, "Pokémom", &mut alerts);
                        check_timestamp(&lock.last_iv, half_an_hour_ago, &city.name, "IV", &mut alerts);
                        if now.hour() >= 6 && now.hour() <= 21 {
                            check_timestamp(&lock.last_raid, half_an_hour_ago, &city.name, "Raid", &mut alerts);
                        }
                        check_timestamp(&lock.last_quest, now.timestamp() - 86400, &city.name, "Quest", &mut alerts);
                        check_timestamp(&lock.last_invasion, half_an_hour_ago, &city.name, "Invasioni", &mut alerts);
                    }

                    if !alerts.is_empty() {
                        send_message(bot_token, chat_id, &alerts.join("\n"), None, None, None, None, None).await.ok();
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
    // else {
    //     alerts.push(format!("La zona {} non ha MAI avuto scansioni {} dall'ultimo avvio del bot", city, descr));
    // }
}

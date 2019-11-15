use std::time::Duration;

use futures_util::stream::StreamExt;

use tokio::timer::Interval;
use tokio::spawn;

use chrono::{Local/*, Timelike*/};

use crate::config::CONFIG;
use crate::lists::CITIES;
use crate::telegram::send_message;

const INTERVAL: i64 = 900;

pub fn init() {
    spawn(async {
        if let (Some(bot_token), Some(chat_id)) = (CONFIG.telegram.alert_bot_token.as_ref(), CONFIG.telegram.alert_chat.as_ref()) {
            Interval::new_interval(Duration::from_secs(INTERVAL as u64))
                .for_each(|_| async {
                    let now = Local::now();
                    let half_an_hour_ago = now.timestamp() - 1800;

                    let mut alerts = Vec::new();
                    for (_, city) in CITIES.read().await.iter() {
                        let lock = city.stats.read().await;

                        let mut city_alerts = Vec::new();
                        check_timestamp(&lock.last_pokemon, half_an_hour_ago, "Pokémom", &mut city_alerts);
                        if city.scan_iv > 0 {
                            check_timestamp(&lock.last_iv, half_an_hour_ago, "IV", &mut city_alerts);
                        }
                        // if now.hour() >= 6 && now.hour() <= 20 {
                        //     check_timestamp(&lock.last_raid, half_an_hour_ago, "Raid", &mut city_alerts);
                        //     check_timestamp(&lock.last_invasion, half_an_hour_ago, "Invasioni", &mut city_alerts);
                        // }
                        check_timestamp(&lock.last_quest, now.timestamp() - 86400, "Quest", &mut city_alerts);

                        if !city_alerts.is_empty() {
                            alerts.push(format!("@{} la zona {} non ha scansioni:\n{}", city.admins_users, city.name, city_alerts.join("\n")));
                        }
                    }

                    if !alerts.is_empty() {
                        send_message(bot_token, chat_id, &alerts.join("\n\n"), None, None, None, None, None).await.ok();
                    }
                }).await
        }
    });
}

fn check_timestamp(var: &Option<i64>, check: i64, descr: &str, alerts: &mut Vec<String>) {
    if let Some(timestamp) = var {
        let elapsed = check - timestamp;
        // alert only problems created since last time we checked
        if elapsed > INTERVAL && (elapsed % 3600) < INTERVAL {
            alerts.push(format!("* {} da {}", descr, format_time(elapsed)));
        }
    }
}

fn format_time(seconds: i64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours > 0 && minutes > 0 {
        format!("{} ore e {} minuti", hours, minutes)
    }
    else if hours > 0 {
        format!("{} ore", hours)
    }
    else if minutes > 0 {
        format!("{} minuti", minutes)
    }
    else {
        // should never happen
        format!("{} secondi", seconds)
    }
}

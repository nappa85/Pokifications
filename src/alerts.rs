use std::time::Duration;

use tokio::time::interval;
use tokio::spawn;

use chrono::{Local/*, Timelike*/};

use crate::config::CONFIG;
use crate::lists::{CITIES, CITYSTATS};
use crate::telegram::send_message;

const INTERVAL: i64 = 900;

pub fn init() {
    spawn(async {
        if let (Some(bot_token), Some(chat_id)) = (CONFIG.telegram.alert_bot_token.as_ref(), CONFIG.telegram.alert_chat.as_ref()) {
            let mut interval = interval(Duration::from_secs(INTERVAL as u64));
            loop {
                interval.tick().await;

                let now = Local::now();
                let timestamp = now.timestamp();

                let mut alerts = Vec::new();
                for (city_id, stats) in CITYSTATS.read().await.iter() {
                    let lock = CITIES.read().await;
                    let city = match lock.get(city_id) {
                        Some(c) => c,
                        None => continue,
                    };

                    // if the quest scanner is active, other scans are obviously unactive
                    if city.scadenza < timestamp || stats.last_quest > Some(timestamp - 60) {
                        continue;
                    }

                    let mut city_alerts = Vec::new();
                    check_timestamp(&stats.last_pokemon, timestamp, "Pokémon", &mut city_alerts);
                    if city.scan_iv > 0 {
                        check_timestamp(&stats.last_iv, timestamp, "IV", &mut city_alerts);
                    }
                    // if now.hour() >= 6 && now.hour() <= 20 {
                    //     check_timestamp(&stats.last_raid, timestamp, "Raid", &mut city_alerts);
                    //     check_timestamp(&stats.last_invasion, timestamp, "Invasioni", &mut city_alerts);
                    // }
                    check_timestamp(&stats.last_quest, timestamp - 86400, "Quest", &mut city_alerts);

                    if !city_alerts.is_empty() {
                        alerts.push(format!("@{} la zona {} non ha scansioni:\n{}", city.admins_users.join(", @"), city.name, city_alerts.join("\n")));
                    }
                }

                if !alerts.is_empty() {
                    send_message(bot_token, chat_id, &alerts.join("\n\n"), None, None, None, None, None).await.ok();
                }
            }
        }
    });
}

fn check_timestamp(var: &Option<i64>, check: i64, descr: &str, alerts: &mut Vec<String>) {
    if let Some(timestamp) = var {
        let elapsed = check - timestamp;

        // alert only problems created since last time we checked
        if elapsed > INTERVAL && (elapsed % 3600) <= INTERVAL {
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

#[cfg(test)]
mod tests {
    use chrono::Local;

    use super::check_timestamp;

    #[test]
    fn alert() {
        env_logger::try_init().ok();

        let now = Local::now().timestamp();

        let mut alerts = Vec::new();
        check_timestamp(&Some(now - 6300), now - 1800, "", &mut alerts);
        assert!(!alerts.is_empty());

        let mut alerts = Vec::new();
        check_timestamp(&Some(now - 900), now, "", &mut alerts);
        assert!(alerts.is_empty());

        let mut alerts = Vec::new();
        check_timestamp(&Some(now - 3000), now, "", &mut alerts);
        assert!(alerts.is_empty());

        let mut alerts = Vec::new();
        check_timestamp(&Some(now - 7200), now, "", &mut alerts);
        assert!(!alerts.is_empty());
    }
}

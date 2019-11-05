use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Instant, Duration};

use future_parking_lot::rwlock::{RwLock, FutureReadable, FutureWriteable};

use tokio::timer::delay;
use tokio::spawn;

use chrono::{Local, DateTime};

use geo::Point;

use geo_raycasting::RayCasting;

use lazy_static::lazy_static;

use log::{info, error, debug, warn};

mod config;
mod message;

use message::{Message, PokemonMessage, RaidMessage, InvasionMessage, WeatherMessage};

use crate::entities::{Request, Weather, Watch};
use crate::lists::CITIES;
use crate::config::CONFIG;
use crate::db::MYSQL;
use crate::telegram::{Image, send_message};

lazy_static! {
    static ref BOT_CONFIGS: Arc<RwLock<HashMap<String, config::BotConfig>>> = Arc::new(RwLock::new(BotConfigs::init()));
    static ref WATCHES: Arc<RwLock<Vec<Watch>>> = Arc::new(RwLock::new(BotConfigs::watches()));
}

enum LoadResult {
    Ok,
    Disabled,
    Invalid,
    Error,
}

pub struct BotConfigs;

impl BotConfigs {
    fn init() -> HashMap<String, config::BotConfig> {
        let mut res = HashMap::new();
        Self::load(&mut res, None).expect("Unable to init BotConfigs");
        res
    }

    fn watches() -> Vec<Watch> {
        Vec::new()//TODO load from DB
    }

    pub async fn reload(user_ids: Vec<String>) -> Result<(), ()> {
        delay(Instant::now() + Duration::from_secs(1)).await;
        let mut lock = BOT_CONFIGS.future_write().await;
        let res = Self::load(&mut lock, Some(user_ids.clone()))?;
        for (user_id, result) in res {
            let msg = match result {
                LoadResult::Ok => {
                    info!("Successfully reloaded configs for user {}", user_id);
                    // $msg = "\xe2\x84\xb9\xef\xb8\x8f <b>Impostazioni modificate!</b>\n";
                    // $msg .= "<code>      ───────</code>\n";
                    // $msg .= "Le modifiche sono state applicate.";
                    // if($e == 0){ $msg .= "\nRicorda di attivare la ricezione delle notifiche con: /start";}
                    // SendTelegram($USER["user_id"], $msg);
                    format!("{} <b>Impostazioni modificate!</b>\n<code>      ───────</code>\nLe modifiche sono state applicate.",
                        String::from_utf8(vec![0xe2, 0x84, 0xb9, 0xef, 0xb8, 0x8f]).map_err(|e| error!("error converting info icon: {}", e))?)
                },
                LoadResult::Invalid => {
                    warn!("User {} has invalid configs", user_id);
                    format!("{} <b>Impostazioni non valide!</b>\n<code>      ───────</code>\nControlla che i tuoi cursori siano all'interno della tua città di appartenenza.\nSe hai bisogno di spostarti temporaneamente, invia la tua nuova posizione al bot per usarla come posizione temporanea.",
                        String::from_utf8(vec![0xE2, 0x9A, 0xA0]).map_err(|e| error!("error converting warning icon: {}", e))?)
                },
                LoadResult::Disabled => {
                    warn!("User {} has been disabled", user_id);
                    format!("{} <b>Impostazioni modificate!</b>\n<code>      ───────</code>\nLe modifiche sono state applicate.\nRicorda di attivare la ricezione delle notifiche con: /start",
                        String::from_utf8(vec![0xe2, 0x84, 0xb9, 0xef, 0xb8, 0x8f]).map_err(|e| error!("error converting info icon: {}", e))?)
                },
                LoadResult::Error => {
                    error!("Error reloading configs for user {}", user_id);
                    format!("{} <b>Errore!</b>\n<code>      ───────</code>\nC'è stato un errore applicando le tue nuova impostazioni, se il problema persiste contatta il tuo amministratore di zona.",
                        String::from_utf8(vec![0xF0, 0x9F, 0x9B, 0x91]).map_err(|e| error!("error converting error icon: {}", e))?)
                },
            };
            spawn(async move {
                send_message(&CONFIG.telegram.bot_token, &user_id, &msg, Some("HTML"), None, None, None, None).await
                    .map_err(|_| ())
                    .ok();
            });
        }
        Ok(())
    }

    fn load(configs: &mut HashMap<String, config::BotConfig>, user_ids: Option<Vec<String>>) -> Result<HashMap<String, LoadResult>, ()> {
        if let Some(ref user_ids) = user_ids {
            for user_id in user_ids {
                configs.remove(user_id);
            }
        }

        let query = format!("SELECT b.enabled, b.user_id, b.config, b.beta, u.status, c.scadenza, u.city_id FROM utenti_config_bot b
            INNER JOIN utenti u ON u.user_id = b.user_id
            INNER JOIN city c ON c.id = u.city_id AND c.scadenza > UNIX_TIMESTAMP()
            WHERE {}", user_ids.and_then(|v| if v.is_empty() {
                    None
                }
                else {
                    Some(format!("b.user_id IN ({})", v.join(", ")))
                }).unwrap_or_else(|| String::from("b.enabled = 1 AND b.beta = 1 AND u.status != 0")));

        let mut conn = MYSQL.get_conn().map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
        let res = conn.query(query).map_err(|e| error!("MySQL query error: {}", e))?;

        let now: u64 = Local::now().timestamp() as u64;
        let mut results = HashMap::new();
        for r in res {
            let mut row = r.map_err(|e| error!("MySQL row error: {}", e))?;

            let user_id: String = row.take("user_id").ok_or_else(|| error!("MySQL utenti_config_bot.user_id encoding error"))?;

            let result = Self::load_user(configs,
                row.take("enabled").ok_or_else(|| error!("MySQL utenti_config_bot.enabled encoding error"))?,
                user_id.clone(),
                row.take("config").ok_or_else(|| error!("MySQL utenti_config_bot.config encoding error for user_id {}", user_id))?,
                row.take("beta").ok_or_else(|| error!("MySQL utenti_config_bot.beta encoding error for user_id {}", user_id))?,
                row.take("status").ok_or_else(|| error!("MySQL utenti.status encoding error for user_id {}", user_id))?,
                row.take("city_id").ok_or_else(|| error!("MySQL utenti.city_id encoding error for user_id {}", user_id))?,
                row.take("scadenza").ok_or_else(|| error!("MySQL city.scadenza encoding error for user_id {}", user_id))?,
                now).unwrap_or_else(|_| LoadResult::Error);
            results.insert(user_id, result);
        }

        Ok(results)
    }

    fn load_user(configs: &mut HashMap<String, config::BotConfig>, enabled: u8, user_id: String, config: String, beta: u8, status: u8, city_id: u16, scadenza: u64, now: u64) -> Result<LoadResult, ()> {
        if enabled > 0 && beta > 0 && status > 0 {
            let config: config::BotConfig = serde_json::from_str(&config).map_err(|e| error!("MySQL utenti_config_bot.config decoding error for user_id {}: {}", user_id, e))?;
            if config.validate(city_id) {
                configs.insert(user_id.clone(), config);

                spawn(async move {
                    delay(Instant::now() + Duration::from_secs(scadenza - now)).await;
                    //.map_err(|e| error!("timer error: {}", e))
                    BotConfigs::reload(vec![user_id]).await.ok();
                });

                Ok(LoadResult::Ok)
            }
            else {
                Ok(LoadResult::Invalid)
            }
        }
        else {
            Ok(LoadResult::Disabled)
        }
    }

    async fn add_watches(watch: Watch) {
        let mut lock = WATCHES.future_write().await;
        let now = Local::now().timestamp();

        // remove expired watches
        let mut remove = Vec::new();
        for (index, watch) in lock.iter().enumerate() {
            if watch.expire < now {
                remove.push(index);
            }
        }
        for index in remove.iter().rev() {
            lock.remove(*index);
        }

        if watch.expire > now {
            if !lock.contains(&watch) {
                //TODO: add to DB
                lock.push(watch);
            }
        }
    }

    async fn submit_weather(weather: Weather) {
        let lock = WATCHES.future_read().await;
        let now = Local::now().timestamp();

        for watch in lock.iter() {
            if watch.expire < now {
                continue;
            }

            if weather.polygon.within(&watch.point) {
                match unsafe { watch.reference_weather.get().as_mut() } {
                    Some(reference_weather) => {
                        if reference_weather.is_none() {
                            *reference_weather = Some(weather.clone());
                            continue;
                        }

                        if reference_weather.as_ref() != Some(&weather) {
                            let old_weather = reference_weather.take();
                            *reference_weather = Some(weather.clone());

                            let chat_id = watch.user_id.clone();
                            let message = WeatherMessage {
                                old_weather: old_weather.unwrap(),
                                new_weather: weather.clone(),
                                position: Some(watch.point.x_y()),
                                debug: None,
                            };

                            spawn(async move {
                                let map_type = {
                                    let lock = BOT_CONFIGS.future_read().await;
                                    lock.get(&chat_id).map(|c| c.more.l.clone())
                                };

                                if let Some(l) = map_type {
                                    if let Ok(file_id) = message.prepare().await {
                                        message::send_message(&message, &chat_id, file_id, l.as_str()).await.ok();
                                    }
                                }
                            });
                        }
                    },
                    None => {},
                }
            }
        }
    }

    pub async fn submit(now: DateTime<Local>, inputs: Vec<Request>) {
        for input in inputs.into_iter() {
            // non config-related requests
            match input {
                Request::Reload(user_ids) => {
                    spawn(async {
                        BotConfigs::reload(user_ids).await.ok();
                    });
                    continue;
                },
                Request::Watch(watches) => {
                    spawn(async {
                        BotConfigs::add_watches(watches).await;
                    });
                    continue;
                },
                Request::Weather(weather) => {
                    spawn(async {
                        BotConfigs::submit_weather(weather).await;
                    });
                    continue;
                },
                Request::Pokemon(_) | Request::Raid(_) | Request::Invasion(_) | Request::Quest(_) => {
                    BotConfigs::update_city_stats(&input, now.timestamp());
                },
                _ => debug!("Unmanaged webhook: {:?}", input),
            }

            let mut futures = Vec::new();
            {
                let lock = BOT_CONFIGS.future_read().await;
                lock.iter().for_each(|(chat_id, config)| {
                    if let Ok(future) = config.submit(&now, chat_id, &input) {
                        futures.push(future);
                    }
                });
            }

            if !futures.is_empty() {
                spawn(async move {
                    if let Ok(file_id) = Self::prepare(now, input).await {
                        for future in futures.into_iter() {
                            future(file_id.clone());
                        }
                    }
                });
            }
        }
    }

    async fn prepare(now: DateTime<Local>, input: Request) -> Result<Image, ()> {
        match input {
            Request::Pokemon(i) => message::prepare(PokemonMessage::get_dummy(i), now).await,
            Request::Raid(i) => message::prepare(RaidMessage::get_dummy(i), now).await,
            Request::Invasion(i) => message::prepare(InvasionMessage::get_dummy(i), now).await,
            _ => Err(()),
        }
    }

    fn update_city_stats(input: &Request, now: i64) {
        match input {
            Request::Pokemon(p) => {
                let iv = match (p.individual_attack, p.individual_defense, p.individual_stamina) {
                    (Some(_), Some(_), Some(_)) => true,
                    _ => false,
                };
                let point: Point<f64> = (p.latitude, p.longitude).into();

                spawn(async move {
                    for (_, city) in CITIES.iter() {
                        if city.coordinates.within(&point) {
                            let update = {
                                let lock = city.stats.future_read().await;
                                (!iv && lock.last_pokemon != Some(now)) || (iv && lock.last_iv != Some(now))
                            };

                            if update {
                                let mut lock = city.stats.future_write().await;
                                if iv {
                                    lock.last_iv = Some(now);
                                }
                                else {
                                    lock.last_pokemon = Some(now);
                                }
                            }

                            break;
                        }
                    }
                });
            },
            Request::Raid(r) => {
                let point: Point<f64> = (r.latitude, r.longitude).into();

                spawn(async move {
                    for (_, city) in CITIES.iter() {
                        if city.coordinates.within(&point) {
                            let update = {
                                let lock = city.stats.future_read().await;
                                lock.last_raid != Some(now)
                            };

                            if update {
                                let mut lock = city.stats.future_write().await;
                                lock.last_raid = Some(now);
                            }

                            break;
                        }
                    }
                });
            },
            Request::Invasion(i) => {
                let point: Point<f64> = (i.latitude, i.longitude).into();

                spawn(async move {
                    for (_, city) in CITIES.iter() {
                        if city.coordinates.within(&point) {
                            let update = {
                                let lock = city.stats.future_read().await;
                                lock.last_invasion != Some(now)
                            };

                            if update {
                                let mut lock = city.stats.future_write().await;
                                lock.last_invasion = Some(now);
                            }

                            break;
                        }
                    }
                });
            },
            Request::Quest(q) => {
                let point: Point<f64> = (q.latitude, q.longitude).into();

                spawn(async move {
                    for (_, city) in CITIES.iter() {
                        if city.coordinates.within(&point) {
                            let update = {
                                let lock = city.stats.future_read().await;
                                lock.last_quest != Some(now)
                            };

                            if update {
                                let mut lock = city.stats.future_write().await;
                                lock.last_quest = Some(now);
                            }

                            break;
                        }
                    }
                });
            },
            _ => {},
        }
    }
}

#[cfg(test)]
mod tests {
    use super::message::{self, Message, PokemonMessage, RaidMessage, InvasionMessage};
    use crate::entities::Weather;

    use chrono::Local;

    use geo::Point;
    use geo_raycasting::RayCasting;

    #[tokio::test]
    async fn pokemon_image_iv() {
        env_logger::try_init().ok();

        message::prepare(PokemonMessage::get_dummy(serde_json::from_str(
                r#"{"latitude":43.771209013571,"last_modified_time":1564175718,"gender":2,"weight":1.48829138278961,"individual_stamina":13,"cp":656,"individual_attack":15,"pokemon_level":30,"spawnpoint_id":"A54016D1","disappear_time_verified":true,"form":0,"individual_defense":4,"first_seen":1564174573,"pokestop_id":"e5bab95925c040ba8ba9d480665f94dc.16","encounter_id":"12854125514185017067","longitude":11.2560545151937,"costume":0,"move_1":216,"disappear_time":1564175797,"move_2":58,"weather":1,"pokemon_id":339,"height":0.409816652536392}"#
            ).unwrap()), Local::now()).await.unwrap();
    }

    #[tokio::test]
    async fn pokemon_image_no_iv() {
        env_logger::try_init().ok();

        message::prepare(PokemonMessage::get_dummy(serde_json::from_str(
                r#"{"latitude":43.771081,"last_modified_time":1564175718,"gender":2,"weight":null,"individual_stamina":null,"cp":null,"individual_attack":null,"pokemon_level":null,"spawnpoint_id":"None","disappear_time_verified":false,"form":0,"individual_defense":null,"first_seen":1564175718,"pokestop_id":"34eadde33a9a412e9cf74a6137b38d48.16","encounter_id":"1432619712785063368","longitude":11.266252,"costume":0,"move_1":null,"disappear_time":1564176918,"move_2":null,"weather":0,"pokemon_id":109,"height":null}"#
            ).unwrap()), Local::now()).await.unwrap();
    }

    #[tokio::test]
    async fn raid_image_egg() {
        env_logger::try_init().ok();

        message::prepare(RaidMessage::get_dummy(serde_json::from_str(
                r#"{"gym_name":"Cesena - Stadio Dino Manuzzi","gym_url":"http://lh3.googleusercontent.com/EDFoOOm1G0hhjtBdJmBUjcUAcfB1se0zPatRviNuHcsK2oxwIokSONXWb8CUAJYYHI0Cl32dWiVlIMXVZe_X","end":1564338495,"form":0,"is_exclusive":false,"longitude":12.262141,"cp":0,"team_id":1,"pokemon_id":0,"gym_id":"fe20dd37398341a4b83751c5c050aaec.16","move_2":0,"level":4,"move_1":0,"start":1564335795,"ex_raid_eligible":false,"spawn":1564332195,"latitude":44.139762,"gender":0}"#
            ).unwrap()), Local::now()).await.unwrap();
    }

    #[tokio::test]
    async fn raid_image_raidboss() {
        env_logger::try_init().ok();

        message::prepare(RaidMessage::get_dummy(serde_json::from_str(
                r#"{"latitude":45.468203,"team_id":2,"start":1564212138,"end":1564214838,"spawn":1564208538,"cp":2527,"ex_raid_eligible":false,"form":63,"gym_id":"03213b28587c4063b81c6d9ed39e5f54.16","gym_name":"Parrocchia di San Pietro in Sala","longitude":9.156488,"move_1":202,"move_2":16,"is_exclusive":false,"gym_url":"http://lh5.ggpht.com/5sxTUTkH0Ch1l-w-jMN8i_2-wz1XVjcPv4EAHTBJunHONWr7KFIWHjnh_RfU0lqKPL4j12sSgQJKyKNqD3-p","level":1,"pokemon_id":52,"gender":0}"#
            ).unwrap()), Local::now()).await.unwrap();
    }

    #[tokio::test]
    async fn invasion_image() {
        env_logger::try_init().ok();

        message::prepare(InvasionMessage::get_dummy(serde_json::from_str(
                r#"{"lure_expiration":0,"enabled":true,"updated":1564332327,"url":"http://lh6.ggpht.com/ga78DsEtufPUGu0H0oE2ZOeagwxe8aQ4k4-kBLEDdSfeFVshH8gHhQN1GMcw1OFd_n94NpwTkOa16zR5DqUL","pokestop_display":1,"longitude":11.236241,"lure_id":501,"last_modified":1564329955,"pokestop_id":"54e0ee4c0e7a42ca93d2e93ee720dc90.16","name":"Ancora Un'altra Madonnina.","incident_expire_timestamp":1564333601,"grunt_type":33,"latitude":44.723203}"#
            ).unwrap()), Local::now()).await.unwrap();
    }

    #[tokio::test]
    async fn meteo() {
        let weather: Weather = serde_json::from_str(r#"{"latitude":43.9035391062106,"cloud_level":3,"severity":1,"polygon":[[43.8701745107509,10.9807003113263],[43.8613668356012,11.071175792204],[43.9368382752307,11.071175792204],[43.9456468389706,10.9807003113263]],"special_effect_level":0,"longitude":11.0259450112985,"id":1381185616091283456,"rain_level":0,"fog_level":0,"wind_direction":231,"snow_level":0,"warn_weather":true,"updated":1,"gameplay_condition":4,"wind_level":0}"#).unwrap();
        let point: Point<f64> = serde_json::from_str(r#"[43.9035391062106,11.0259450112985]"#).unwrap();
        assert!(weather.polygon.within(&point));
    }
}

use std::sync::Arc;
use std::collections::HashMap;
use std::time::Duration;

use async_std::sync::{RwLock, Mutex};

use futures_util::stream::StreamExt;

use tokio::{spawn, time::interval};

use mysql_async::{from_row, prelude::Queryable, params};

use chrono::{Local, DateTime, Timelike, offset::TimeZone};

use geo::Point;

use geo_raycasting::RayCasting;

use once_cell::sync::Lazy;

use log::{info, error, debug, warn};

mod config;
mod message;

use message::{Message, WeatherMessage};

use crate::entities::{Request, Weather, Watch};
use crate::lists::{CITIES, CITYSTATS, CITYPARKS, CityStats};
use crate::config::CONFIG;
use crate::db::MYSQL;
use crate::telegram::send_message;

static BOT_CONFIGS: Lazy<Arc<RwLock<HashMap<String, config::BotConfig>>>> = Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));
static WATCHES: Lazy<Arc<Mutex<Vec<Watch>>>> = Lazy::new(|| Arc::new(Mutex::new(Vec::new())));

enum LoadResult {
    Ok,
    Disabled,
    Invalid,
    Error,
}

pub struct BotConfigs;

impl BotConfigs {
    pub async fn init() -> Result<(), ()> {
        {
            let mut res = BOT_CONFIGS.write().await;
            Self::load(&mut res, None).await?;
            spawn(async {
                interval(Duration::from_secs(60)).for_each(|_| async {
                    let user_ids = {
                        let lock = BOT_CONFIGS.read().await;
                        let now = Some(Local::now().timestamp());
                        lock.iter().filter(|(_, config)| config.scadenza < now).map(|(id, _)| id.clone()).collect::<Vec<String>>()
                    };
                    if !user_ids.is_empty() {
                        Self::reload(user_ids).await.ok();
                    }
                }).await;
            });
        }

        {
            let conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
            let res = conn.query("SELECT user_id, encounter_id, iv, latitude, longitude, expire FROM bot_weather_watches WHERE expire > UNIX_TIMESTAMP()").await.map_err(|e| error!("MySQL query error: {}", e))?;
            let mut lock = WATCHES.lock().await;
            res.for_each_and_drop(|row| {
                let (user_id, encounter_id, iv, latitude, longitude, expire) = from_row::<(String, String, Option<u8>, f64, f64, i64)>(row);
                lock.push(Watch {
                    user_id,
                    encounter_id,
                    iv,
                    point: (latitude, longitude).into(),
                    expire,
                    reference_weather: None,
                });
            }).await.map_err(|e| error!("MySQL collect error: {}", e))?;
        }
        
        Ok(())
    }

    pub async fn reload(user_ids: Vec<String>) -> Result<(), ()> {
        let mut lock = BOT_CONFIGS.write().await;
        let res = Self::load(&mut lock, Some(user_ids.clone())).await?;
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

    async fn load(configs: &mut HashMap<String, config::BotConfig>, user_ids: Option<Vec<String>>) -> Result<HashMap<String, LoadResult>, ()> {
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

        let conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
        let res = conn.query(query).await.map_err(|e| error!("MySQL query error: {}", e))?;

        let mut results = HashMap::new();
        let (_, temp) = res.map_and_drop(from_row::<(u8, String, String, u8, u8, i64, u16)>).await.map_err(|e| error!("MySQL collect error: {}", e))?;
        for (enabled, user_id, config, beta, status, scadenza, city_id) in temp {
            let result = Self::load_user(configs, enabled, user_id.clone(), config, beta, status, city_id, scadenza).await.unwrap_or_else(|_| LoadResult::Error);
            results.insert(user_id, result);
        }

        Ok(results)
    }

    async fn load_user(configs: &mut HashMap<String, config::BotConfig>, enabled: u8, user_id: String, config: String, beta: u8, status: u8, city_id: u16, scadenza: i64) -> Result<LoadResult, ()> {
        if enabled > 0 && beta > 0 && status > 0 {
            let mut config: config::BotConfig = serde_json::from_str(&config).map_err(|e| error!("MySQL utenti_config_bot.config decoding error for user_id {}: {}", user_id, e))?;
            if config.validate(&user_id, city_id).await {
                config.scadenza = Some(scadenza);
                configs.insert(user_id, config);

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

    async fn add_watches(watch: Watch) -> Result<(), ()> {
        let mut lock = WATCHES.lock().await;
        let now = Local::now().timestamp();

        // remove expired watches
        let mut remove = Vec::new();
        for (index, watch) in lock.iter().enumerate() {
            if watch.expire < now {
                remove.push(index);
            }
        }
        for index in remove.into_iter().rev() {
            lock.remove(index);
        }

        if watch.expire > now && Local::now().hour() != Local.timestamp(watch.expire, 0).hour() {
            if !lock.contains(&watch) {
                let conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
                conn.drop_query("DELETE FROM bot_weather_watches WHERE expire < UNIX_TIMESTAMP()").await.map_err(|e| error!("MySQL delete error: {}", e))?
                    .drop_exec(
                        "INSERT INTO bot_weather_watches (user_id, encounter_id, iv, latitude, longitude, expire) VALUES (:user_id, :encounter_id, :iv, :latitude, :longitude, :expire)",
                        params! {
                            "user_id" => watch.user_id.clone(),
                            "encounter_id" => watch.encounter_id.clone(),
                            "iv" => watch.iv,
                            "latitude" => watch.point.x(),
                            "longitude" => watch.point.y(),
                            "expire" => watch.expire,
                        }
                    ).await.map_err(|e| error!("MySQL insert error: {}", e))?;

                lock.push(watch);
            }
        }

        Ok(())
    }

    async fn submit_weather(weather: Weather) {
        let mut lock = WATCHES.lock().await;
        let now = Local::now();
        let timestamp = now.timestamp();
        let hour = now.hour();

        let mut remove = Vec::new();
        let mut fire = Vec::new();
        for (index, watch) in lock.iter_mut().enumerate() {
            if watch.expire < timestamp {
                remove.push(index);
                continue;
            }

            if weather.polygon.within(&watch.point) {
                if watch.reference_weather.is_none() {
                    if hour != Local.timestamp(watch.expire, 0).hour() {
                        watch.reference_weather = Some(weather.clone());
                    }
                    else {
                        remove.push(index);
                    }
                    continue;
                }

                if hour == Local.timestamp(watch.expire, 0).hour() {
                    fire.push(index);
                    remove.push(index);
                }
            }
        }

        for index in fire.into_iter() {
            let message = WeatherMessage {
                watch: lock[index].clone(),
                actual_weather: weather.clone(),
                debug: None,
            };

            spawn(async move {
                let lock = BOT_CONFIGS.read().await;
                if let Some(l) = lock.get(&message.watch.user_id).map(|c| c.more.l.clone()) {
                    if let Ok(file_id) = message.prepare(Local::now()).await {
                        message.send(&message.watch.user_id, file_id, l.as_str()).await
                            .map_err(|_| error!("Error sending weather notification"))
                            .ok();
                    }
                }
            });
        }

        for index in remove.into_iter().rev() {
            lock.remove(index);
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
                        BotConfigs::add_watches(watches).await.ok();
                    });
                    continue;
                },
                Request::Weather(weather) => {
                    spawn(async {
                        BotConfigs::submit_weather(weather).await;
                    });
                    continue;
                },
                Request::Pokemon(ref p) => {
                    match (p.individual_attack, p.individual_defense, p.individual_stamina) {
                        (Some(_), Some(_), Some(_)) => {},
                        _ => {
                            let point = Point::new(p.latitude, p.longitude);
                            let pokemon_id = p.pokemon_id;
                            spawn(async move {
                                let city = {
                                    let lock = CITIES.read().await;
                                    lock.iter().find_map(|(id, city)| if city.coordinates.within(&point) { Some(*id) } else { None })
                                };
                                if let Some(city_id) = city {
                                    let lock = CITYPARKS.read().await;
                                    if let Some(parks) = lock.get(&city_id) {
                                        for park in parks {
                                            if park.coordinates.within(&point) {
                                                match MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e)) {
                                                    Ok(conn) => {
                                                        conn.drop_exec("INSERT INTO city_park_stats (park_id, pokemon_id, spawns) VALUES (:park_id, :pokemon_id, 1) ON DUPLICATE KEY UPDATE spawns = spawns + 1", params! {
                                                                "park_id" => park.id,
                                                                "pokemon_id" => pokemon_id,
                                                            }).await
                                                            .map_err(|e| error!("MySQL query error: {}", e)).ok();
                                                    },
                                                    Err(_) => {},
                                                }
                                            }
                                        }
                                    }
                                }
                            });
                        },
                    }

                    BotConfigs::update_city_stats(&input, now.timestamp());
                },
                Request::Raid(_) | Request::Invasion(_) | Request::Quest(_) => {
                    BotConfigs::update_city_stats(&input, now.timestamp());
                },
                _ => debug!("Unmanaged webhook: {:?}", input),
            }

            let mut messages = Vec::new();
            {
                let lock = BOT_CONFIGS.read().await;
                lock.iter().for_each(|(chat_id, config)| {
                    if let Ok(message) = config.submit(&now, &input) {
                        messages.push((chat_id.clone(), message, config.more.l.clone()));
                    }
                });
            }

            if !messages.is_empty() {
                spawn(async move {
                    if let Ok(file_id) = messages[0].1.prepare(now).await {
                        for (chat_id, message, map_type) in messages.into_iter() {
                            let file_id = file_id.clone();
                            spawn(async move {
                                message.send(&chat_id, file_id, &map_type).await.ok();
                            });
                        }
                    }
                });
            }
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
                    for (id, city) in CITIES.read().await.iter() {
                        if city.coordinates.within(&point) {
                            let update = {
                                let lock = CITYSTATS.read().await;
                                lock.get(id).map(|lock| (!iv && lock.last_pokemon != Some(now)) || (iv && lock.last_iv != Some(now)))
                            };

                            if update.is_none() || update == Some(true) {
                                let mut lock = CITYSTATS.write().await;
                                let entry = lock.entry(*id).or_insert_with(|| CityStats::default());
                                if iv {
                                    entry.last_iv = Some(now);
                                }
                                else {
                                    entry.last_pokemon = Some(now);
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
                    for (id, city) in CITIES.read().await.iter() {
                        if city.coordinates.within(&point) {
                            let update = {
                                let lock = CITYSTATS.read().await;
                                lock.get(id).map(|lock| lock.last_raid != Some(now))
                            };

                            if update.is_none() || update == Some(true) {
                                let mut lock = CITYSTATS.write().await;
                                let entry = lock.entry(*id).or_insert_with(|| CityStats::default());
                                entry.last_raid = Some(now);
                            }

                            break;
                        }
                    }
                });
            },
            Request::Invasion(i) => {
                let point: Point<f64> = (i.latitude, i.longitude).into();

                spawn(async move {
                    for (id, city) in CITIES.read().await.iter() {
                        if city.coordinates.within(&point) {
                            let update = {
                                let lock = CITYSTATS.read().await;
                                lock.get(id).map(|lock| lock.last_invasion != Some(now))
                            };

                            if update.is_none() || update == Some(true) {
                                let mut lock = CITYSTATS.write().await;
                                let entry = lock.entry(*id).or_insert_with(|| CityStats::default());
                                entry.last_invasion = Some(now);
                            }

                            break;
                        }
                    }
                });
            },
            Request::Quest(q) => {
                let point: Point<f64> = (q.latitude, q.longitude).into();

                spawn(async move {
                    for (id, city) in CITIES.read().await.iter() {
                        if city.coordinates.within(&point) {
                            let update = {
                                let lock = CITYSTATS.read().await;
                                lock.get(id).map(|lock| lock.last_quest != Some(now))
                            };

                            if update.is_none() || update == Some(true) {
                                let mut lock = CITYSTATS.write().await;
                                let entry = lock.entry(*id).or_insert_with(|| CityStats::default());
                                entry.last_quest = Some(now);
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
    use super::message::{Message, PokemonMessage, RaidMessage, InvasionMessage};

    // use super::config::BotConfig;

    // use crate::entities::{Request, Pokemon, Gender};

    use chrono::Local;

    #[tokio::test]
    async fn pokemon_image_iv() {
        env_logger::try_init().ok();

        let message = PokemonMessage {
            pokemon: serde_json::from_str(
                    r#"{"latitude":43.771209013571,"last_modified_time":1564175718,"gender":2,"weight":1.48829138278961,"individual_stamina":13,"cp":656,"individual_attack":15,"pokemon_level":30,"spawnpoint_id":"A54016D1","disappear_time_verified":true,"form":0,"individual_defense":4,"first_seen":1564174573,"pokestop_id":"e5bab95925c040ba8ba9d480665f94dc.16","encounter_id":"12854125514185017067","longitude":11.2560545151937,"costume":0,"move_1":216,"disappear_time":1564175797,"move_2":58,"weather":1,"pokemon_id":339,"height":0.409816652536392}"#
                ).unwrap(),
            iv: Some(100_f32),
            distance: 0_f64,
            direction: String::new(),
            debug: None,
        };
        message.prepare(Local::now()).await.unwrap();
    }

    #[tokio::test]
    async fn pokemon_image_no_iv() {
        env_logger::try_init().ok();

        let message = PokemonMessage {
            pokemon: serde_json::from_str(
                    r#"{"latitude":43.771081,"last_modified_time":1564175718,"gender":2,"weight":null,"individual_stamina":null,"cp":null,"individual_attack":null,"pokemon_level":null,"spawnpoint_id":"None","disappear_time_verified":false,"form":0,"individual_defense":null,"first_seen":1564175718,"pokestop_id":"34eadde33a9a412e9cf74a6137b38d48.16","encounter_id":"1432619712785063368","longitude":11.266252,"costume":0,"move_1":null,"disappear_time":1564176918,"move_2":null,"weather":0,"pokemon_id":109,"height":null}"#
                ).unwrap(),
            iv: Some(100_f32),
            distance: 0_f64,
            direction: String::new(),
            debug: None,
        };
        message.prepare(Local::now()).await.unwrap();
    }

    #[tokio::test]
    async fn raid_image_egg() {
        env_logger::try_init().ok();

        let message = RaidMessage {
            raid: serde_json::from_str(
                    r#"{"gym_name":"Cesena - Stadio Dino Manuzzi","gym_url":"http://lh3.googleusercontent.com/EDFoOOm1G0hhjtBdJmBUjcUAcfB1se0zPatRviNuHcsK2oxwIokSONXWb8CUAJYYHI0Cl32dWiVlIMXVZe_X","end":1564338495,"form":0,"is_exclusive":false,"longitude":12.262141,"cp":0,"team_id":1,"pokemon_id":0,"gym_id":"fe20dd37398341a4b83751c5c050aaec.16","move_2":0,"level":4,"move_1":0,"start":1564335795,"ex_raid_eligible":false,"spawn":1564332195,"latitude":44.139762,"gender":0}"#
                ).unwrap(),
            distance: 0_f64,
            debug: None,
        };
        message.prepare(Local::now()).await.unwrap();
    }

    #[tokio::test]
    async fn raid_image_raidboss() {
        env_logger::try_init().ok();

        let message = RaidMessage {
            raid: serde_json::from_str(
                    r#"{"latitude":45.468203,"team_id":2,"start":1564212138,"end":1564214838,"spawn":1564208538,"cp":2527,"ex_raid_eligible":false,"form":63,"gym_id":"03213b28587c4063b81c6d9ed39e5f54.16","gym_name":"Parrocchia di San Pietro in Sala","longitude":9.156488,"move_1":202,"move_2":16,"is_exclusive":false,"gym_url":"http://lh5.ggpht.com/5sxTUTkH0Ch1l-w-jMN8i_2-wz1XVjcPv4EAHTBJunHONWr7KFIWHjnh_RfU0lqKPL4j12sSgQJKyKNqD3-p","level":1,"pokemon_id":52,"gender":0}"#
                ).unwrap(),
            distance: 0_f64,
            debug: None,
        };
        message.prepare(Local::now()).await.unwrap();
    }

    #[tokio::test]
    async fn invasion_image() {
        env_logger::try_init().ok();

        let message = InvasionMessage {
            invasion: serde_json::from_str(
                    r#"{"lure_expiration":0,"enabled":true,"updated":1564332327,"url":"http://lh6.ggpht.com/ga78DsEtufPUGu0H0oE2ZOeagwxe8aQ4k4-kBLEDdSfeFVshH8gHhQN1GMcw1OFd_n94NpwTkOa16zR5DqUL","pokestop_display":1,"longitude":11.236241,"lure_id":501,"last_modified":1564329955,"pokestop_id":"54e0ee4c0e7a42ca93d2e93ee720dc90.16","name":"Ancora Un'altra Madonnina.","incident_expire_timestamp":1564333601,"grunt_type":33,"latitude":44.723203}"#
                ).unwrap(),
            debug: None,
        };
        message.prepare(Local::now()).await.unwrap();
    }

    // #[tokio::test]
    // async fn lasa() {
    //     let config: BotConfig = serde_json::from_str(
    //         r#"{"locs":{"h":["43.898817","12.454947"],"p":["43.975864","12.510105","15"],"r":["43.941428","12.476234","8"],"i":["43.894017","12.454884","0"],"t_p":["43.983675","12.511092","1573586762"],"t_r":["0","0","0"],"t_i":["","",""]},"raid":{"u":0,"s":1,"x":0,"l":[1,2,3,4,5],"p":[26]},"pkmn":{"l":{"1":[1,1,100,0,0,0,0,1],"2":[1,1,100,0,0,0,0,1],"3":[1,1,100,0,0,0,0,1],"4":[1,1,100,0,0,0,0,1],"5":[1,1,100,0,0,0,0,1],"6":[1,1,100,0,0,0,0,1],"7":[1,1,100,0,0,0,0,1],"8":[1,1,100,0,0,0,0,1],"9":[1,1,100,0,0,0,0,1],"10":[1,2,100,0,0,0,0,1],"11":[1,1,100,0,0,0,0,1],"12":[1,1,100,0,0,0,0,1],"13":[1,2,100,0,0,0,0,1],"14":[1,1,100,0,0,0,0,1],"15":[1,1,100,0,0,0,0,1],"16":[1,2,100,0,0,0,0,1],"17":[1,1,100,0,0,0,0,1],"18":[1,1,100,0,0,0,0,1],"19":[1,2,100,0,0,0,0,1],"20":[1,1,100,0,0,0,0,1],"21":[1,2,100,0,0,0,0,1],"22":[1,1,100,0,0,0,0,1],"23":[1,2,100,0,0,0,0,1],"24":[1,1,100,0,0,0,0,1],"25":[1,2,100,0,0,0,0,1],"26":[1,1,100,0,0,0,0,1],"27":[1,1,100,0,0,0,0,1],"28":[1,1,100,0,0,0,0,1],"29":[1,2,100,0,0,0,0,1],"30":[1,1,100,0,0,0,0,1],"31":[1,1,100,0,0,0,0,1],"32":[1,2,100,0,0,0,0,1],"33":[1,1,100,0,0,0,0,1],"34":[1,1,100,0,0,0,0,1],"35":[1,1,100,0,0,0,0,1],"36":[1,1,100,0,0,0,0,1],"37":[1,2,100,0,0,0,0,1],"38":[1,1,100,0,0,0,0,1],"39":[1,1,100,0,0,0,0,1],"40":[1,1,100,0,0,0,0,1],"41":[1,2,100,0,0,0,0,1],"42":[1,1,100,0,0,0,0,1],"43":[1,1,100,0,0,0,0,1],"44":[1,1,100,0,0,0,0,1],"45":[1,1,100,0,0,0,0,1],"46":[1,2,100,0,0,0,0,1],"47":[1,1,100,0,0,0,0,1],"48":[1,2,100,0,0,0,0,1],"49":[1,1,100,0,0,0,0,1],"50":[1,1,100,0,0,0,0,1],"51":[1,1,100,0,0,0,0,1],"52":[1,1,100,0,0,0,0,1],"53":[1,1,100,0,0,0,0,1],"54":[1,1,100,0,0,0,0,1],"55":[1,1,100,0,0,0,0,1],"56":[1,1,100,0,0,0,0,1],"57":[1,1,100,0,0,0,0,1],"58":[1,1,100,0,0,0,0,1],"59":[1,1,100,0,0,0,0,1],"60":[1,1,100,0,0,0,0,1],"61":[1,1,100,0,0,0,0,1],"62":[1,1,100,0,0,0,0,1],"63":[1,1,100,0,0,0,0,1],"64":[1,1,100,0,0,0,0,1],"65":[1,1,100,0,0,0,0,1],"66":[1,1,100,0,0,0,0,1],"67":[1,1,100,0,0,0,0,1],"68":[1,1,100,0,0,0,0,1],"69":[1,1,100,0,0,0,0,1],"70":[1,1,100,0,0,0,0,1],"71":[1,1,100,0,0,0,0,1],"72":[1,1,100,0,0,0,0,1],"73":[1,1,100,0,0,0,0,1],"74":[1,2,100,0,0,0,0,1],"75":[1,1,100,0,0,0,0,1],"76":[1,1,100,0,0,0,0,1],"77":[1,1,100,0,0,0,0,1],"78":[1,1,100,0,0,0,0,1],"79":[1,1,100,0,0,0,0,1],"80":[1,1,100,0,0,0,0,1],"81":[1,1,100,0,0,0,0,1],"82":[1,1,100,0,0,0,0,1],"83":[1,1,100,0,0,0,0,1],"84":[1,1,100,0,0,0,0,1],"85":[1,1,100,0,0,0,0,1],"86":[1,1,100,0,0,0,0,1],"87":[1,1,100,0,0,0,0,1],"88":[1,1,100,0,0,0,0,1],"89":[1,1,100,0,0,0,0,1],"90":[1,2,100,0,0,0,0,1],"91":[1,1,100,0,0,0,0,1],"92":[1,1,100,0,0,0,0,1],"93":[1,1,100,0,0,0,0,1],"94":[1,1,100,0,0,0,0,1],"95":[1,1,100,0,0,0,0,1],"96":[1,1,100,0,0,0,0,1],"97":[1,1,100,0,0,0,0,1],"98":[1,1,100,0,0,0,0,1],"99":[1,1,100,0,0,0,0,1],"100":[1,1,100,0,0,0,0,1],"101":[1,1,100,0,0,0,0,1],"102":[1,1,100,0,0,0,0,1],"103":[1,1,100,0,0,0,0,1],"104":[1,1,100,0,0,0,0,1],"105":[1,1,100,0,0,0,0,1],"106":[1,1,100,0,0,0,0,1],"107":[1,1,100,0,0,0,0,1],"108":[1,1,100,0,0,0,0,1],"109":[1,1,100,0,0,0,0,1],"110":[1,1,100,0,0,0,0,1],"111":[1,1,100,0,0,0,0,1],"112":[1,1,100,0,0,0,0,1],"113":[1,1,100,0,0,0,0,1],"114":[1,1,100,0,0,0,0,1],"115":[1,1,100,0,0,0,0,1],"116":[1,1,100,0,0,0,0,1],"117":[1,1,100,0,0,0,0,1],"118":[1,1,100,0,0,0,0,1],"119":[1,1,100,0,0,0,0,1],"120":[1,1,100,0,0,0,0,1],"121":[1,1,100,0,0,0,0,1],"122":[1,1,100,0,0,0,0,1],"123":[1,1,100,0,0,0,0,1],"124":[1,1,100,0,0,0,0,1],"125":[1,1,100,0,0,0,0,1],"126":[1,1,100,0,0,0,0,1],"127":[1,1,100,0,0,0,0,1],"128":[1,1,100,0,0,0,0,1],"129":[1,2,100,0,0,0,0,1],"130":[1,1,100,0,0,0,0,1],"131":[1,1,100,0,0,0,0,1],"132":[1,1,100,0,0,0,0,1],"133":[1,2,100,0,0,0,0,1],"134":[1,1,100,0,0,0,0,1],"135":[1,1,100,0,0,0,0,1],"136":[1,1,100,0,0,0,0,1],"137":[1,1,100,0,0,0,0,1],"138":[1,1,100,0,0,0,0,1],"139":[1,1,100,0,0,0,0,1],"140":[1,1,100,0,0,0,0,1],"141":[1,1,100,0,0,0,0,1],"142":[1,1,100,0,0,0,0,1],"143":[1,1,100,0,0,0,0,1],"147":[1,1,100,0,0,0,0,1],"148":[1,1,100,0,0,0,0,1],"149":[1,1,100,0,0,0,0,1],"152":[1,2,100,0,0,0,0,1],"153":[1,1,100,0,0,0,0,1],"154":[1,1,100,0,0,0,0,1],"155":[1,2,100,0,0,0,0,1],"156":[1,1,100,0,0,0,0,1],"157":[1,1,100,0,0,0,0,1],"158":[1,2,100,0,0,0,0,1],"159":[1,1,100,0,0,0,0,1],"160":[1,1,100,0,0,0,0,1],"161":[1,2,100,0,0,0,0,1],"162":[1,1,100,0,0,0,0,1],"163":[1,2,100,0,0,0,0,1],"164":[1,1,100,0,0,0,0,1],"165":[1,2,100,0,0,0,0,1],"166":[1,1,100,0,0,0,0,1],"167":[1,2,100,0,0,0,0,1],"168":[1,1,100,0,0,0,0,1],"169":[1,1,100,0,0,0,0,1],"170":[1,1,100,0,0,0,0,1],"171":[1,1,100,0,0,0,0,1],"176":[1,1,1,0,0,0,0,1],"177":[1,2,100,0,0,0,0,1],"178":[1,1,100,0,0,0,0,1],"179":[1,1,100,0,0,0,0,1],"180":[1,1,100,0,0,0,0,1],"181":[1,1,100,0,0,0,0,1],"183":[1,1,100,0,0,0,0,1],"184":[1,1,100,0,0,0,0,1],"185":[1,1,100,0,0,0,0,1],"187":[1,2,100,0,0,0,0,1],"188":[1,1,100,0,0,0,0,1],"189":[1,1,100,0,0,0,0,1],"190":[1,2,100,0,0,0,0,1],"191":[1,1,100,0,0,0,0,1],"193":[1,1,100,0,0,0,0,1],"194":[1,2,100,0,0,0,0,1],"195":[1,1,100,0,0,0,0,1],"198":[1,2,100,0,0,0,0,1],"200":[1,1,100,0,0,0,0,1],"201":[1,1,1,0,0,0,0,1],"202":[1,1,100,0,0,0,0,1],"203":[1,1,100,0,0,0,0,1],"204":[1,1,100,0,0,0,0,1],"205":[1,1,100,0,0,0,0,1],"206":[1,1,100,0,0,0,0,1],"207":[1,1,100,0,0,0,0,1],"209":[1,1,100,0,0,0,0,1],"210":[1,1,100,0,0,0,0,1],"211":[1,1,100,0,0,0,0,1],"213":[1,1,100,0,0,0,0,1],"214":[1,1,100,0,0,0,0,1],"215":[1,1,100,0,0,0,0,1],"216":[1,2,100,0,0,0,0,1],"217":[1,1,100,0,0,0,0,1],"218":[1,1,100,0,0,0,0,1],"219":[1,1,100,0,0,0,0,1],"220":[1,1,100,0,0,0,0,1],"221":[1,1,100,0,0,0,0,1],"222":[1,1,100,0,0,0,0,1],"223":[1,1,100,0,0,0,0,1],"224":[1,1,100,0,0,0,0,1],"225":[1,2,100,0,0,0,0,1],"226":[1,1,100,0,0,0,0,1],"227":[1,1,100,0,0,0,0,1],"228":[1,1,100,0,0,0,0,1],"229":[1,1,100,0,0,0,0,1],"231":[1,1,100,0,0,0,0,1],"232":[1,1,100,0,0,0,0,1],"233":[1,1,100,0,0,0,0,1],"234":[1,1,100,0,0,0,0,1],"237":[1,1,100,0,0,0,0,1],"241":[1,1,100,0,0,0,0,1],"242":[1,1,100,0,0,0,0,1],"246":[1,1,100,0,0,0,0,1],"247":[1,1,100,0,0,0,0,1],"248":[1,1,100,0,0,0,0,1],"252":[1,2,100,0,0,0,0,1],"253":[1,1,100,0,0,0,0,1],"254":[1,1,100,0,0,0,0,1],"255":[1,2,100,0,0,0,0,1],"256":[1,1,100,0,0,0,0,1],"257":[1,1,100,0,0,0,0,1],"258":[1,2,100,0,0,0,0,1],"259":[1,1,100,0,0,0,0,1],"260":[1,1,100,0,0,0,0,1],"261":[1,2,100,0,0,0,0,1],"262":[1,1,100,0,0,0,0,1],"263":[1,2,100,0,0,0,0,1],"264":[1,1,100,0,0,0,0,1],"265":[1,2,100,0,0,0,0,1],"266":[1,1,100,0,0,0,0,1],"267":[1,1,100,0,0,0,0,1],"268":[1,1,100,0,0,0,0,1],"269":[1,1,100,0,0,0,0,1],"270":[1,1,100,0,0,0,0,1],"271":[1,1,100,0,0,0,0,1],"272":[1,1,100,0,0,0,0,1],"273":[1,2,100,0,0,0,0,1],"274":[1,1,100,0,0,0,0,1],"275":[1,1,100,0,0,0,0,1],"276":[1,1,100,0,0,0,0,1],"277":[1,1,100,0,0,0,0,1],"278":[1,1,100,0,0,0,0,1],"279":[1,1,100,0,0,0,0,1],"280":[1,1,100,0,0,0,0,1],"281":[1,1,100,0,0,0,0,1],"282":[1,1,100,0,0,0,0,1],"283":[1,1,100,0,0,0,0,1],"284":[1,1,100,0,0,0,0,1],"285":[1,2,100,0,0,0,0,1],"286":[1,1,100,0,0,0,0,1],"287":[1,1,100,0,0,0,0,1],"288":[1,1,100,0,0,0,0,1],"289":[1,1,100,0,0,0,0,1],"290":[1,1,100,0,0,0,0,1],"291":[1,1,100,0,0,0,0,1],"292":[1,1,100,0,0,0,0,1],"293":[1,2,100,0,0,0,0,1],"294":[1,1,100,0,0,0,0,1],"295":[1,1,100,0,0,0,0,1],"296":[1,2,100,0,0,0,0,1],"297":[1,1,100,0,0,0,0,1],"298":[1,1,100,0,0,0,0,1],"299":[1,2,100,0,0,0,0,1],"300":[1,2,100,0,0,0,0,1],"301":[1,1,100,0,0,0,0,1],"302":[1,2,100,0,0,0,0,1],"303":[1,1,100,0,0,0,0,1],"304":[1,2,100,0,0,0,0,1],"305":[1,1,100,0,0,0,0,1],"306":[1,1,100,0,0,0,0,1],"307":[1,2,100,0,0,0,0,1],"308":[1,1,100,0,0,0,0,1],"309":[1,2,100,0,0,0,0,1],"310":[1,1,100,0,0,0,0,1],"311":[1,1,100,0,0,0,0,1],"312":[1,2,100,0,0,0,0,1],"313":[1,1,100,0,0,0,0,1],"314":[1,1,100,0,0,0,0,1],"315":[1,2,100,0,0,0,0,1],"316":[1,2,100,0,0,0,0,1],"317":[1,1,100,0,0,0,0,1],"318":[1,1,100,0,0,0,0,1],"319":[1,1,100,0,0,0,0,1],"320":[1,2,100,0,0,0,0,1],"321":[1,1,100,0,0,0,0,1],"322":[1,2,100,0,0,0,0,1],"323":[1,1,100,0,0,0,0,1],"324":[1,1,100,0,0,0,0,1],"325":[1,2,100,0,0,0,0,1],"326":[1,1,100,0,0,0,0,1],"327":[1,1,100,0,0,0,0,1],"328":[1,1,100,0,0,0,0,1],"329":[1,1,100,0,0,0,0,1],"330":[1,1,100,0,0,0,0,1],"331":[1,2,100,0,0,0,0,1],"332":[1,1,100,0,0,0,0,1],"333":[1,1,100,0,0,0,0,1],"334":[1,1,100,0,0,0,0,1],"335":[1,2,100,0,0,0,0,1],"336":[1,2,100,0,0,0,0,1],"337":[1,1,100,0,0,0,0,1],"338":[1,1,100,0,0,0,0,1],"339":[1,2,100,0,0,0,0,1],"340":[1,1,100,0,0,0,0,1],"341":[1,1,100,0,0,0,0,1],"342":[1,1,100,0,0,0,0,1],"343":[1,2,100,0,0,0,0,1],"344":[1,1,100,0,0,0,0,1],"345":[1,1,100,0,0,0,0,1],"346":[1,1,100,0,0,0,0,1],"347":[1,1,100,0,0,0,0,1],"348":[1,1,100,0,0,0,0,1],"349":[1,1,100,0,0,0,0,1],"350":[1,1,100,0,0,0,0,1],"351":[1,1,100,0,0,0,0,1],"352":[1,1,100,0,0,0,0,1],"353":[1,2,100,0,0,0,0,1],"354":[1,1,100,0,0,0,0,1],"355":[1,2,100,0,0,0,0,1],"356":[1,1,100,0,0,0,0,1],"357":[1,1,100,0,0,0,0,1],"358":[1,1,100,0,0,0,0,1],"359":[1,1,100,0,0,0,0,1],"360":[1,1,100,0,0,0,0,1],"361":[1,1,100,0,0,0,0,1],"362":[1,1,100,0,0,0,0,1],"363":[1,2,100,0,0,0,0,1],"364":[1,1,100,0,0,0,0,1],"365":[1,1,100,0,0,0,0,1],"366":[1,1,100,0,0,0,0,1],"367":[1,1,100,0,0,0,0,1],"368":[1,1,100,0,0,0,0,1],"369":[1,1,100,0,0,0,0,1],"370":[1,1,100,0,0,0,0,1],"371":[1,1,100,0,0,0,0,1],"372":[1,1,100,0,0,0,0,1],"373":[1,1,100,0,0,0,0,1],"374":[1,1,100,0,0,0,0,1],"375":[1,1,100,0,0,0,0,1],"376":[1,1,100,0,0,0,0,1],"380":[1,1,100,0,0,0,0,1],"387":[1,1,100,0,0,0,0,1],"388":[1,1,100,0,0,0,0,1],"389":[1,1,100,0,0,0,0,1],"390":[1,1,100,0,0,0,0,1],"391":[1,1,100,0,0,0,0,1],"392":[1,1,100,0,0,0,0,1],"393":[1,1,100,0,0,0,0,1],"394":[1,1,100,0,0,0,0,1],"395":[1,1,100,0,0,0,0,1],"396":[1,1,100,0,0,0,0,1],"397":[1,1,100,0,0,0,0,1],"398":[1,1,100,0,0,0,0,1],"399":[1,1,100,0,0,0,0,1],"400":[1,1,100,0,0,0,0,1],"401":[1,1,100,0,0,0,0,1],"402":[1,1,100,0,0,0,0,1],"403":[1,1,100,0,0,0,0,1],"404":[1,1,100,0,0,0,0,1],"405":[1,1,100,0,0,0,0,1],"406":[1,1,100,0,0,0,0,1],"407":[1,1,100,0,0,0,0,1],"408":[1,1,100,0,0,0,0,1],"409":[1,1,100,0,0,0,0,1],"410":[1,1,100,0,0,0,0,1],"411":[1,1,100,0,0,0,0,1],"412":[1,1,100,0,0,0,0,1],"415":[1,1,100,0,0,0,0,1],"416":[1,1,100,0,0,0,0,1],"418":[1,1,100,0,0,0,0,1],"419":[1,1,100,0,0,0,0,1],"420":[1,1,100,0,0,0,0,1],"421":[1,1,100,0,0,0,0,1],"422":[1,1,100,0,0,0,0,1],"423":[1,1,100,0,0,0,0,1],"425":[1,1,100,0,0,0,0,1],"426":[1,1,100,0,0,0,0,1],"427":[1,1,100,0,0,0,0,1],"428":[1,1,100,0,0,0,0,1],"429":[1,1,100,0,0,0,0,1],"430":[1,1,100,0,0,0,0,1],"431":[1,1,100,0,0,0,0,1],"432":[1,1,100,0,0,0,0,1],"433":[1,1,100,0,0,0,0,1],"434":[1,1,100,0,0,0,0,1],"435":[1,1,100,0,0,0,0,1],"436":[1,1,100,0,0,0,0,1],"437":[1,1,100,0,0,0,0,1],"442":[1,1,100,0,0,0,0,1],"443":[1,1,100,0,0,0,0,1],"444":[1,1,100,0,0,0,0,1],"446":[1,1,100,0,0,0,0,1],"447":[1,1,100,0,0,0,0,1],"448":[1,1,100,0,0,0,0,1],"449":[1,1,100,0,0,0,0,1],"450":[1,1,100,0,0,0,0,1],"451":[1,1,100,0,0,0,0,1],"452":[1,1,100,0,0,0,0,1],"453":[1,1,100,0,0,0,0,1],"454":[1,1,100,0,0,0,0,1],"455":[1,1,100,0,0,0,0,1],"456":[1,1,100,0,0,0,0,1],"457":[1,1,100,0,0,0,0,1],"458":[1,1,100,0,0,0,0,1],"459":[1,1,100,0,0,0,0,1],"460":[1,1,100,0,0,0,0,1],"464":[1,1,100,0,0,0,0,1],"466":[1,1,100,0,0,0,0,1],"467":[1,1,100,0,0,0,0,1],"468":[1,1,100,0,0,0,0,1],"480":[1,1,57,0,0,0,0,1],"481":[1,1,52,0,0,0,0,1],"482":[1,1,57,0,0,0,0,1],"485":[1,1,100,0,0,0,0,1],"487":[1,1,100,0,0,0,0,1],"488":[1,1,100,0,0,0,0,1],"495":[1,1,100,0,0,0,0,1],"496":[1,1,100,0,0,0,0,1],"497":[1,1,100,0,0,0,0,1],"498":[1,1,100,0,0,0,0,1],"499":[1,1,100,0,0,0,0,1],"500":[1,1,100,0,0,0,0,1],"501":[1,1,100,0,0,0,0,1],"502":[1,1,100,0,0,0,0,1],"503":[1,1,100,0,0,0,0,1],"504":[1,2,100,0,0,0,0,1],"505":[1,1,100,0,0,0,0,1],"506":[1,2,100,0,0,0,0,1],"507":[1,1,100,0,0,0,0,1],"508":[1,1,100,0,0,0,0,1],"509":[1,1,100,0,0,0,0,1],"510":[1,1,100,0,0,0,0,1],"511":[1,1,100,0,0,0,0,1],"512":[1,1,100,0,0,0,0,1],"513":[1,1,100,0,0,0,0,1],"514":[1,1,100,0,0,0,0,1],"515":[1,1,100,0,0,0,0,1],"516":[1,1,100,0,0,0,0,1],"517":[1,1,100,0,0,0,0,1],"518":[1,1,100,0,0,0,0,1],"519":[1,2,100,0,0,0,0,1],"520":[1,1,100,0,0,0,0,1],"521":[1,1,100,0,0,0,0,1],"522":[1,1,100,0,0,0,0,1],"523":[1,1,100,0,0,0,0,1],"524":[1,1,100,0,0,0,0,1],"525":[1,1,100,0,0,0,0,1],"526":[1,1,100,0,0,0,0,1],"527":[1,1,100,0,0,0,0,1],"528":[1,1,100,0,0,0,0,1],"529":[1,1,100,0,0,0,0,1],"530":[1,1,100,0,0,0,0,1],"531":[1,1,100,0,0,0,0,1],"532":[1,1,100,0,0,0,0,1],"533":[1,1,100,0,0,0,0,1],"534":[1,1,100,0,0,0,0,1],"535":[1,1,100,0,0,0,0,1],"536":[1,1,100,0,0,0,0,1],"537":[1,1,100,0,0,0,0,1],"538":[1,1,100,0,0,0,0,1],"539":[1,1,100,0,0,0,0,1],"540":[1,1,100,0,0,0,0,1],"541":[1,1,100,0,0,0,0,1],"542":[1,1,100,0,0,0,0,1],"543":[1,1,100,0,0,0,0,1],"544":[1,1,100,0,0,0,0,1],"545":[1,1,100,0,0,0,0,1],"546":[1,1,100,0,0,0,0,1],"547":[1,1,100,0,0,0,0,1],"548":[1,1,100,0,0,0,0,1],"549":[1,1,100,0,0,0,0,1],"550":[1,1,100,0,0,0,0,1],"551":[1,1,100,0,0,0,0,1],"552":[1,1,100,0,0,0,0,1],"553":[1,1,100,0,0,0,0,1],"554":[1,1,100,0,0,0,0,1],"555":[1,1,100,0,0,0,0,1],"556":[1,1,100,0,0,0,0,1],"557":[1,1,100,0,0,0,0,1],"558":[1,1,100,0,0,0,0,1],"559":[1,1,100,0,0,0,0,1],"560":[1,1,100,0,0,0,0,1],"561":[1,1,100,0,0,0,0,1],"562":[1,1,100,0,0,0,0,1],"563":[1,1,100,0,0,0,0,1],"564":[1,1,100,0,0,0,0,1],"565":[1,1,100,0,0,0,0,1],"566":[1,1,100,0,0,0,0,1],"567":[1,1,100,0,0,0,0,1],"568":[1,1,100,0,0,0,0,1],"569":[1,1,100,0,0,0,0,1],"570":[1,1,100,0,0,0,0,1],"571":[1,1,100,0,0,0,0,1],"572":[1,1,100,0,0,0,0,1],"573":[1,1,100,0,0,0,0,1],"574":[1,1,100,0,0,0,0,1],"575":[1,1,100,0,0,0,0,1],"576":[1,1,100,0,0,0,0,1],"577":[1,1,100,0,0,0,0,1],"578":[1,1,100,0,0,0,0,1],"579":[1,1,100,0,0,0,0,1],"580":[1,1,100,0,0,0,0,1],"581":[1,1,100,0,0,0,0,1],"582":[1,1,100,0,0,0,0,1],"583":[1,1,100,0,0,0,0,1],"584":[1,1,100,0,0,0,0,1],"585":[1,1,100,0,0,0,0,1],"586":[1,1,100,0,0,0,0,1],"587":[1,1,100,0,0,0,0,1],"588":[1,1,100,0,0,0,0,1],"589":[1,1,100,0,0,0,0,1],"590":[1,1,100,0,0,0,0,1],"591":[1,1,100,0,0,0,0,1],"592":[1,1,100,0,0,0,0,1],"593":[1,1,100,0,0,0,0,1],"594":[1,1,100,0,0,0,0,1],"595":[1,1,100,0,0,0,0,1],"596":[1,1,100,0,0,0,0,1],"597":[1,1,100,0,0,0,0,1],"598":[1,1,100,0,0,0,0,1],"599":[1,1,100,0,0,0,0,1],"600":[1,1,100,0,0,0,0,1],"601":[1,1,100,0,0,0,0,1],"602":[1,1,100,0,0,0,0,1],"603":[1,1,100,0,0,0,0,1],"604":[1,1,100,0,0,0,0,1],"605":[1,1,100,0,0,0,0,1],"606":[1,1,100,0,0,0,0,1],"607":[1,1,100,0,0,0,0,1],"608":[1,1,100,0,0,0,0,1],"609":[1,1,100,0,0,0,0,1],"610":[1,1,100,0,0,0,0,1],"611":[1,1,100,0,0,0,0,1],"612":[1,1,100,0,0,0,0,1],"613":[1,1,100,0,0,0,0,1],"614":[1,1,100,0,0,0,0,1],"615":[1,1,100,0,0,0,0,1],"616":[1,1,100,0,0,0,0,1],"617":[1,1,100,0,0,0,0,1],"618":[1,1,100,0,0,0,0,1],"619":[1,1,100,0,0,0,0,1],"620":[1,1,100,0,0,0,0,1],"621":[1,1,100,0,0,0,0,1],"622":[1,1,5,0,0,0,0,1],"623":[1,1,100,0,0,0,0,1],"624":[1,1,100,0,0,0,0,1],"625":[1,1,100,0,0,0,0,1],"626":[1,1,100,0,0,0,0,1],"627":[1,1,100,0,0,0,0,1],"628":[1,1,100,0,0,0,0,1],"629":[1,1,100,0,0,0,0,1],"630":[1,1,100,0,0,0,0,1],"631":[1,1,100,0,0,0,0,1],"632":[1,1,100,0,0,0,0,1],"633":[1,1,5,0,0,0,0,1],"634":[1,1,5,0,0,0,0,1],"635":[1,1,100,0,0,0,0,1],"636":[1,1,100,0,0,0,0,1],"637":[1,1,100,0,0,0,0,1]}},"time":{"fi":[0,80],"fl":[0,30],"fc":0,"w1":[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23],"w2":[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23]},"qest":{"n":0,"l":[]},"invs":{"n":0,"f":0,"l":[]},"more":{"l":"g"}}"#
    //     ).unwrap();
    //     let message = config.submit(&Local::now(), &Request::Pokemon(Box::new(Pokemon {
    //         spawnpoint_id: String::new(),
    //         encounter_id: String::new(),
    //         pokemon_id: 100,
    //         pokestop_id: None,
    //         latitude: 44.066772,
    //         longitude: 12.582556,
    //         disappear_time: 1510783800,
    //         disappear_time_verified: true,
    //         last_modified_time: 1510780200,
    //         first_seen: 1510780200,
    //         gender: Gender::Male,
    //         cp: Some(197),
    //         form: None,
    //         costume: None,
    //         individual_attack: Some(15),
    //         individual_defense: Some(15),
    //         individual_stamina: Some(15),
    //         cp_multiplier: None,
    //         move_1: None,
    //         move_2: None,
    //         weight: None,
    //         height: None,
    //         base_catch: None,
    //         great_catch: None,
    //         ultra_catch: None,
    //         boosted_weather: None,
    //         def_grade: None,
    //         atk_grade: None,
    //         rating_attack: None,
    //         rating_defense: None,
    //         catch_prob_1: None,
    //         catch_prob_2: None,
    //         catch_prob_3: None,
    //         weather: None,
    //         weather_boosted_condition: None,
    //         pokemon_level: Some(7),
    //         s2_cell_id:None,
    //         username: None,
    //         shiny: None,
    //         display_pokemon_id: None,
    //     }))).unwrap();
    //     message.prepare(Local::now()).await.unwrap();
    // }
}

use std::{collections::HashMap, sync::Arc, time::Duration};

use futures_util::stream::unfold;

use stream_throttle::{ThrottlePool, ThrottleRate, ThrottledStream};

use tokio::{spawn, sync::{RwLock, RwLockWriteGuard, broadcast, Mutex}, time::interval};

use mysql_async::{from_row, prelude::Queryable, params};

use chrono::{Local, DateTime, Timelike, offset::TimeZone};

use lru_time_cache::LruCache;

use geo::Point;

use geo_raycasting::RayCasting;

use once_cell::sync::Lazy;

use log::{info, error, debug, warn};

use rocketmap_entities::{Request, Watch, DeviceTier};

mod config;
mod message;
mod map;
mod select_all;
mod file_cache;

use message::{Message, DeviceTierMessage, LagMessage};

use crate::lists::{CITIES, CITYSTATS, CITYPARKS, City, CityStats};
use crate::config::CONFIG;
use crate::db::MYSQL;
use crate::telegram::send_message;

static BOT_CONFIGS: Lazy<RwLock<HashMap<String, config::BotConfig>>> = Lazy::new(|| RwLock::new(HashMap::new()));
static WATCHES: Lazy<RwLock<HashMap<String, Vec<Watch>>>> = Lazy::new(|| RwLock::new(HashMap::new()));
#[allow(clippy::type_complexity)]
static SENDER: Lazy<broadcast::Sender<Arc<(DateTime<Local>, Request)>>> = Lazy::new(|| {
    let (tx, _) = broadcast::channel(CONFIG.service.queue_size);
    tx
});
static SENT_CACHE: Lazy<Mutex<LruCache<String, ()>>> = Lazy::new(|| Mutex::new(LruCache::with_expiry_duration(Duration::from_secs(3600))));//1 hour cache

const RATE_LIMITER_CHECK_INTERVAL: u8 = 10;
const MAX_NOTIFICATIONS_PER_HOUR: u32 = 500;

#[derive(PartialEq)]
enum LoadResult {
    Ok,
    Disabled,
    Flood,
    Invalid,
    Error,
}

pub struct BotConfigs;

impl BotConfigs {
    pub async fn init() -> Result<(), ()> {
        // load first config
        {
            let mut res = BOT_CONFIGS.write().await;
            Self::load(&mut res, None).await?;
        }

        // set reload interval
        spawn(async {
            let mut interval = interval(Duration::from_secs(60));
            let mut index: u8 = 0;
            loop {
                interval.tick().await;

                //being index an u8, if 255 is not a multiple of RATE_LIMITER_CHECK_INTERVAL we'll get a shift every round
                index = index.wrapping_add(1);

                // the intent here was to reaload only expired users, but this won't block users who hit rate limiter
                // repeating the query (really similar to the one done in BotConfigs::load) every minute would be a waste of resources
                // so this is an (ugly) hybrid solution
                let uids = if index % RATE_LIMITER_CHECK_INTERVAL == 0 {
                    // here we can't use functional-style code because of async
                    if let Ok(mut conn) = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e)) {
                        let query = format!("SELECT b.user_id
                            FROM utenti_config_bot b
                            INNER JOIN utenti u ON u.user_id = b.user_id
                            INNER JOIN city c ON c.id = u.city_id
                            LEFT JOIN utenti_bot_stats s ON s.user_id = b.user_id AND s.day = CURDATE()
                            WHERE b.enabled = 1 AND b.beta = 1 AND u.status != 0 AND (CAST(IFNULL(s.sent, 0) / (HOUR(NOW()) + 1) AS UNSIGNED) > {} OR c.scadenza < UNIX_TIMESTAMP())", MAX_NOTIFICATIONS_PER_HOUR);

                        if let Ok(res) = conn.query_iter(query).await.map_err(|e| error!("MySQL query error: get users to disable\n{}", e)) {
                            res.map_and_drop(|mut row| row.take("user_id").unwrap_or_else(String::new)).await.map_err(|e| error!("MySQL map error: get users to disable\n{}", e))
                        }
                        else {
                            Err(())
                        }
                    }
                    else {
                        Err(())
                    }
                }
                else {
                    let lock = BOT_CONFIGS.read().await;
                    let now = Some(Local::now().timestamp());
                    Ok(lock.iter().filter(|(_, config)| now > config.scadenza).map(|(id, _)| id.clone()).collect::<Vec<String>>())
                };

                if let Ok(user_ids) = uids {
                    if !user_ids.is_empty() {
                        let mut lock = BOT_CONFIGS.write().await;
                        if let Ok(res) = Self::load(&mut lock, Some(user_ids)).await {
                            for (user_id, result) in res {
                                Self::notify_user(user_id, result, true).ok();
                            }
                        }
                    }
                }
            }
        });

        // load weather watches
        {
            let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
            let res = conn.query_iter("SELECT user_id, encounter_id, pokemon_id, iv, latitude, longitude, expire FROM bot_weather_watches WHERE expire > UNIX_TIMESTAMP()").await.map_err(|e| error!("MySQL query error: get weather watches\n{}", e))?;
            let mut lock = WATCHES.write().await;
            res.for_each_and_drop(|row| {
                let (user_id, encounter_id, pokemon_id, iv, latitude, longitude, expire) = from_row::<(String, String, u16, Option<u8>, f64, f64, i64)>(row);
                let entry = lock.entry(user_id.clone()).or_insert_with(Vec::new);
                entry.push(Watch {
                    user_id,
                    encounter_id,
                    pokemon_id,
                    iv,
                    point: (latitude, longitude).into(),
                    expire,
                    // reference_weather: None,
                });
            }).await.map_err(|e| error!("MySQL collect error: {}", e))?;
        }
        
        Ok(())
    }

    async fn reload_city(city_id: u16) -> Result<(), ()> {
        let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
        let res = conn.exec_iter(
                "SELECT id, name, coordinates, scadenza, monitor, admins_users FROM city WHERE id = :id",
                params! {
                    "id" => city_id,
                }
            ).await.map_err(|e| error!("MySQL query error: get city\n{}", e))?;

        if !res.is_empty() {
            {
                let mut cities = CITIES.write().await;
                res.for_each_and_drop(|row| {
                    let city: City = row.into();
                    cities.insert(city.id, city);
                }).await.map_err(|e| error!("MySQL for_each error: {}", e))?
            }

            let res = conn.exec_iter(
                    "SELECT user_id FROM utenti WHERE city_id = :id",
                    params! {
                        "id" => city_id
                    }
                ).await.map_err(|e| error!("MySQL query error: get city users\n{}", e))?;
            // let (_, user_ids) = res.collect_and_drop().await.map_err(|e| error!("MySQL collect error: {}", e))?;
            let user_ids = res.map_and_drop(|mut row| row.take::<u64, _>("user_id").map(|i| i.to_string()).unwrap_or_else(String::new)).await.map_err(|e| error!("MySQL collect error: {}", e))?;

            let mut lock = BOT_CONFIGS.write().await;
            let res = Self::load(&mut lock, Some(user_ids)).await?;
            for (user_id, result) in res {
                Self::notify_user(user_id, result, true)?;
            }
        }

        Ok(())
    }

    fn notify_user(user_id: String, result: LoadResult, silent: bool) -> Result<(), ()> {
        let msg = match result {
            LoadResult::Ok => {
                if silent {
                    return Ok(());
                }

                info!("Successfully reloaded configs for user {}", user_id);
                // $msg = "\xe2\x84\xb9\xef\xb8\x8f <b>Impostazioni modificate!</b>\n";
                // $msg .= "<code>      ───────</code>\n";
                // $msg .= "Le modifiche sono state applicate.";
                // if($e == 0){ $msg .= "\nRicorda di attivare la ricezione delle notifiche con: /start";}
                // SendTelegram($USER["user_id"], $msg);
                format!("{} <b>Impostazioni modificate!</b>\n<code>      ───────</code>\nLe modifiche sono state applicate.",
                    String::from_utf8(vec![0xe2, 0x84, 0xb9, 0xef, 0xb8, 0x8f]).map_err(|e| error!("error converting info icon: {}", e))?)
            },
            LoadResult::Flood => {
                warn!("User {} is flooding", user_id);
                format!("{} <b>Troppe notifiche!</b>\n<code>      ───────</code>\nLe tue configurazioni generano troppe notifiche, rivedile per limitarne il numero. ",
                    String::from_utf8(vec![0xE2, 0x9A, 0xA0]).map_err(|e| error!("error converting warning icon: {}", e))?)
            },
            LoadResult::Invalid => {
                if silent {
                    return Ok(());
                }

                warn!("User {} has invalid configs", user_id);
                format!("{} <b>Impostazioni non valide!</b>\n<code>      ───────</code>\nControlla che i tuoi cursori siano all'interno della tua città di appartenenza.\nSe hai bisogno di spostarti temporaneamente, invia la tua nuova posizione al bot per usarla come posizione temporanea.",
                    String::from_utf8(vec![0xE2, 0x9A, 0xA0]).map_err(|e| error!("error converting warning icon: {}", e))?)
            },
            LoadResult::Disabled => {
                if silent {
                    return Ok(());
                }

                warn!("User {} has been disabled", user_id);
                format!("{} <b>Impostazioni modificate!</b>\n<code>      ───────</code>\nLe modifiche sono state applicate.\nRicorda di attivare la ricezione delle notifiche con: /start",
                    String::from_utf8(vec![0xe2, 0x84, 0xb9, 0xef, 0xb8, 0x8f]).map_err(|e| error!("error converting info icon: {}", e))?)
            },
            LoadResult::Error => {
                if silent {
                    return Ok(());
                }

                error!("Error reloading configs for user {}", user_id);
                format!("{} <b>Errore!</b>\n<code>      ───────</code>\nC'è stato un errore applicando le tue nuove impostazioni, se il problema persiste contatta il tuo amministratore di zona.",
                    String::from_utf8(vec![0xF0, 0x9F, 0x9B, 0x91]).map_err(|e| error!("error converting error icon: {}", e))?)
            },
        };

        spawn(async move {
            send_message(&CONFIG.telegram.bot_token, &user_id, &msg)
                .set_parse_mode("HTML")
                .send()
                .await
                .map_err(|_| ())
                .ok();
        });

        Ok(())
    }

    pub async fn reload(user_ids: Vec<String>) -> Result<(), ()> {
        let mut lock = BOT_CONFIGS.write().await;
        let res = Self::load(&mut lock, Some(user_ids.clone())).await?;
        for (user_id, result) in res {
            Self::notify_user(user_id, result, false)?;
        }
        Ok(())
    }

    async fn load(configs: &mut HashMap<String, config::BotConfig>, user_ids: Option<Vec<String>>) -> Result<HashMap<String, LoadResult>, ()> {
        let query = format!("SELECT b.enabled, b.user_id, b.config, b.beta, u.status, c.scadenza, u.city_id, CAST(IFNULL(s.sent, 0) / (HOUR(NOW()) + 1) AS UNSIGNED)
            FROM utenti_config_bot b
            INNER JOIN utenti u ON u.user_id = b.user_id
            INNER JOIN city c ON c.id = u.city_id AND c.scadenza > UNIX_TIMESTAMP()
            LEFT JOIN utenti_bot_stats s ON s.user_id = b.user_id AND s.day = CURDATE()
            WHERE {}", user_ids.as_ref().and_then(|v| if v.is_empty() {
                    None
                }
                else {
                    Some(format!("b.user_id IN ({})", v.join(", ")))
                }).unwrap_or_else(|| String::from("b.enabled = 1 AND b.beta = 1 AND u.status != 0")));

        let mut ids = user_ids.unwrap_or_else(|| configs.iter().map(|(id, _)| id.clone()).collect());

        let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
        let res = conn.query_iter(query).await.map_err(|e| error!("MySQL query error: get users configs\n{}", e))?;

        let mut results = HashMap::new();
        let temp = res.map_and_drop(from_row::<(u8, u64, String, u8, u8, i64, u16, u32)>).await.map_err(|e| error!("MySQL collect error: {}", e))?;
        for (enabled, user_id, config, beta, status, scadenza, city_id, sent) in temp {
            let id = user_id.to_string();
            let pos = ids.iter().position(|i| i == &id);
            let result = Self::load_user(configs, enabled, id.clone(), config, beta, status, city_id, scadenza, sent).await.unwrap_or(LoadResult::Error);
            if result == LoadResult::Ok {
                if let Some(i) = pos {
                    ids.remove(i);
                }
            }
            results.insert(id, result);
        }

        for id in ids.into_iter() {
            configs.remove(&id);
        }

        Ok(results)
    }

    #[allow(clippy::too_many_arguments)]
    async fn load_user(configs: &mut HashMap<String, config::BotConfig>, enabled: u8, user_id: String, config: String, beta: u8, status: u8, city_id: u16, scadenza: i64, sent: u32) -> Result<LoadResult, ()> {
        if enabled > 0 && beta > 0 && status > 0 {
            if sent < MAX_NOTIFICATIONS_PER_HOUR {
                let mut config: config::BotConfig = serde_json::from_str(&config).map_err(|e| error!("MySQL utenti_config_bot.config decoding error for user_id {}: {}", user_id, e))?;
                if config.validate(&user_id, city_id).await? {
                    config.user_id = Some(user_id.clone());
                    config.scadenza = Some(scadenza);
                    if let Some(c) = configs.get_mut(&user_id) {
                        *c = config;
                    }
                    else {
                        configs.insert(user_id.clone(), config);
                        let stream = unfold((SENDER.subscribe(), user_id), |(mut rx, user_id)| Box::pin(async {
                            let res: select_all::Message;
                            loop {
                                let temp = match rx.recv().await {
                                    Ok(t) => t,
                                    Err(broadcast::error::RecvError::Lagged(lag)) => {
                                        res = (user_id.clone(), Box::new(LagMessage { lag }), String::new());
                                        break;
                                    },
                                    _ => return None,
                                };
                                let (time, req) = temp.as_ref();
                                let lock = BOT_CONFIGS.read().await;
                                let conf = lock.get(&user_id)?;
                                if let Ok(msg) = conf.submit(time, req).await {
                                    res = (user_id.clone(), msg, conf.more.l.clone());
                                    break;
                                }
                            }
                            Some((res, (rx, user_id)))
                        }));
                        // We can send a single message per telegram chat per second
                        let rate = ThrottleRate::new(1, Duration::from_secs(1));
                        let pool = ThrottlePool::new(rate);
                        select_all::add(stream.throttle(pool)).await.ok();
                    }

                    Ok(LoadResult::Ok)
                }
                else {
                    Ok(LoadResult::Invalid)
                }
            }
            else {
                Ok(LoadResult::Flood)
            }
        }
        else {
            Ok(LoadResult::Disabled)
        }
    }

    async fn clean_watches<'a, 'b>(now: i64, watch: &'a Watch) -> RwLockWriteGuard<'b, HashMap<String, Vec<Watch>>> {
        // remove expired watches
        let mut lock = WATCHES.write().await;
        for (_, v) in lock.iter_mut() {
            let mut remove = Vec::new();
            for (index, w) in v.iter().enumerate() {
                if w.expire < now || w == watch {
                    remove.push(index);
                }
            }
            for index in remove.into_iter().rev() {
                v.remove(index);
            }
        }

        lock
    }

    async fn remove_watches(watch: Box<Watch>) -> Result<(), ()> {
        let now = Local::now().timestamp();

        Self::clean_watches(now, &watch).await;

        let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
        conn.exec_drop(
            "DELETE FROM bot_weather_watches WHERE expire < UNIX_TIMESTAMP() OR (user_id = :user_id AND encounter_id = :encounter_id AND pokemon_id = :pokemon_id AND iv = :iv AND latitude = :latitude AND longitude = :longitude AND expire = :expire)",
            params! {
                "user_id" => watch.user_id.clone(),
                "encounter_id" => watch.encounter_id.clone(),
                "pokemon_id" => watch.pokemon_id,
                "iv" => watch.iv,
                "latitude" => watch.point.x(),
                "longitude" => watch.point.y(),
                "expire" => watch.expire,
            }
        ).await.map_err(|e| error!("MySQL delete error: {}", e))?;

        Ok(())
    }

    async fn add_watches(watch: Box<Watch>) -> Result<(), ()> {
        let now = Local::now().timestamp();

        let mut lock = Self::clean_watches(now, &watch).await;

        if watch.expire > now && Local::now().hour() != Local.timestamp(watch.expire, 0).hour() && lock.get(&watch.user_id).map(|v| v.contains(&watch)) != Some(true) {
            let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
            conn.query_drop("DELETE FROM bot_weather_watches WHERE expire < UNIX_TIMESTAMP()").await.map_err(|e| error!("MySQL delete error: {}", e))?;
            conn.exec_drop(
                "INSERT INTO bot_weather_watches (user_id, encounter_id, pokemon_id, iv, latitude, longitude, expire) VALUES (:user_id, :encounter_id, :pokemon_id, :iv, :latitude, :longitude, :expire)",
                params! {
                    "user_id" => watch.user_id.clone(),
                    "encounter_id" => watch.encounter_id.clone(),
                    "pokemon_id" => watch.pokemon_id,
                    "iv" => watch.iv,
                    "latitude" => watch.point.x(),
                    "longitude" => watch.point.y(),
                    "expire" => watch.expire,
                }
            ).await.map_err(|e| error!("MySQL insert error: insert weather watch\n{}", e))?;

            let entry = lock.entry(watch.user_id.clone()).or_insert_with(Vec::new);
            entry.push(*watch);
        }

        Ok(())
    }

    // async fn submit_weather(weather: Box<Weather>, now: DateTime<Local>) {
    //     let timestamp = now.timestamp();
    //     let time = now.format("%T").to_string();

    //     let mut remove = Vec::new();
    //     let mut fire = Vec::new();
    //     let mut lock = WATCHES.lock().await;
    //     let users = BOT_CONFIGS.read().await;
    //     for (index, watch) in lock.iter_mut().enumerate() {
    //         if watch.expire < timestamp {
    //             remove.push(index);
    //             continue;
    //         }

    //         if weather.polygon.within(&watch.point) {
    //             let debug = users.get(&watch.user_id).and_then(|c| c.debug);
    //             fire.push((index, debug));
    //         }
    //     }

    //     for (index, debug) in fire.into_iter() {
    //         let message = WeatherMessage {
    //             watch: lock[index].clone(),
    //             // actual_weather: weather.clone(),
    //             debug: if debug == Some(true) { Some(time.clone()) } else { None },
    //         };

    //         spawn(async move {
    //             let lock = BOT_CONFIGS.read().await;
    //             if let Some(l) = lock.get(&message.watch.user_id).map(|c| c.more.l.clone()) {
    //                 if let Ok(file_id) = message.prepare(Local::now()).await {
    //                     message.send(&message.watch.user_id, file_id, l.as_str()).await
    //                         .map_err(|_| error!("Error sending weather notification"))
    //                         .ok();
    //                 }
    //             }
    //         });
    //     }

    //     for index in remove.into_iter().rev() {
    //         lock.remove(index);
    //     }
    // }

    pub async fn submit(now: DateTime<Local>, inputs: Vec<Request>) {
        let mut lock = SENT_CACHE.lock().await;
        for input in inputs.into_iter().filter(|r| match r {
            Request::Reload(_) |
            Request::ReloadCity(_) |
            Request::StartWatch(_) |
            Request::StopWatch(_) |
            Request::DeviceTier(_) => true,
            _ => lock.notify_insert(format!("{:?}", r), ()).0.is_none(),
        }) {
            // non config-related requests
            match input {
                Request::Reload(user_ids) => {
                    spawn(async {
                        BotConfigs::reload(user_ids).await.ok();
                    });
                    continue;
                },
                Request::ReloadCity(city_id) => {
                    spawn(async move {
                        BotConfigs::reload_city(city_id).await.ok();
                    });
                    continue;
                },
                Request::StartWatch(watches) => {
                    spawn(async {
                        BotConfigs::add_watches(watches).await.ok();
                    });
                    continue;
                },
                Request::StopWatch(watches) => {
                    spawn(async {
                        BotConfigs::remove_watches(watches).await.ok();
                    });
                    continue;
                },
                // Request::Weather(weather) => {
                //     spawn(async move {
                //         BotConfigs::submit_weather(weather, now).await;
                //     });
                //     continue;
                // },
                Request::Pokemon(ref p) => {
                    BotConfigs::update_park_stats((p.latitude, p.longitude).into(), p.pokemon_id, p.encounter_id.clone());

                    BotConfigs::update_city_stats(&input, now.timestamp());
                },
                Request::Raid(_) | Request::Invasion(_) | Request::Quest(_) => {
                    BotConfigs::update_city_stats(&input, now.timestamp());
                },
                Request::DeviceTier(dt) => {
                    spawn(async move {
                        BotConfigs::update_device_tier(&dt).await.ok();
                    });
                    continue;
                },
                Request::Weather(_) | Request::Pokestop(_) | Request::GymDetails(_) => {},
                _ => debug!("Unmanaged webhook: {:?}", input),
            }

            // let mut messages = Vec::new();
            // {
            //     let lock = BOT_CONFIGS.read().await;
            //     for (chat_id, config) in lock.iter() {
            //         if let Ok(message) = config.submit(&now, &input).await {
            //             messages.push((chat_id.clone(), message, config.more.l.clone()));
            //         }
            //     }
            // }

            // if !messages.is_empty() {
            //     spawn(async move {
            //         if let Ok(file_id) = messages[0].1.prepare(now).await {
            //             for (chat_id, message, map_type) in messages.into_iter() {
            //                 let file_id = file_id.clone();
            //                 spawn(async move {
            //                     message.send(&chat_id, file_id, &map_type).await.ok();
            //                 });
            //             }
            //         }
            //     });
            // }
            SENDER.send(Arc::new((now, input))).map_err(|e| error!("Stream send error: {}", e)).ok();
        }
    }

    fn update_park_stats(point: Point<f64>, pokemon_id: u16, encounter_id: String) {
        spawn(async move {
            let city = {
                let lock = CITIES.read().await;
                lock.iter().find_map(|(id, city)| {
                    if city.coordinates.within(&point) {
                        Some(*id)
                    }
                    else {
                        None
                    }
                })
            };

            if let Some(city_id) = city {
                let lock = CITYPARKS.read().await;
                if let Some(parks) = lock.get(&city_id) {
                    for park in parks {
                        if park.coordinates.within(&point) {
                            match MYSQL.get_conn().await {
                                Ok(mut conn) => {
                                    conn.exec_drop("REPLACE INTO city_park_stats (park_id, encounter_id, pokemon_id) VALUES (:park_id, :encounter_id, :pokemon_id)", params! {
                                            "park_id" => park.id,
                                            "encounter_id" => encounter_id.as_str(),
                                            "pokemon_id" => pokemon_id,
                                        }).await
                                        .map_err(|e| error!("MySQL query error: insert park stat\n{}", e)).ok();
                                },
                                Err(e) => error!("MySQL retrieve connection error: {}", e),
                            }
                        }
                    }
                }
            }
        });
    }

    fn update_city_stats(input: &Request, now: i64) {
        match input {
            Request::Pokemon(p) => {
                let iv = matches!((p.individual_attack, p.individual_defense, p.individual_stamina), (Some(_), Some(_), Some(_)));
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
                                let entry = lock.entry(*id).or_insert_with(CityStats::default);
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
                                let entry = lock.entry(*id).or_insert_with(CityStats::default);
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
                                let entry = lock.entry(*id).or_insert_with(CityStats::default);
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
                                let entry = lock.entry(*id).or_insert_with(CityStats::default);
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

    async fn update_device_tier(dt: &DeviceTier) -> Result<(), ()> {
        let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
        if let Some(name) = &dt.name {
            conn.exec_drop(
                    "REPLACE INTO device_tier (id, name, url, release_date, app_version, api_version, reboot, uninstall) VALUES (:id, :name, :url, :release_date, :app_version, :api_version, :reboot, :uninstall)",
                    params! {
                        "id" => dt.id,
                        "name" => name,
                        "url" => &dt.url,
                        "release_date" => dt.release_date,
                        "app_version" => &dt.app_version,
                        "api_version" => &dt.api_version,
                        "reboot" => dt.reboot,
                        "uninstall" => dt.uninstall,
                    }
                )
        }
        else {
            conn.exec_drop(
                    "UPDATE device_tier SET url = :url, release_date = :release_date, app_version = :app_version, api_version = :api_version, reboot = :reboot, uninstall = :uninstall WHERE id = :id",
                    params! {
                        "id" => dt.id,
                        "url" => &dt.url,
                        "release_date" => dt.release_date,
                        "app_version" => &dt.app_version,
                        "api_version" => &dt.api_version,
                        "reboot" => dt.reboot,
                        "uninstall" => dt.uninstall,
                    }
                )
        }.await.map_err(|e| error!("MySQL query error: update device tier\n{}", e))?;

        if let Some(version_chat) = &CONFIG.telegram.version_chat {
            let message = DeviceTierMessage {
                tier: dt,
            };
            let image = message.get_image().await?;
            message.send(version_chat, image, "").await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::message::{Message, PokemonMessage, RaidMessage, InvasionMessage, GymMessage};

    #[tokio::test]
    async fn pokemon_image_iv() {
        env_logger::try_init().ok();

        let message = PokemonMessage {
            pokemon: serde_json::from_str(
                    r#"{"latitude":43.889,"last_modified_time":1564175718,"gender":2,"weight":1.48829138278961,"individual_stamina":13,"cp":656,"individual_attack":15,"pokemon_level":30,"spawnpoint_id":"A54016D1","disappear_time_verified":true,"form":0,"individual_defense":4,"first_seen":1564174573,"pokestop_id":"e5bab95925c040ba8ba9d480665f94dc.16","encounter_id":"12854125514185017067","longitude":12.909,"costume":0,"move_1":216,"disappear_time":1564175797,"move_2":58,"weather":1,"pokemon_id":339,"height":0.409816652536392}"#
                ).unwrap(),
            iv: Some(100),
            distance: 0_f64,
            direction: String::new(),
            debug: None,
        };
        message.get_image().await.unwrap();
    }

    #[tokio::test]
    async fn pokemon_image_no_iv() {
        env_logger::try_init().ok();

        let message = PokemonMessage {
            pokemon: serde_json::from_str(
                    r#"{"latitude":43.771081,"last_modified_time":1564175718,"gender":2,"weight":null,"individual_stamina":null,"cp":null,"individual_attack":null,"pokemon_level":null,"spawnpoint_id":"None","disappear_time_verified":false,"form":0,"individual_defense":null,"first_seen":1564175718,"pokestop_id":"34eadde33a9a412e9cf74a6137b38d48.16","encounter_id":"1432619712785063368","longitude":11.266252,"costume":0,"move_1":null,"disappear_time":1564176918,"move_2":null,"weather":0,"pokemon_id":109,"height":null}"#
                ).unwrap(),
            iv: Some(100),
            distance: 0_f64,
            direction: String::new(),
            debug: None,
        };
        message.get_image().await.unwrap();
    }

    #[tokio::test]
    async fn raid_image_egg() {
        env_logger::try_init().ok();

        let message = RaidMessage {
            raid: serde_json::from_str(
                    r#"{"gym_name":"Cesena - Stadio Dino Manuzzi","gym_url":"http://lh3.googleusercontent.com/EDFoOOm1G0hhjtBdJmBUjcUAcfB1se0zPatRviNuHcsK2oxwIokSONXWb8CUAJYYHI0Cl32dWiVlIMXVZe_X","end":1564338495,"form":0,"is_exclusive":false,"longitude":12.413359,"cp":0,"team_id":1,"pokemon_id":0,"gym_id":"fe20dd37398341a4b83751c5c050aaec.16","move_2":0,"level":4,"move_1":0,"start":1564335795,"ex_raid_eligible":false,"spawn":1564332195,"latitude":45.567758,"gender":0}"#
                ).unwrap(),
            distance: 0_f64,
            debug: None,
        };
        message.get_image().await.unwrap();
    }

    #[tokio::test]
    async fn raid_image_raidboss() {
        env_logger::try_init().ok();

        let message = RaidMessage {
            raid: serde_json::from_str(
                    r#"{"latitude":45.468203,"team_id":2,"start":1564212138,"end":1564214838,"spawn":1564208538,"cp":2527,"ex_raid_eligible":false,"form":63,"gym_id":"03213b28587c4063b81c6d9ed39e5f54.16","gym_name":"Parrocchia di San Pietro in Sala","longitude":9.156488,"move_1":202,"move_2":16,"is_exclusive":false,"gym_url":"http://lh5.ggpht.com/5sxTUTkH0Ch1l-w-jMN8i_2-wz1XVjcPv4EAHTBJunHONWr7KFIWHjnh_RfU0lqKPL4j12sSgQJKyKNqD3-p","level":1,"pokemon_id":52,"gender":1}"#
                ).unwrap(),
            distance: 0_f64,
            debug: None,
        };
        message.get_image().await.unwrap();
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
        message.get_image().await.unwrap();
    }

    #[tokio::test]
    async fn gym_image() {
        env_logger::try_init().ok();

        let message = GymMessage {
            gym: serde_json::from_str(
                    r#"{"name":"Genova - Ponte Monumentale","latitude":44.405798,"slots_available":5,"sponsor_od":0,"team":2,"id":"e697b9b81d24466f90f97fe362a9bc02.11","url":"http://lh3.googleusercontent.com/Xg9dpwYjV3wEHSbLdsJep2duokPdQEuUt3iwcxVhajTt1OucrYU9p-nFYnn60cOP42Uf3MYn3zJ3hyiDaEPqi3-WyA","longitude":8.939554,"ex_raid_eligible":false,"in_battle":false}"#
                ).unwrap(),
            distance: 0_f64,
            debug: None,
        };
        message.get_image().await.unwrap();
    }
}

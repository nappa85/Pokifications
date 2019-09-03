use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Instant, Duration};

use future_parking_lot::rwlock::{RwLock, FutureReadable, FutureWriteable};

use tokio::timer::delay;
use tokio::spawn;

use chrono::{Local, DateTime};

use lazy_static::lazy_static;

use log::{info, error};

mod config;
mod message;

use message::{Image, Message, PokemonMessage, RaidMessage, InvasionMessage};

use crate::entities::Request;
use crate::db::MYSQL;

lazy_static! {
    static ref BOT_CONFIGS: Arc<RwLock<HashMap<String, config::BotConfig>>> = Arc::new(RwLock::new(BotConfigs::init()));
}

pub struct BotConfigs;

impl BotConfigs {
    fn init() -> HashMap<String, config::BotConfig> {
        let mut res = HashMap::new();
        Self::load(&mut res, None).expect("Unable to init BotConfigs");
        res
    }

    pub async fn reload(user_ids: Vec<String>) -> Result<(), ()> {
        let mut lock = BOT_CONFIGS.future_write().await;
        info!("reloading configs for users {:?}", user_ids);
        Self::load(&mut lock, Some(user_ids))
    }

    fn load(configs: &mut HashMap<String, config::BotConfig>, user_ids: Option<Vec<String>>) -> Result<(), ()> {
        if let Some(ref user_ids) = user_ids {
            for user_id in user_ids {
                configs.remove(user_id);
            }
        }

        let query = format!("SELECT b.enabled, b.user_id, b.config, b.beta, u.status, c.scadenza FROM utenti_config_bot b
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
        for r in res {
            let mut row = r.map_err(|e| error!("MySQL row error: {}", e))?;

            let enabled: u8 = row.take("enabled").ok_or_else(|| error!("MySQL utenti_config_bot.enabled encoding error"))?;
            let user_id: String = row.take("user_id").ok_or_else(|| error!("MySQL utenti_config_bot.user_id encoding error"))?;
            let config: String = row.take("config").ok_or_else(|| error!("MySQL utenti_config_bot.config encoding error"))?;
            let beta: u8 = row.take("beta").ok_or_else(|| error!("MySQL utenti_config_bot.beta encoding error"))?;
            let status: u8 = row.take("status").ok_or_else(|| error!("MySQL utenti.status encoding error"))?;

            if enabled > 0 && beta > 0 && status > 0 {
                let config: config::BotConfig = serde_json::from_str(&config).map_err(|e| error!("MySQL utenti_config_bot.config decoding error: {}", e))?;
                configs.insert(user_id.clone(), config);

                let scadenza: u64 = row.take("scadenza").ok_or_else(|| error!("MySQL city.scadenza encoding error"))?;
                spawn(async move {
                    delay(Instant::now() + Duration::from_secs(scadenza - now)).await;
                    //.map_err(|e| error!("timer error: {}", e))
                    BotConfigs::reload(vec![user_id]).await.ok();
                });
            }
        }

        Ok(())
    }

    pub async fn submit(now: DateTime<Local>, inputs: Vec<Request>) {
        for input in inputs.into_iter() {
            if let Request::Reload(user_ids) = input {
                spawn(async {
                    BotConfigs::reload(user_ids).await.ok();
                });
                continue;
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
}

#[cfg(test)]
mod tests {
    use super::message::{self, Message, PokemonMessage, RaidMessage, InvasionMessage};

    use tokio::prelude::Future;
    use tokio::runtime::current_thread::Runtime;

    use chrono::Local;

    fn block_on_all<F: Future>(f: F) -> F::Output {
        let mut runtime = Runtime::new().unwrap();
        runtime.block_on(f)
    }

    #[test]
    fn pokemon_image_iv() {
        env_logger::try_init().ok();

        block_on_all(message::prepare(PokemonMessage::get_dummy(serde_json::from_str(
                r#"{"latitude":43.771209013571,"last_modified_time":1564175718,"gender":2,"weight":1.48829138278961,"individual_stamina":13,"cp":656,"individual_attack":15,"pokemon_level":30,"spawnpoint_id":"A54016D1","disappear_time_verified":true,"form":0,"individual_defense":4,"first_seen":1564174573,"pokestop_id":"e5bab95925c040ba8ba9d480665f94dc.16","encounter_id":"12854125514185017067","longitude":11.2560545151937,"costume":0,"move_1":216,"disappear_time":1564175797,"move_2":58,"weather":1,"pokemon_id":339,"height":0.409816652536392}"#
            ).unwrap()), Local::now())).unwrap();
    }

    #[test]
    fn pokemon_image_no_iv() {
        env_logger::try_init().ok();

        block_on_all(message::prepare(PokemonMessage::get_dummy(serde_json::from_str(
                r#"{"latitude":43.771081,"last_modified_time":1564175718,"gender":2,"weight":null,"individual_stamina":null,"cp":null,"individual_attack":null,"pokemon_level":null,"spawnpoint_id":"None","disappear_time_verified":false,"form":0,"individual_defense":null,"first_seen":1564175718,"pokestop_id":"34eadde33a9a412e9cf74a6137b38d48.16","encounter_id":"1432619712785063368","longitude":11.266252,"costume":0,"move_1":null,"disappear_time":1564176918,"move_2":null,"weather":0,"pokemon_id":109,"height":null}"#
            ).unwrap()), Local::now())).unwrap();
    }

    #[test]
    fn raid_image_egg() {
        env_logger::try_init().ok();

        block_on_all(message::prepare(RaidMessage::get_dummy(serde_json::from_str(
                r#"{"gym_name":"Cesena - Stadio Dino Manuzzi","gym_url":"http://lh3.googleusercontent.com/EDFoOOm1G0hhjtBdJmBUjcUAcfB1se0zPatRviNuHcsK2oxwIokSONXWb8CUAJYYHI0Cl32dWiVlIMXVZe_X","end":1564338495,"form":0,"is_exclusive":false,"longitude":12.262141,"cp":0,"team_id":1,"pokemon_id":0,"gym_id":"fe20dd37398341a4b83751c5c050aaec.16","move_2":0,"level":4,"move_1":0,"start":1564335795,"ex_raid_eligible":false,"spawn":1564332195,"latitude":44.139762}"#
            ).unwrap()), Local::now())).unwrap();
    }

    #[test]
    fn raid_image_raidboss() {
        env_logger::try_init().ok();

        block_on_all(message::prepare(RaidMessage::get_dummy(serde_json::from_str(
                r#"{"latitude":45.468203,"team_id":2,"start":1564212138,"end":1564214838,"spawn":1564208538,"cp":2527,"ex_raid_eligible":false,"form":63,"gym_id":"03213b28587c4063b81c6d9ed39e5f54.16","gym_name":"Parrocchia di San Pietro in Sala","longitude":9.156488,"move_1":202,"move_2":16,"is_exclusive":false,"gym_url":"http://lh5.ggpht.com/5sxTUTkH0Ch1l-w-jMN8i_2-wz1XVjcPv4EAHTBJunHONWr7KFIWHjnh_RfU0lqKPL4j12sSgQJKyKNqD3-p","level":1,"pokemon_id":52}"#
            ).unwrap()), Local::now())).unwrap();
    }

    #[test]
    fn invasion_image() {
        env_logger::try_init().ok();

        block_on_all(message::prepare(InvasionMessage::get_dummy(serde_json::from_str(
                r#"{"lure_expiration":0,"enabled":true,"updated":1564332327,"url":"http://lh6.ggpht.com/ga78DsEtufPUGu0H0oE2ZOeagwxe8aQ4k4-kBLEDdSfeFVshH8gHhQN1GMcw1OFd_n94NpwTkOa16zR5DqUL","pokestop_display":1,"longitude":11.236241,"lure_id":501,"last_modified":1564329955,"pokestop_id":"54e0ee4c0e7a42ca93d2e93ee720dc90.16","name":"Ancora Un'altra Madonnina.","incident_expire_timestamp":1564333601,"grunt_type":33,"latitude":44.723203}"#
            ).unwrap()), Local::now())).unwrap();
    }
}

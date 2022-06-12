use std::collections::HashMap;
use std::f64::consts::PI;

use serde::{Deserialize, Serialize};

use serde_json::Value as JsonValue;

use chrono::{DateTime, Utc};

use chrono_tz::Europe::Rome;

use mysql_async::{params, prelude::Queryable};

use geo::Point;

use geo_raycasting::RayCasting;

use tracing::{error, info};

use rocketmap_entities::{Gender, GymDetails, Pokemon, Pokestop, PvpRanking, Raid, Weather};

use crate::Platform;
// use crate::lists::COMMON;
use crate::db::MYSQL;
use crate::lists::{CITIES, FORMS, LIST};
// use crate::telegram::Image;

use super::{
    message::{
        GymMessage, InvasionMessage, LureMessage, Message, PokemonMessage, RaidMessage,
        WeatherMessage,
    },
    Request, WATCHES,
};

const MAX_DISTANCE: f64 = 15f64;
// const MIN_IV_LIMIT: f32 = 36f32;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotConfig {
    pub user_id: Option<String>,
    pub scadenza: Option<i64>,
    pub debug: Option<bool>,
    pub locs: BotLocs,
    pub raid: BotRaid,
    pub pkmn: BotPkmn,
    pub time: BotTime,
    pub qest: Option<BotQest>,
    pub lure: Option<BotPkst>,
    pub invs: Option<BotPkst>,
    pub more: BotMore,
}

impl BotConfig {
    pub async fn validate(&self, user_id: &str, city_id: u16) -> Result<bool, ()> {
        let cities = CITIES.load();
        let polygon = match cities.get(&city_id) {
            Some(c) => c.coordinates.clone(),
            None => {
                info!("{} is associated to disabled city {}", user_id, city_id);
                return Ok(false);
            }
        };

        // if let (Ok(x), Ok(y)) = (BotLocs::convert_to_f64(&self.locs.h[0]), BotLocs::convert_to_f64(&self.locs.h[1])) {
        //     let p: Point<f64> = (x, y).into();
        //     if !polygon.within(&p) {
        //         info!("{} has home pointer out of city {}", user_id, city_id);
        //         return Ok(false);
        //     }
        // }

        if let (Ok(x), Ok(y)) = (
            BotLocs::convert_to_f64(&self.locs.p[0]),
            BotLocs::convert_to_f64(&self.locs.p[1]),
        ) {
            let p: Point<f64> = (x, y).into();
            if !polygon.within(&p) {
                info!("{} has pokemon pointer out of city {}", user_id, city_id);
                return Ok(false);
            }
        }

        if let (Ok(x), Ok(y)) = (
            BotLocs::convert_to_f64(&self.locs.r[0]),
            BotLocs::convert_to_f64(&self.locs.r[1]),
        ) {
            let p: Point<f64> = (x, y).into();
            if !polygon.within(&p) {
                info!("{} has raid pointer out of city {}", user_id, city_id);
                return Ok(false);
            }
        }

        if let Some(pos) = self.locs.i.as_ref() {
            if let (Ok(x), Ok(y)) = (
                BotLocs::convert_to_f64(&pos[0]),
                BotLocs::convert_to_f64(&pos[1]),
            ) {
                let p: Point<f64> = (x, y).into();
                if !polygon.within(&p) {
                    info!("{} has pokestop pointer out of city {}", user_id, city_id);
                    return Ok(false);
                }
            }
        }

        let now = Utc::now().timestamp();

        if BotLocs::convert_to_i64(&self.locs.t_p[2]).map(|i| i > now) == Ok(true) {
            if let (Ok(x), Ok(y)) = (
                BotLocs::convert_to_f64(&self.locs.t_p[0]),
                BotLocs::convert_to_f64(&self.locs.t_p[1]),
            ) {
                let p: Point<f64> = (x, y).into();
                let mut not_found = true;
                let mut city_id: u16 = 0;
                for (id, city) in cities.iter() {
                    if city.coordinates.within(&p) {
                        not_found = false;
                        city_id = *id;
                        break;
                    }
                }
                if not_found {
                    info!("{} has temp pokemon pointer out of any city", user_id);
                    return Ok(false);
                } else {
                    // update city_id on temp pos log
                    let mut conn = MYSQL
                        .get_conn()
                        .await
                        .map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
                    conn.exec_drop(
                        "UPDATE utenti_temp_pos SET city_id = :city_id WHERE user_id = :user_id AND pos_type IN ('a', 'p') AND start_time <= :now AND end_time > :now",
                        params! {
                            "city_id" => city_id,
                            "user_id" => user_id,
                            "now" => now,
                        }
                    ).await.map_err(|e| error!("MySQL query error: update pokemon temp pos\n{}", e))?;
                }
            }
        }

        if BotLocs::convert_to_i64(&self.locs.t_r[2]).map(|i| i > now) == Ok(true) {
            if let (Ok(x), Ok(y)) = (
                BotLocs::convert_to_f64(&self.locs.t_r[0]),
                BotLocs::convert_to_f64(&self.locs.t_r[1]),
            ) {
                let p: Point<f64> = (x, y).into();
                let mut not_found = true;
                let mut city_id: u16 = 0;
                for (id, city) in cities.iter() {
                    if city.coordinates.within(&p) {
                        not_found = false;
                        city_id = *id;
                        break;
                    }
                }
                if not_found {
                    info!("{} has temp raid pointer out of any city", user_id);
                    return Ok(false);
                } else {
                    // update city_id on temp pos log
                    let mut conn = MYSQL
                        .get_conn()
                        .await
                        .map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
                    conn.exec_drop(
                        "UPDATE utenti_temp_pos SET city_id = :city_id WHERE user_id = :user_id AND pos_type IN ('a', 'r') AND start_time <= :now AND end_time > :now",
                        params! {
                            "city_id" => city_id,
                            "user_id" => user_id,
                            "now" => now,
                        }
                    ).await.map_err(|e| error!("MySQL query error: update raid temp pos\n{}", e))?;
                }
            }
        }

        if let Some(pos) = self.locs.t_i.as_ref() {
            if BotLocs::convert_to_i64(&pos[2]).map(|i| i > now) == Ok(true) {
                if let (Ok(x), Ok(y)) = (
                    BotLocs::convert_to_f64(&pos[0]),
                    BotLocs::convert_to_f64(&pos[1]),
                ) {
                    let p: Point<f64> = (x, y).into();
                    let mut not_found = true;
                    let mut city_id: u16 = 0;
                    for (id, city) in cities.iter() {
                        if city.coordinates.within(&p) {
                            not_found = false;
                            city_id = *id;
                            break;
                        }
                    }
                    if not_found {
                        info!("{} has temp pokestop pointer out of any city", user_id);
                        return Ok(false);
                    } else {
                        // update city_id on temp pos log
                        let mut conn = MYSQL
                            .get_conn()
                            .await
                            .map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
                        conn.exec_drop(
                            "UPDATE utenti_temp_pos SET city_id = :city_id WHERE user_id = :user_id AND pos_type IN ('a', 'i') AND start_time <= :now AND end_time > :now",
                            params! {
                                "city_id" => city_id,
                                "user_id" => user_id,
                                "now" => now,
                            }
                        ).await.map_err(|e| error!("MySQL query error: update pokestop temp pos\n{}", e))?;
                    }
                }
            }
        }

        Ok(true)
    }

    pub async fn submit(
        &self,
        now: &DateTime<Utc>,
        platform: &Platform,
        input: &Request,
    ) -> Result<Box<dyn Message + Send + Sync>, ()> {
        if !self.time.is_active(now)? && self.time.fi[0] == 0 && self.time.fl[0] == 0 {
            #[cfg(test)]
            info!("Webhook discarded for time configs");

            Err(())
        } else {
            match input {
                Request::Pokemon(i) => Ok(Box::new(self.submit_pokemon(now, platform, i)?)),
                Request::Raid(i) => Ok(Box::new(self.submit_raid(now, platform, i)?)),
                Request::Pokestop(i) => {
                    // MAD workaround
                    if i.lure_id.is_none() && i.grunt_type.is_some() {
                        Ok(Box::new(self.submit_invasion(now, platform, i)?))
                    } else {
                        Ok(Box::new(self.submit_pokestop(now, platform, i)?))
                    }
                }
                Request::Invasion(i) => Ok(Box::new(self.submit_invasion(now, platform, i)?)),
                Request::GymDetails(i) => Ok(Box::new(self.submit_gym(now, platform, i)?)),
                Request::Weather(i) => Ok(Box::new(self.submit_weather(now, platform, i).await?)),
                _ => Err(()),
            }
        }
    }

    fn submit_pokemon(
        &self,
        now: &DateTime<Utc>,
        platform: &Platform,
        input: &Pokemon,
    ) -> Result<PokemonMessage, ()> {
        let loc = self.locs.get_pokemon_settings();
        let pos = (input.latitude, input.longitude);
        let iv = match (
            input.individual_attack,
            input.individual_defense,
            input.individual_stamina,
        ) {
            (Some(atk), Some(def), Some(sta)) => {
                Some(((f64::from(atk + def + sta) / 45_f64) * 100_f64).round() as u8)
            }
            _ => None,
        };
        let mut debug = format!(
            "Scansione avvenuta{} alle {}\n",
            platform,
            now.with_timezone(&Rome).format("%T").to_string()
        );

        if (self.pkmn.p1 == Some(1) && iv == Some(100))
            || (self.pkmn.p0 == Some(1) && iv == Some(0))
        {
            let rad = MAX_DISTANCE
                .min(BotLocs::convert_to_f64(
                    loc.get(3).unwrap_or_else(|| &self.locs.p[2]),
                )?)
                .max(0.1);
            let dist = BotLocs::calc_dist(loc, pos)?;
            if dist <= rad {
                debug.push_str(&format!("Bypass IV {:.0}%", iv.unwrap_or_default()));

                return Ok(PokemonMessage {
                    pokemon: input.clone(),
                    iv,
                    distance: BotLocs::calc_dist(&self.locs.h, pos)?,
                    direction: BotLocs::get_direction(&self.locs.h, pos)?,
                    debug: if self.debug == Some(true) {
                        Some(debug)
                    } else {
                        None
                    },
                });
            }
        }

        let pokemon_id = input.pokemon_id.to_string();
        let filter = self.pkmn.l.get(&pokemon_id).ok_or_else(|| {
            #[cfg(test)]
            info!("Pokémon not configured");
        })?;
        if filter.get(0) == Some(&0) {
            #[cfg(test)]
            info!("Pokémon disabled");

            return Err(());
        }

        let rad = if filter.get(5) == Some(&1) {
            // $pkmn_rad = ValMinMax($filter[6], 0.1, MAX_DISTANCE);
            let rad = MAX_DISTANCE
                .min(f64::from(
                    *(filter.get(6).ok_or_else(|| {
                        #[cfg(test)]
                        info!("Custom distance but no custom distance value");
                    })?),
                ))
                .max(0.1);
            debug.push_str(&format!(
                "Distanza personalizzata per Pokémon inferiore a {:.2} km",
                rad
            ));
            rad
        } else {
            // $pkmn_rad = ValMinMax($locs["p"][2], 0.1, MAX_DISTANCE);
            let rad = MAX_DISTANCE
                .min(BotLocs::convert_to_f64(
                    loc.get(3).unwrap_or_else(|| &self.locs.p[2]),
                )?)
                .max(0.1);
            debug.push_str(&format!(
                "Distanza standard per Pokémon inferiore a {:.2} km",
                rad
            ));
            rad
        };

        let dist = BotLocs::calc_dist(loc, pos)?;
        if dist > rad {
            #[cfg(test)]
            info!(
                "Pokémon discarded for distance: loc {:?} pos {:?} dist {} rad {}",
                loc, pos, dist, rad
            );

            return Err(());
        } else {
            debug.push_str(&format!(" ({:.2} km)", dist));
        }

        let badge = BotPkmn::check_badge(filter, input);

        // if !badge && COMMON.contains(&input.pokemon_id) {
        //     if let Some(i) = iv {
        //         if i < MIN_IV_LIMIT {
        //             #[cfg(test)]
        //             info!("Pokémon discarded because common and with low IV");

        //             return Err(());
        //         }
        //         else {
        //             debug.push_str(&format!("\nPokémon comune ma con IV superiori alla soglia del {:.0}% ({:.0}%)", MIN_IV_LIMIT, i));
        //         }
        //     }
        //     else {
        //         #[cfg(test)]
        //         info!("Pokémon discarded because common and without IV");

        //         return Err(());
        //     }
        // }

        if !self.time.is_active(now)? {
            if let Some(s) = self.time.bypass(iv, input.pokemon_level) {
                debug.push_str(&format!(
                    "\nFiltro orario non attivo ma eccezione per {}",
                    s
                ));
            } else {
                #[cfg(test)]
                info!(
                    "Pokémon discarded for time config: pokemon_id {} iv {:?} level {:?}",
                    pokemon_id, iv, input.pokemon_level
                );

                return Err(());
            }
        } else if badge {
            debug.push_str("\nEccezione per medaglia");
        } else if let Some(s) = BotPkmn::filter(filter, iv.as_ref(), input.pokemon_level.as_ref()) {
            debug.push_str(&format!("\nFiltro orario attivo e {}", s));
        } else {
            #[cfg(test)]
            info!(
                "Pokémon discarded for IV-Level config: pokemon_id {} iv {:?} level {:?}",
                pokemon_id, iv, input.pokemon_level
            );

            return Err(());
        }

        if !badge {
            if let Some(dbg) = BotPkmn::advanced_filters(filter, input) {
                debug.push_str(&dbg);
            } else {
                return Err(());
            }
        }

        Ok(PokemonMessage {
            pokemon: input.clone(),
            iv,
            distance: BotLocs::calc_dist(&self.locs.h, pos)?,
            direction: BotLocs::get_direction(&self.locs.h, pos)?,
            debug: if self.debug == Some(true) {
                Some(debug)
            } else {
                None
            },
        })
    }

    fn submit_raid(
        &self,
        now: &DateTime<Utc>,
        platform: &Platform,
        input: &Raid,
    ) -> Result<RaidMessage, ()> {
        let pokemon_id =
            input
                .pokemon_id
                .and_then(|id| if id > 0 { Some(id.to_string()) } else { None });
        let loc = self.locs.get_raid_settings();
        let pos = (input.latitude, input.longitude);
        if self.raid.x != Some(1) || input.ex_raid_eligible != Some(true) {
            if self.raid.s == 0 && pokemon_id.is_some() {
                #[cfg(test)]
                info!("Raid discarded for disabled raids");

                return Err(());
            }
            if self.raid.u == 0 && pokemon_id.is_none() {
                #[cfg(test)]
                info!("Raid discarded for disabled eggs");

                return Err(());
            }
        }

        // $raid_rad = ValMinMax($locs["r"][2], 0.1, MAX_DISTANCE);
        let rad = MAX_DISTANCE
            .min(BotLocs::convert_to_f64(
                loc.get(3).unwrap_or_else(|| &self.locs.r[2]),
            )?)
            .max(0.1);

        let mut debug = format!(
            "Scansione avvenuta{} alle {}\n",
            platform,
            now.with_timezone(&Rome).format("%T").to_string()
        );
        let dist = BotLocs::calc_dist(loc, pos)?;
        if dist > rad {
            #[cfg(test)]
            info!(
                "Raid discarded for distance: loc {:?} pos {:?} dist {} rad {}",
                loc, pos, dist, rad
            );

            return Err(());
        } else {
            debug.push_str(&format!(
                "Distanza per Raid inferiore a {:.2} km ({:.2} km)",
                rad, dist
            ));
        }

        if !self.time.is_active(now)? {
            #[cfg(test)]
            info!("Raid discarded for time config");

            return Err(());
        }

        if self.raid.x == Some(1) && input.ex_raid_eligible == Some(true) {
            debug.push_str("\nBypass Palestre EX abilitato");
        } else {
            match input.pokemon_id {
                Some(pkmn_id) if pkmn_id > 0 => {
                    if !self.raid.p.contains(&(pkmn_id as i16))
                        && !self.raid.p.contains(&(-(input.level as i16)))
                    {
                        #[cfg(test)]
                        info!(
                            "Raid discarded for disabled raidboss: raidboss {} config {:?}",
                            pkmn_id, self.raid.p
                        );

                        return Err(());
                    } else {
                        debug.push_str("\nPokémon presente nella lista raidboss abilitati");
                    }
                }
                _ => {
                    if !self.raid.l.contains(&input.level) {
                        #[cfg(test)]
                        info!(
                            "Raid discarded for disabled egg level: level {} config {:?}",
                            input.level, self.raid.l
                        );

                        return Err(());
                    } else {
                        debug.push_str("\nLivello uovo abilitato");
                    }
                }
            }
        }

        Ok(RaidMessage {
            raid: input.clone(),
            distance: BotLocs::calc_dist(&self.locs.h, pos)?,
            debug: if self.debug == Some(true) {
                Some(debug)
            } else {
                None
            },
        })
    }

    fn submit_pokestop(
        &self,
        now: &DateTime<Utc>,
        platform: &Platform,
        input: &Pokestop,
    ) -> Result<LureMessage, ()> {
        let lure = self.lure.as_ref().ok_or(())?;
        if lure.n == 0
            || input.lure_id.unwrap_or_default() == 0
            || input.lure_expiration <= Some(now.timestamp())
        {
            return Err(());
        }

        let loc = self.locs.get_invs_settings()?;
        let pos = (input.latitude, input.longitude);

        let rad = MAX_DISTANCE
            .min(BotLocs::convert_to_f64(
                loc.get(3).unwrap_or_else(|| &loc[2]),
            )?)
            .max(0.1);

        let mut debug = format!(
            "Scansione avvenuta{} alle {}\n",
            platform,
            now.with_timezone(&Rome).format("%T").to_string()
        );
        let dist = BotLocs::calc_dist(loc, pos)?;
        if dist > rad {
            return Err(());
        } else {
            debug.push_str(&format!(
                "Distanza per Pokéstop inferiore a {:.2} km ({:.2} km)",
                rad, dist
            ));
        }

        if lure.f == 1 {
            if let Some(lure_id) = input.lure_id {
                if !lure.l.contains(&((lure_id - 500) as u16)) {
                    return Err(());
                } else {
                    debug.push_str("\nEsca presente nella lista delle esche abilitate");
                }
            }
        } else {
            debug.push_str("\nNessun filtro esche attivo");
        }

        Ok(LureMessage {
            pokestop: input.clone(),
            debug: if self.debug == Some(true) {
                Some(debug)
            } else {
                None
            },
        })
    }

    fn submit_invasion(
        &self,
        now: &DateTime<Utc>,
        platform: &Platform,
        input: &Pokestop,
    ) -> Result<InvasionMessage, ()> {
        let invs = self.invs.as_ref().ok_or(())?;
        if invs.n == 0 {
            return Err(());
        }

        let loc = self.locs.get_invs_settings()?;
        let pos = (input.latitude, input.longitude);

        let rad = MAX_DISTANCE
            .min(BotLocs::convert_to_f64(
                loc.get(3).unwrap_or_else(|| &loc[2]),
            )?)
            .max(0.1);

        let mut debug = format!(
            "Scansione avvenuta{} alle {}\n",
            platform,
            now.with_timezone(&Rome).format("%T").to_string()
        );
        let dist = BotLocs::calc_dist(loc, pos)?;
        if dist > rad {
            return Err(());
        } else {
            debug.push_str(&format!(
                "Distanza per Pokéstop inferiore a {:.2} km ({:.2} km)",
                rad, dist
            ));
        }

        if invs.f == 1 {
            if !invs.l.contains(input.grunt_type.as_ref().ok_or(())?) {
                return Err(());
            } else {
                debug.push_str("\nScagnozzo presente nella lista degli scagnozzi abilitati");
            }
        } else {
            debug.push_str("\nNessun filtro scagnozzi attivo");
        }

        Ok(InvasionMessage {
            invasion: input.clone(),
            debug: if self.debug == Some(true) {
                Some(debug)
            } else {
                None
            },
        })
    }

    fn submit_gym(
        &self,
        now: &DateTime<Utc>,
        platform: &Platform,
        input: &GymDetails,
    ) -> Result<GymMessage, ()> {
        if self.raid.c != Some(1) {
            return Err(());
        }

        let loc = self.locs.get_raid_settings();
        let pos = (input.latitude, input.longitude);

        let rad = MAX_DISTANCE
            .min(
                // here we have an optional override that remains even with temp position
                if self.locs.r.get(3).map(BotLocs::convert_to_i64) == Some(Ok(1)) {
                    self.locs.r.get(4).map(BotLocs::convert_to_f64).transpose()?.unwrap_or_default()
                }
                else {
                    BotLocs::convert_to_f64(loc.get(3).unwrap_or_else(|| &self.locs.r[2]))?
                }
            )
            .max(0.1);

        let mut debug = format!(
            "Scansione avvenuta{} alle {}\n",
            platform,
            now.with_timezone(&Rome).format("%T").to_string()
        );
        let dist = BotLocs::calc_dist(loc, pos)?;
        if dist > rad {
            #[cfg(test)]
            info!(
                "Gym discarded for distance: loc {:?} pos {:?} dist {} rad {}",
                loc, pos, dist, rad
            );

            return Err(());
        } else {
            debug.push_str(&format!(
                "Distanza per Palestre inferiore a {:.2} km ({:.2} km)",
                rad, dist
            ));
        }

        if !self.time.is_active(now)? {
            #[cfg(test)]
            info!("Gym discarded for time config");

            return Err(());
        }

        Ok(GymMessage {
            gym: input.clone(),
            distance: BotLocs::calc_dist(&self.locs.h, pos)?,
            debug: if self.debug == Some(true) {
                Some(debug)
            } else {
                None
            },
        })
    }

    async fn submit_weather(
        &self,
        now: &DateTime<Utc>,
        _platform: &Platform,
        weather: &Weather,
    ) -> Result<WeatherMessage, ()> {
        if let Some(user_id) = self.user_id.as_deref() {
            let lock = WATCHES.read().await;
            if let Some(watches) = lock.get(user_id) {
                let timestamp = now.timestamp();
                let time = now.with_timezone(&Rome).format("%T").to_string();

                for watch in watches {
                    if watch.expire < timestamp {
                        continue;
                    }

                    if weather.polygon.within(&watch.point) {
                        return Ok(WeatherMessage {
                            watch: watch.clone(),
                            // actual_weather: weather.clone(),
                            debug: if self.debug == Some(true) {
                                Some(time)
                            } else {
                                None
                            },
                        });
                    }
                }
            }
        }
        Err(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotLocs {
    pub h: Vec<JsonValue>,
    pub p: Vec<JsonValue>,
    pub r: Vec<JsonValue>,
    pub i: Option<Vec<JsonValue>>,
    pub t_p: Vec<JsonValue>,
    pub t_r: Vec<JsonValue>,
    pub t_i: Option<Vec<JsonValue>>,
}

impl BotLocs {
    fn convert_to_i64(input: &JsonValue) -> Result<i64, ()> {
        match input {
            JsonValue::String(s) => {
                if s.is_empty() {
                    Err(())
                } else {
                    s.parse()
                        .map_err(|e| error!("json value convert_to_i64 error: {}", e))
                }
            }
            JsonValue::Number(n) => n.as_i64().ok_or_else(|| {
                error!("json value convert_to_i64 error: json element isn't an integer")
            }),
            _ => {
                error!(
                    "json value convert_to_i64 format not recognized: {:?}",
                    input
                );
                Err(())
            }
        }
    }

    fn convert_to_f64(input: &JsonValue) -> Result<f64, ()> {
        match input {
            JsonValue::String(s) => {
                if s.is_empty() {
                    #[cfg(test)]
                    info!("json value convert_to_f64 error: empty string");

                    Err(())
                } else {
                    s.parse()
                        .map_err(|e| error!("json value convert_to_f64 error: {}", e))
                }
            }
            JsonValue::Number(n) => n.as_f64().ok_or_else(|| {
                error!("json value convert_to_f64 error: json element isn't a float")
            }),
            _ => {
                error!(
                    "json value convert_to_f64 format not recognized: {:?}",
                    input
                );
                Err(())
            }
        }
    }

    fn get_pokemon_settings(&self) -> &Vec<JsonValue> {
        if Self::convert_to_i64(&self.t_p[2]).map(|i| i > Utc::now().timestamp()) == Ok(true) {
            &self.t_p
        } else {
            &self.p
        }
    }

    fn get_raid_settings(&self) -> &Vec<JsonValue> {
        if Self::convert_to_i64(&self.t_r[2]).map(|i| i > Utc::now().timestamp()) == Ok(true) {
            &self.t_r
        } else {
            &self.r
        }
    }

    fn get_invs_settings(&self) -> Result<&Vec<JsonValue>, ()> {
        match self.t_i {
            Some(ref t_i)
                if !t_i[2].is_null()
                    && Self::convert_to_i64(&t_i[2]).map(|i| i > Utc::now().timestamp())
                        == Ok(true) =>
            {
                Ok(t_i)
            }
            _ => self.i.as_ref().ok_or(()),
        }
    }

    fn calc_dist(here: &[JsonValue], pos: (f64, f64)) -> Result<f64, ()> {
        // $latFrom = deg2rad(floatval($here[0]));
        let lat_from = Self::convert_to_f64(&here[0])?.to_radians();
        // $lonFrom = deg2rad(floatval($here[1]));
        let lon_from = Self::convert_to_f64(&here[1])?.to_radians();
        // $latTo = deg2rad($toLat);
        let lat_to = pos.0.to_radians();
        // $lonTo = deg2rad($toLon);
        let lon_to = pos.1.to_radians();

        // $latDelta = $latTo - $latFrom;
        let lat_delta = lat_to - lat_from;
        // $lonDelta = $lonTo - $lonFrom;
        let lon_delta = lon_to - lon_from;

        // $angle = 2 * asin(sqrt(pow(sin($latDelta / 2), 2) + cos($latFrom) * cos($latTo) * pow(sin($lonDelta / 2), 2)));
        let angle = 2f64
            * ((lat_delta / 2f64).sin().powi(2)
                + lat_from.cos() * lat_to.cos() * (lon_delta / 2f64).sin().powi(2))
            .sqrt()
            .asin();
        // return intval($angle * 63710) / 10;
        Ok(angle * 6371f64)
    }

    fn get_direction(here: &[JsonValue], pos: (f64, f64)) -> Result<String, ()> {
        // $lat1 = floatval($here[0]);
        let lat1 = Self::convert_to_f64(&here[0])?;
        // $lon1 = floatval($here[1]);
        let lon1 = Self::convert_to_f64(&here[1])?;

        // $dLon = deg2rad($lon2) - deg2rad($lon1);
        let mut d_lon = pos.1.to_radians() - lon1.to_radians();
        // $dPhi = log(tan(deg2rad($lat2) / 2 + pi() / 4) / tan(deg2rad($lat1) / 2 + pi() / 4));
        let d_phi = ((pos.0.to_radians() / 2f64 + PI / 4f64).tan()
            / (lat1.to_radians() / 2f64 + PI / 4f64).tan())
        .ln();

        if d_lon.abs() > PI {
            if d_lon > 0f64 {
                // $dLon = (2 * pi() - $dLon) * -1;
                d_lon = (2f64 * PI - d_lon) * -1f64;
            } else {
                // $dLon = 2 * pi() + $dLon;
                d_lon += 2f64 * PI;
            }
        }

        // $tmp = round(((rad2deg(atan2($dLon, $dPhi)) + 360) % 360) / 45);
        let tmp = (((d_lon.atan2(d_phi).to_degrees() + 360f64) % 360f64) / 45f64).round() as usize;
        match tmp {
            1 => String::from_utf8(vec![0xe2, 0x86, 0x97, 0xef, 0xb8, 0x8f]),
            2 => String::from_utf8(vec![0xe2, 0x9e, 0xa1, 0xef, 0xb8, 0x8f]),
            3 => String::from_utf8(vec![0xe2, 0x86, 0x98, 0xef, 0xb8, 0x8f]),
            4 => String::from_utf8(vec![0xe2, 0xac, 0x87, 0xef, 0xb8, 0x8f]),
            5 => String::from_utf8(vec![0xe2, 0x86, 0x99, 0xef, 0xb8, 0x8f]),
            6 => String::from_utf8(vec![0xe2, 0xac, 0x85, 0xef, 0xb8, 0x8f]),
            7 => String::from_utf8(vec![0xe2, 0x86, 0x96, 0xef, 0xb8, 0x8f]),
            _ => String::from_utf8(vec![0xe2, 0xac, 0x86, 0xef, 0xb8, 0x8f]),
        }
        .map_err(|e| error!("direction gliph error: {}", e))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotRaid {
    pub c: Option<u8>,
    pub u: u8,
    pub s: u8,
    pub l: Vec<u8>,
    pub p: Vec<i16>,
    pub x: Option<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotPkmn {
    pub p1: Option<u8>,
    pub p0: Option<u8>,
    #[serde(deserialize_with = "deserialize_list")]
    pub l: HashMap<String, Vec<u8>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PkmnList {
    Map(HashMap<String, Vec<u8>>),
    Unknown(JsonValue),
}

fn deserialize_list<'de, D>(deserializer: D) -> Result<HashMap<String, Vec<u8>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match PkmnList::deserialize(deserializer)? {
        PkmnList::Map(m) => Ok(m),
        _ => Ok(HashMap::new()), //fallback for malformed lists
    }
}

impl BotPkmn {
    /**
     * [1, 1, 100, 0, 25, 0, 10, 1]
     * 0: active
     * 1: IV
     * 2: IV_min
     * 3: LVL
     * 4: LVL_min
     * 5: rad
     * 6: custom_rad
     * 7: or/and
     * 8: badge
     * 9: gender (1: Male, 2: Female)
     * 10: atk filter (1: <, 2: =, 3: >)
     * 11: atk value
     * 12: def filter (1: <, 2: =, 3: >)
     * 13: def value
     * 14: sta filter (1: <, 2: =, 3: >)
     * 15: sta value
     * 16: bypass 100%
     * 17: form (first byte)
     * 18: form (second byte)
     * 19: mega check
     * 20: Mega
     * 21: ultra check
     * 22: Ultra
     */
    fn filter(filter: &[u8], iv: Option<&u8>, lvl: Option<&u8>) -> Option<String> {
        if filter.get(1) >= Some(&1) && filter.get(3) == Some(&1) {
            // IV e PL attivi
            if filter.get(7) == Some(&1) {
                if iv >= filter.get(2) || lvl >= filter.get(4) {
                    return Some(format!(
                        "IV >= {} O LVL >= {}",
                        filter.get(2).unwrap_or(&0),
                        filter.get(4).unwrap_or(&0)
                    ));
                }
            } else if iv >= filter.get(2) && lvl >= filter.get(4) {
                return Some(format!(
                    "IV >= {} E LVL >= {}",
                    filter.get(2).unwrap_or(&0),
                    filter.get(4).unwrap_or(&0)
                ));
            }
            None
        } else if filter.get(1) >= Some(&1) || filter.get(3) == Some(&1) {
            // IV o PL attivi
            if filter.get(1) >= Some(&1) && iv >= filter.get(2) {
                return Some(format!("IV >= {}", filter.get(2).unwrap_or(&0)));
            }
            if filter.get(3) == Some(&1) && lvl >= filter.get(4) {
                return Some(format!("LVL >= {}", filter.get(4).unwrap_or(&0)));
            }
            None
        } else {
            Some(String::from("nessun filtro IV/LVL attivo"))
        }
    }

    /**
     * [1, 1, 100, 0, 25, 0, 10, 1]
     * 0: active
     * 1: IV
     * 2: IV_min
     * 3: LVL
     * 4: LVL_min
     * 5: rad
     * 6: custom_rad
     * 7: or/and
     * 8: badge
     * 9: gender (1: Male, 2: Female)
     * 10: atk filter (1: <, 2: =, 3: >)
     * 11: atk value
     * 12: def filter (1: <, 2: =, 3: >)
     * 13: def value
     * 14: sta filter (1: <, 2: =, 3: >)
     * 15: sta value
     * 16: bypass 100%
     * 17: form (first byte)
     * 18: form (second byte)
     * 19: mega check
     * 20: Mega
     * 21: ultra check
     * 22: Ultra
     */
    fn check_badge(filter: &[u8], input: &Pokemon) -> bool {
        if filter.get(8) == Some(&1) {
            match input.pokemon_id {
                // rattata
                19 => {
                    match (input.height, input.weight) {
                        // https://www.reddit.com/r/TheSilphRoad/comments/5931wy/youngster_medal_you/
                        (Some(h), Some(w)) => (h / 0.3f64 + w / 3.5f64) < 1.5f64 && w < 2.41,
                        _ => false,
                    }
                }
                // magikarp
                129 => {
                    match (input.height, input.weight) {
                        // https://www.reddit.com/r/TheSilphRoad/comments/5nuipx/fisherman_medal_weights/
                        (Some(h), Some(w)) => (h / 0.9f64 + w / 10f64) > 2.5f64 && w >= 13.13,
                        _ => false,
                    }
                }
                _ => false,
            }
        } else {
            false
        }
    }

    /**
     * [1, 1, 100, 0, 25, 0, 10, 1]
     * 0: active
     * 1: IV
     * 2: IV_min
     * 3: LVL
     * 4: LVL_min
     * 5: rad
     * 6: custom_rad
     * 7: or/and
     * 8: badge
     * 9: gender (1: Male, 2: Female)
     * 10: atk filter (1: <, 2: =, 3: >)
     * 11: atk value
     * 12: def filter (1: <, 2: =, 3: >)
     * 13: def value
     * 14: sta filter (1: <, 2: =, 3: >)
     * 15: sta value
     * 16: bypass 100%
     * 17: form (first byte)
     * 18: form (second byte)
     * 19: mega check
     * 20: Mega
     * 21: ultra check
     * 22: Ultra
     */
    fn advanced_filters(filter: &[u8], input: &Pokemon) -> Option<String> {
        if filter.get(16) == Some(&1)
            && input.individual_attack == Some(15)
            && input.individual_defense == Some(15)
            && input.individual_stamina == Some(15)
        {
            return Some(String::from("\nFiltro avanzato: 100%"));
        }

        let mut dbg = String::new();

        match filter.get(9) {
            Some(&1) => {
                if input.gender != Gender::Male {
                    #[cfg(test)]
                    info!("Pokémon discarded for Advanced Filters config: isn't male");

                    return None;
                } else {
                    dbg.push_str("\nFiltro avanzato: Sesso maschio");
                }
            }
            Some(&2) => {
                if input.gender != Gender::Female {
                    #[cfg(test)]
                    info!("Pokémon discarded for Advanced Filters config: isn't female");

                    return None;
                } else {
                    dbg.push_str("\nFiltro avanzato: Sesso femmina");
                }
            }
            _ => {}
        }

        if let Some(f) = filter.get(17) {
            let mut f = *f as u16;
            if let Some(i) = filter.get(18) {
                f += (*i as u16) * 255;
            }
            if f > 0 {
                if Some(f) != input.form {
                    #[cfg(test)]
                    info!("Pokémon discarded for Advanced Filters config: wrong form");

                    return None;
                } else {
                    let forms = FORMS.load();
                    dbg.push_str(&format!(
                        "\nFiltro avanzato: Forma {}",
                        forms
                            .get(&f)
                            .map(|f| f.name.as_str())
                            .unwrap_or_else(|| "<sconosciuta>")
                    ));
                }
            }
        }

        fn filter_rank<'a>(
            check: Option<&u8>,
            filter: Option<&u8>,
            pvp: Option<&'a Vec<PvpRanking>>,
        ) -> Option<Option<&'a PvpRanking>> {
            match check {
                Some(&1) => {
                    if let Some(perf) = filter {
                        if let Some(ranks) = pvp {
                            let perf = (*perf as f64) / 100_f64;
                            for rank in ranks {
                                if rank.percentage.map(|p| p >= perf) == Some(true) {
                                    return Some(Some(rank));
                                }
                            }
                        }
                    }
                }
                Some(&2) => {
                    if let Some(perf) = filter {
                        if let Some(ranks) = pvp {
                            let perf = *perf as u16;
                            for rank in ranks {
                                if rank.rank.map(|r| r <= perf) == Some(true) {
                                    return Some(Some(rank));
                                }
                            }
                        }
                    }
                }
                _ => return None,
            }

            Some(None)
        }

        fn rank_to_string(r: &PvpRanking) -> String {
            let mut res = String::new();
            {
                let list = LIST.load();
                res.push_str(&format!(
                    " pokémon {}",
                    list.get(&r.pokemon)
                        .map(|s| s.name.as_str())
                        .unwrap_or_else(|| "<sconosciuto>")
                ));
            }
            if let Some(v) = &r.form {
                if v > &0 {
                    let forms = FORMS.load();
                    res.push_str(&format!(
                        " forma {}",
                        forms
                            .get(v)
                            .map(|f| f.name.as_str())
                            .unwrap_or_else(|| "<sconosciuta>")
                    ));
                }
            }
            if let Some(v) = &r.cp {
                res.push_str(&format!(" ps {}", v));
            }
            if let Some(v) = &r.level {
                res.push_str(&format!(" livello {}", v));
            }
            if let Some(v) = &r.rank {
                res.push_str(&format!(" rank {}", v));
            }
            if let Some(v) = &r.percentage {
                res.push_str(&format!(" percentuale {:.1}%", v * 100_f64));
            }
            res
        }

        #[allow(clippy::too_many_arguments)]
        fn filter_iv(
            atkf: Option<&u8>,
            atkv: Option<&u8>,
            deff: Option<&u8>,
            defv: Option<&u8>,
            staf: Option<&u8>,
            stav: Option<&u8>,
            atk: Option<&u8>,
            def: Option<&u8>,
            sta: Option<&u8>,
        ) -> Option<Option<String>> {
            let mut res = String::new();
            match atkf {
                Some(&1) => {
                    if atkv > atk {
                        res.push_str(&format!(
                            " ATK {} < {}",
                            atkv.unwrap_or(&0),
                            atk.unwrap_or(&0)
                        ));
                    } else {
                        return Some(None);
                    }
                }
                Some(&2) => {
                    if atkv == atk {
                        res.push_str(&format!(
                            " ATK {} = {}",
                            atkv.unwrap_or(&0),
                            atk.unwrap_or(&0)
                        ));
                    } else {
                        return Some(None);
                    }
                }
                Some(&3) => {
                    if atkv < atk {
                        res.push_str(&format!(
                            " ATK {} > {}",
                            atkv.unwrap_or(&0),
                            atk.unwrap_or(&0)
                        ));
                    } else {
                        return Some(None);
                    }
                }
                _ => {}
            }
            match deff {
                Some(&1) => {
                    if defv > def {
                        res.push_str(&format!(
                            " DEF {} < {}",
                            defv.unwrap_or(&0),
                            def.unwrap_or(&0)
                        ));
                    } else {
                        return Some(None);
                    }
                }
                Some(&2) => {
                    if defv == def {
                        res.push_str(&format!(
                            " DEF {} = {}",
                            defv.unwrap_or(&0),
                            def.unwrap_or(&0)
                        ));
                    } else {
                        return Some(None);
                    }
                }
                Some(&3) => {
                    if defv < def {
                        res.push_str(&format!(
                            " DEF {} > {}",
                            defv.unwrap_or(&0),
                            def.unwrap_or(&0)
                        ));
                    } else {
                        return Some(None);
                    }
                }
                _ => {}
            }
            match staf {
                Some(&1) => {
                    if stav > sta {
                        res.push_str(&format!(
                            " STA {} < {}",
                            stav.unwrap_or(&0),
                            sta.unwrap_or(&0)
                        ));
                    } else {
                        return Some(None);
                    }
                }
                Some(&2) => {
                    if stav == sta {
                        res.push_str(&format!(
                            " STA {} = {}",
                            stav.unwrap_or(&0),
                            sta.unwrap_or(&0)
                        ));
                    } else {
                        return Some(None);
                    }
                }
                Some(&3) => {
                    if stav < sta {
                        res.push_str(&format!(
                            " STA {} > {}",
                            stav.unwrap_or(&0),
                            sta.unwrap_or(&0)
                        ));
                    } else {
                        return Some(None);
                    }
                }
                _ => {}
            }
            if res.is_empty() {
                None
            } else {
                Some(Some(res))
            }
        }

        // filters are in OR condition
        // None => not checked
        // Some(None) => check failed
        // Some(Some(s)) => check passed
        match (
            filter_rank(
                filter.get(19),
                filter.get(20),
                input.pvp_rankings_great_league.as_ref(),
            ),
            filter_rank(
                filter.get(21),
                filter.get(22),
                input.pvp_rankings_ultra_league.as_ref(),
            ),
            filter_iv(
                filter.get(10),
                filter.get(11),
                filter.get(12),
                filter.get(13),
                filter.get(14),
                filter.get(15),
                input.individual_attack.as_ref(),
                input.individual_defense.as_ref(),
                input.individual_stamina.as_ref(),
            ),
        ) {
            (Some(Some(mega)), Some(Some(ultra)), Some(Some(s))) => {
                dbg.push_str(&format!(
                    "\nFiltro avanzato: Mega{}\nFiltro avanzato: Ultra{}\nFiltro avanzato: IV{}",
                    rank_to_string(mega),
                    rank_to_string(ultra),
                    s,
                ));
            }
            (Some(Some(mega)), Some(Some(ultra)), _) => {
                dbg.push_str(&format!(
                    "\nFiltro avanzato: Mega{}\nFiltro avanzato: Ultra{}",
                    rank_to_string(mega),
                    rank_to_string(ultra),
                ));
            }
            (Some(Some(mega)), _, Some(Some(s))) => {
                dbg.push_str(&format!(
                    "\nFiltro avanzato: Mega{}\nFiltro avanzato: IV{}",
                    rank_to_string(mega),
                    s,
                ));
            }
            (Some(Some(mega)), _, _) => {
                dbg.push_str(&format!("\nFiltro avanzato: Mega{}", rank_to_string(mega),));
            }
            (_, Some(Some(ultra)), Some(Some(s))) => {
                dbg.push_str(&format!(
                    "\nFiltro avanzato: Ultra{}\nFiltro avanzato: IV{}",
                    rank_to_string(ultra),
                    s,
                ));
            }
            (_, Some(Some(ultra)), _) => {
                dbg.push_str(&format!(
                    "\nFiltro avanzato: Ultra{}",
                    rank_to_string(ultra),
                ));
            }
            (_, _, Some(Some(s))) => {
                dbg.push_str(&format!("\nFiltro avanzato: IV{}", s));
            }
            (None, None, None) => {}
            (Some(None), _, _) | (_, Some(None), _) | (_, _, Some(None)) => {
                #[cfg(test)]
                info!("Pokémon discarded for Advanced Filters config");

                return None;
            }
        }

        Some(dbg)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotTime {
    pub fi: [u8; 2],
    pub fl: [u8; 2],
    pub fc: u8,
    pub ex: Option<u8>,
    pub w1: Vec<u8>,
    pub w2: Vec<u8>,
    pub w3: Option<Vec<u8>>,
    pub w4: Option<Vec<u8>>,
    pub w5: Option<Vec<u8>>,
    pub w6: Option<Vec<u8>>,
    pub w7: Option<Vec<u8>>,
}

impl BotTime {
    fn is_active(&self, now: &DateTime<Utc>) -> Result<bool, ()> {
        let hour: u8 = now
            .with_timezone(&Rome)
            .format("%H")
            .to_string()
            .parse()
            .map_err(|e| error!("current hour retrieve error: {}", e))?;
        let day = now.with_timezone(&Rome).format("%w").to_string();
        if self.ex == Some(1) {
            Ok(match day.as_str() {
                "1" => self.w1.contains(&hour),
                "2" => self.w2.contains(&hour),
                "3" => self.w3.as_ref().map(|v| v.contains(&hour)) == Some(true),
                "4" => self.w4.as_ref().map(|v| v.contains(&hour)) == Some(true),
                "5" => self.w5.as_ref().map(|v| v.contains(&hour)) == Some(true),
                "6" => self.w6.as_ref().map(|v| v.contains(&hour)) == Some(true),
                _ => self.w7.as_ref().map(|v| v.contains(&hour)) == Some(true),
            })
        } else {
            Ok(match day.as_str() {
                "0" | "6" => self.w2.contains(&hour),
                _ => self.w1.contains(&hour),
            })
        }
    }

    fn bypass(&self, iv: Option<u8>, lvl: Option<u8>) -> Option<String> {
        if self.fi[0] == 1 && self.fl[0] == 1 {
            if self.fc == 1 {
                if iv >= Some(self.fi[1]) || lvl >= Some(self.fl[1]) {
                    return Some(format!("IV >= {} O LVL >= {}", self.fi[1], self.fl[1]));
                }
                None
            } else {
                if iv >= Some(self.fi[1]) && lvl >= Some(self.fl[1]) {
                    return Some(format!("IV >= {} E LVL >= {}", self.fi[1], self.fl[1]));
                }
                None
            }
        } else {
            if self.fi[0] == 1 && iv >= Some(self.fi[1]) {
                return Some(format!("IV >= {}", self.fi[1]));
            }
            if self.fl[0] == 1 && lvl >= Some(self.fl[1]) {
                return Some(format!("LVL >= {}", self.fl[1]));
            }

            None
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotQest {
    pub n: u8,
    pub l: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotPkst {
    pub n: u8,
    pub f: u8,
    pub l: Vec<u16>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotMore {
    #[serde(default = "default_more_l")]
    pub l: String,
}

fn default_more_l() -> String {
    String::from("g")
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::BotConfig;

    use crate::Platform;

    use rocketmap_entities::{Pokemon, Pokestop, Raid};

    use super::Request;

    #[test]
    fn bot_config() {
        tracing_subscriber::fmt::try_init().ok();

        let data = [
            r#"{"locs":{"h":["45.653978","8.787760"],"p":["45.653968","8.787805","9"],"r":["45.655640","8.788785","1"],"t_p":["0","0","0"],"t_r":["0","0","0"]},"raid":{"u":1,"s":1,"l":[5],"p":[403,303,26,359,105,488]},"pkmn":{"l":{"1":[1,1,86,0,25,0,10,1],"2":[1,1,86,0,25,0,10,1],"3":[1,1,86,0,25,0,10,1],"4":[1,1,86,0,25,0,10,1],"5":[1,1,86,0,25,0,10,1],"6":[1,1,86,0,25,0,10,1],"7":[1,1,86,0,25,0,10,1],"8":[1,1,86,0,25,0,10,1],"9":[1,1,86,0,25,0,10,1],"10":[1,2,100,0,25,0,10,1],"11":[1,1,86,0,25,0,10,1],"12":[1,1,86,0,25,0,10,1],"13":[1,2,100,0,25,0,10,1],"14":[1,1,86,0,25,0,10,1],"15":[1,1,86,0,25,0,10,1],"16":[1,2,100,0,25,0,10,1],"17":[1,1,86,0,25,0,10,1],"18":[1,1,86,0,25,0,10,1],"19":[1,2,100,0,25,0,10,1],"20":[1,1,86,0,25,0,10,1],"21":[1,2,100,0,25,0,10,1],"22":[1,1,86,0,25,0,10,1],"23":[1,2,100,0,25,0,10,1],"24":[1,1,86,0,25,0,10,1],"25":[1,2,100,0,25,0,10,1],"26":[1,1,86,0,25,0,10,1],"27":[1,1,86,0,25,0,10,1],"28":[1,1,86,0,25,0,10,1],"29":[1,2,100,0,25,0,10,1],"30":[1,1,86,0,25,0,10,1],"31":[1,1,86,0,25,0,10,1],"32":[1,2,100,0,25,0,10,1],"33":[1,1,86,0,25,0,10,1],"34":[1,1,86,0,25,0,10,1],"35":[1,1,86,0,25,0,10,1],"36":[1,1,86,0,25,0,10,1],"37":[1,2,100,0,25,0,10,1],"38":[1,1,86,0,25,0,10,1],"39":[1,1,86,0,25,0,10,1],"40":[1,1,86,0,25,0,10,1],"41":[1,2,100,0,25,0,10,1],"42":[1,1,86,0,25,0,10,1],"43":[1,1,86,0,25,0,10,1],"44":[1,1,86,0,25,0,10,1],"45":[1,1,86,0,25,0,10,1],"46":[1,2,100,0,25,0,10,1],"47":[1,1,86,0,25,0,10,1],"48":[1,2,100,0,25,0,10,1],"49":[1,1,86,0,25,0,10,1],"50":[1,1,86,0,25,0,10,1],"51":[1,1,86,0,25,0,10,1],"52":[1,1,86,0,25,0,10,1],"53":[1,1,86,0,25,0,10,1],"54":[1,1,86,0,25,0,10,1],"55":[1,1,86,0,25,0,10,1],"56":[1,1,86,0,25,0,10,1],"57":[1,1,86,0,25,0,10,1],"58":[1,1,86,0,25,0,10,1],"59":[1,1,86,0,25,0,10,1],"60":[1,1,86,0,25,0,10,1],"61":[1,1,86,0,25,0,10,1],"62":[1,1,86,0,25,0,10,1],"63":[1,1,86,0,25,0,10,1],"64":[1,1,86,0,25,0,10,1],"65":[1,1,86,0,25,0,10,1],"66":[1,1,86,0,25,0,10,1],"67":[1,1,86,0,25,0,10,1],"68":[1,1,86,0,25,0,10,1],"69":[1,1,86,0,25,0,10,1],"70":[1,1,86,0,25,0,10,1],"71":[1,1,86,0,25,0,10,1],"72":[1,1,86,0,25,0,10,1],"73":[1,1,86,0,25,0,10,1],"74":[1,2,100,0,25,0,10,1],"75":[1,1,86,0,25,0,10,1],"76":[1,1,86,0,25,0,10,1],"77":[1,1,86,0,25,0,10,1],"78":[1,1,86,0,25,0,10,1],"79":[1,1,86,0,25,0,10,1],"80":[1,1,86,0,25,0,10,1],"81":[1,1,86,0,25,0,10,1],"82":[1,1,86,0,25,0,10,1],"83":[1,1,86,0,25,0,10,1],"84":[1,1,86,0,25,0,10,1],"85":[1,1,86,0,25,0,10,1],"86":[1,1,86,0,25,0,10,1],"87":[1,1,86,0,25,0,10,1],"88":[1,1,86,0,25,0,10,1],"89":[1,1,86,0,25,0,10,1],"90":[1,2,100,0,25,0,10,1],"91":[1,1,86,0,25,0,10,1],"92":[1,1,86,0,25,0,10,1],"93":[1,1,86,0,25,0,10,1],"94":[1,1,86,0,25,0,10,1],"95":[1,1,86,0,25,0,10,1],"96":[1,1,86,0,25,0,10,1],"97":[1,1,86,0,25,0,10,1],"98":[1,1,86,0,25,0,10,1],"99":[1,1,86,0,25,0,10,1],"100":[1,1,86,0,25,0,10,1],"101":[1,1,86,0,25,0,10,1],"102":[1,1,86,0,25,0,10,1],"103":[1,1,86,0,25,0,10,1],"104":[1,0,86,0,25,0,10,1],"105":[1,1,86,0,25,0,10,1],"106":[1,1,86,0,25,0,10,1],"107":[1,1,86,0,25,0,10,1],"108":[1,1,86,0,25,0,10,1],"109":[1,1,86,0,25,0,10,1],"110":[1,1,86,0,25,0,10,1],"111":[1,1,86,0,25,0,10,1],"112":[1,1,86,0,25,0,10,1],"113":[1,1,86,0,25,0,10,1],"114":[1,1,86,0,25,0,10,1],"115":[1,1,86,0,25,0,10,1],"116":[1,1,86,0,25,0,10,1],"117":[1,1,86,0,25,0,10,1],"118":[1,1,86,0,25,0,10,1],"119":[1,1,86,0,25,0,10,1],"120":[1,1,86,0,25,0,10,1],"121":[1,1,86,0,25,0,10,1],"122":[1,1,86,0,25,0,10,1],"123":[1,1,86,0,25,0,10,1],"124":[1,1,86,0,25,0,10,1],"125":[1,1,86,0,25,0,10,1],"126":[1,1,86,0,25,0,10,1],"127":[1,1,86,0,25,0,10,1],"128":[1,1,86,0,25,0,10,1],"129":[1,2,100,0,25,0,10,1],"130":[1,1,86,0,25,0,10,1],"131":[1,1,86,0,25,0,10,1],"132":[1,1,86,0,25,0,10,1],"133":[1,2,100,0,25,0,10,1],"134":[1,1,86,0,25,0,10,1],"135":[1,1,86,0,25,0,10,1],"136":[1,1,86,0,25,0,10,1],"137":[1,1,86,0,25,0,10,1],"138":[1,1,86,0,25,0,10,1],"139":[1,1,86,0,25,0,10,1],"140":[1,1,86,0,25,0,10,1],"141":[1,1,86,0,25,0,10,1],"142":[1,1,86,0,25,0,10,1],"143":[1,1,86,0,25,0,10,1],"147":[1,1,86,0,25,0,10,1],"148":[1,1,86,0,25,0,10,1],"149":[1,1,86,0,25,0,10,1],"152":[1,2,100,0,25,0,10,1],"153":[1,1,86,0,25,0,10,1],"154":[1,1,86,0,25,0,10,1],"155":[1,2,100,0,25,0,10,1],"156":[1,1,86,0,25,0,10,1],"157":[1,1,86,0,25,0,10,1],"158":[1,2,100,0,25,0,10,1],"159":[1,1,86,0,25,0,10,1],"160":[1,1,86,0,25,0,10,1],"161":[1,2,100,0,25,0,10,1],"162":[1,1,86,0,25,0,10,1],"163":[1,2,100,0,25,0,10,1],"164":[1,1,86,0,25,0,10,1],"165":[1,2,100,0,25,0,10,1],"166":[1,1,86,0,25,0,10,1],"167":[1,2,100,0,25,0,10,1],"168":[1,1,86,0,25,0,10,1],"169":[1,1,86,0,25,0,10,1],"170":[1,1,86,0,25,0,10,1],"171":[1,1,86,0,25,0,10,1],"176":[1,1,86,0,25,0,10,1],"177":[1,2,100,0,25,0,10,1],"178":[1,1,86,0,25,0,10,1],"179":[1,1,86,0,25,0,10,1],"180":[1,1,86,0,25,0,10,1],"181":[1,1,86,0,25,0,10,1],"183":[1,1,86,0,25,0,10,1],"184":[1,1,86,0,25,0,10,1],"185":[1,1,86,0,25,0,10,1],"187":[1,2,100,0,25,0,10,1],"188":[1,1,86,0,25,0,10,1],"189":[1,1,86,0,25,0,10,1],"190":[1,2,100,0,25,0,10,1],"191":[1,1,86,0,25,0,10,1],"193":[1,1,86,0,25,0,10,1],"194":[1,2,100,0,25,0,10,1],"195":[1,1,86,0,25,0,10,1],"198":[1,2,100,0,25,0,10,1],"200":[1,1,86,0,25,0,10,1],"201":[1,1,86,0,25,0,10,1],"202":[1,1,86,0,25,0,10,1],"203":[1,1,86,0,25,0,10,1],"204":[1,1,86,0,25,0,10,1],"205":[1,1,86,0,25,0,10,1],"206":[1,1,86,0,25,0,10,1],"207":[1,1,86,0,25,0,10,1],"209":[1,1,86,0,25,0,10,1],"210":[1,1,86,0,25,0,10,1],"211":[1,1,86,0,25,0,10,1],"213":[1,1,86,0,25,0,10,1],"214":[1,1,86,0,25,0,10,1],"215":[1,1,86,0,25,0,10,1],"216":[1,2,100,0,25,0,10,1],"217":[1,1,86,0,25,0,10,1],"218":[1,1,86,0,25,0,10,1],"219":[1,1,86,0,25,0,10,1],"220":[1,1,86,0,25,0,10,1],"221":[1,1,86,0,25,0,10,1],"222":[1,1,86,0,25,0,10,1],"223":[1,1,86,0,25,0,10,1],"224":[1,1,86,0,25,0,10,1],"225":[1,2,100,0,25,0,10,1],"226":[1,1,86,0,25,0,10,1],"227":[1,1,86,0,25,0,10,1],"228":[1,1,86,0,25,0,10,1],"229":[1,1,86,0,25,0,10,1],"231":[1,1,86,0,25,0,10,1],"232":[1,1,86,0,25,0,10,1],"233":[1,1,86,0,25,0,10,1],"234":[1,1,86,0,25,0,10,1],"237":[1,1,86,0,25,0,10,1],"241":[1,1,86,0,25,0,10,1],"242":[1,1,86,0,25,0,10,1],"246":[1,1,86,0,25,0,10,1],"247":[1,1,86,0,25,0,10,1],"248":[1,1,86,0,25,0,10,1],"252":[1,2,100,0,25,0,10,1],"253":[1,1,86,0,25,0,10,1],"254":[1,1,86,0,25,0,10,1],"255":[1,2,100,0,25,0,10,1],"256":[1,1,86,0,25,0,10,1],"257":[1,1,86,0,25,0,10,1],"258":[1,2,100,0,25,0,10,1],"259":[1,1,86,0,25,0,10,1],"260":[1,1,86,0,25,0,10,1],"261":[1,2,100,0,25,0,10,1],"262":[1,1,86,0,25,0,10,1],"263":[1,2,100,0,25,0,10,1],"264":[1,1,86,0,25,0,10,1],"265":[1,2,100,0,25,0,10,1],"266":[1,1,86,0,25,0,10,1],"267":[1,1,86,0,25,0,10,1],"268":[1,1,86,0,25,0,10,1],"269":[1,1,86,0,25,0,10,1],"270":[1,1,86,0,25,0,10,1],"271":[1,1,86,0,25,0,10,1],"272":[1,1,86,0,25,0,10,1],"273":[1,2,100,0,25,0,10,1],"274":[1,1,86,0,25,0,10,1],"275":[1,1,86,0,25,0,10,1],"276":[1,1,86,0,25,0,10,1],"277":[1,1,86,0,25,0,10,1],"278":[1,1,86,0,25,0,10,1],"279":[1,1,86,0,25,0,10,1],"280":[1,2,100,0,25,0,10,1],"281":[1,1,86,0,25,0,10,1],"282":[1,1,86,0,25,0,10,1],"283":[1,1,86,0,25,0,10,1],"284":[1,1,86,0,25,0,10,1],"285":[1,2,100,0,25,0,10,1],"286":[1,1,86,0,25,0,10,1],"287":[1,1,86,0,25,0,10,1],"288":[1,1,86,0,25,0,10,1],"289":[1,1,86,0,25,0,10,1],"290":[1,1,86,0,25,0,10,1],"291":[1,1,86,0,25,0,10,1],"292":[1,1,86,0,25,0,10,1],"293":[1,2,100,0,25,0,10,1],"294":[1,1,86,0,25,0,10,1],"295":[1,1,86,0,25,0,10,1],"296":[1,2,100,0,25,0,10,1],"297":[1,1,86,0,25,0,10,1],"298":[1,1,86,0,25,0,10,1],"299":[1,2,100,0,25,0,10,1],"300":[1,2,100,0,25,0,10,1],"301":[1,1,86,0,25,0,10,1],"302":[1,2,100,0,25,0,10,1],"303":[1,1,86,0,25,0,10,1],"304":[1,2,100,0,25,0,10,1],"305":[1,1,86,0,25,0,10,1],"306":[1,1,86,0,25,0,10,1],"307":[1,2,100,0,25,0,10,1],"308":[1,1,86,0,25,0,10,1],"309":[1,2,100,0,25,0,10,1],"310":[1,1,86,0,25,0,10,1],"311":[1,1,86,0,25,0,10,1],"312":[1,2,100,0,25,0,10,1],"313":[1,1,86,0,25,0,10,1],"314":[1,1,86,0,25,0,10,1],"315":[1,2,100,0,25,0,10,1],"316":[1,2,100,0,25,0,10,1],"317":[1,1,86,0,25,0,10,1],"318":[1,1,86,0,25,0,10,1],"319":[1,1,86,0,25,0,10,1],"320":[1,2,100,0,25,0,10,1],"321":[1,1,86,0,25,0,10,1],"322":[1,2,100,0,25,0,10,1],"323":[1,1,86,0,25,0,10,1],"324":[1,1,86,0,25,0,10,1],"325":[1,2,100,0,25,0,10,1],"326":[1,1,86,0,25,0,10,1],"327":[1,1,86,0,25,0,10,1],"328":[1,1,86,0,25,0,10,1],"329":[1,1,86,0,25,0,10,1],"330":[1,1,86,0,25,0,10,1],"331":[1,2,100,0,25,0,10,1],"332":[1,1,86,0,25,0,10,1],"333":[1,1,86,0,25,0,10,1],"334":[1,1,86,0,25,0,10,1],"335":[1,2,100,0,25,0,10,1],"336":[1,2,100,0,25,0,10,1],"337":[1,1,86,0,25,0,10,1],"338":[1,1,86,0,25,0,10,1],"339":[1,2,100,0,25,0,10,1],"340":[1,1,86,0,25,0,10,1],"341":[1,1,86,0,25,0,10,1],"342":[1,1,86,0,25,0,10,1],"343":[1,2,100,0,25,0,10,1],"344":[1,1,86,0,25,0,10,1],"345":[1,1,86,0,25,0,10,1],"346":[1,1,86,0,25,0,10,1],"347":[1,1,86,0,25,0,10,1],"348":[1,1,86,0,25,0,10,1],"349":[1,1,86,0,25,0,10,1],"350":[1,1,86,0,25,0,10,1],"351":[1,1,86,0,25,0,10,1],"352":[1,1,86,0,25,0,10,1],"353":[1,2,100,0,25,0,10,1],"354":[1,1,86,0,25,0,10,1],"355":[1,2,100,0,25,0,10,1],"356":[1,1,86,0,25,0,10,1],"357":[1,1,86,0,25,0,10,1],"358":[1,1,86,0,25,0,10,1],"359":[1,1,86,0,25,0,10,1],"360":[1,1,86,0,25,0,10,1],"361":[1,1,86,0,25,0,10,1],"362":[1,1,86,0,25,0,10,1],"363":[1,2,100,0,25,0,10,1],"364":[1,1,86,0,25,0,10,1],"365":[1,1,86,0,25,0,10,1],"366":[1,1,86,0,25,0,10,1],"367":[1,1,86,0,25,0,10,1],"368":[1,1,86,0,25,0,10,1],"369":[1,1,86,0,25,0,10,1],"370":[1,1,86,0,25,0,10,1],"371":[1,1,86,0,25,0,10,1],"372":[1,1,86,0,25,0,10,1],"373":[1,1,86,0,25,0,10,1],"374":[1,1,86,0,25,0,10,1],"375":[1,1,86,0,25,0,10,1],"376":[1,1,86,0,25,0,10,1],"387":[1,1,86,0,25,0,10,1],"388":[1,1,86,0,25,0,10,1],"389":[1,1,86,0,25,0,10,1],"390":[1,1,86,0,25,0,10,1],"391":[1,1,86,0,25,0,10,1],"392":[1,1,86,0,25,0,10,1],"393":[1,1,86,0,25,0,10,1],"394":[1,1,86,0,25,0,10,1],"395":[1,1,86,0,25,0,10,1],"396":[1,1,86,0,25,0,10,1],"397":[1,1,86,0,25,0,10,1],"398":[1,1,86,0,25,0,10,1],"399":[1,1,86,0,25,0,10,1],"400":[1,1,86,0,25,0,10,1],"401":[1,1,86,0,25,0,10,1],"402":[1,1,86,0,25,0,10,1],"403":[1,1,86,0,25,0,10,1],"404":[1,1,86,0,25,0,10,1],"405":[1,1,86,0,25,0,10,1],"406":[1,1,86,0,25,0,10,1],"407":[1,1,86,0,25,0,10,1],"408":[1,1,86,0,25,0,10,1],"409":[1,1,86,0,25,0,10,1],"410":[1,1,86,0,25,0,10,1],"411":[1,1,86,0,25,0,10,1],"412":[1,1,86,0,25,0,10,1],"413":[1,1,86,0,25,0,10,1],"414":[1,1,86,0,25,0,10,1],"415":[1,1,86,0,25,0,10,1],"416":[1,1,86,0,25,0,10,1],"417":[1,1,86,0,25,0,10,1],"418":[1,1,86,0,25,0,10,1],"419":[1,1,86,0,25,0,10,1],"420":[1,1,86,0,25,0,10,1],"421":[1,1,86,0,25,0,10,1],"422":[1,1,86,0,25,0,10,1],"423":[1,1,86,0,25,0,10,1],"424":[1,1,86,0,25,0,10,1],"425":[1,1,86,0,25,0,10,1],"426":[1,1,86,0,25,0,10,1],"427":[1,1,86,0,25,0,10,1],"428":[1,1,86,0,25,0,10,1],"429":[1,1,86,0,25,0,10,1],"430":[1,1,86,0,25,0,10,1],"431":[1,1,86,0,25,0,10,1],"432":[1,1,86,0,25,0,10,1],"433":[1,1,86,0,25,0,10,1],"434":[1,1,86,0,25,0,10,1],"435":[1,1,86,0,25,0,10,1],"436":[1,1,86,0,25,0,10,1],"437":[1,1,86,0,25,0,10,1],"438":[1,1,86,0,25,0,10,1],"439":[1,1,86,0,25,0,10,1],"440":[1,1,86,0,25,0,10,1],"441":[1,1,86,0,25,0,10,1],"442":[1,1,86,0,25,0,10,1],"443":[1,1,86,0,25,0,10,1],"444":[1,1,86,0,25,0,10,1],"445":[1,1,86,0,25,0,10,1],"446":[1,1,86,0,25,0,10,1],"447":[1,1,86,0,25,0,10,1],"448":[1,1,86,0,25,0,10,1],"449":[1,1,86,0,25,0,10,1],"450":[1,1,86,0,25,0,10,1],"451":[1,1,86,0,25,0,10,1],"452":[1,1,86,0,25,0,10,1],"453":[1,1,86,0,25,0,10,1],"454":[1,1,86,0,25,0,10,1],"455":[1,1,86,0,25,0,10,1],"456":[1,1,86,0,25,0,10,1],"457":[1,1,86,0,25,0,10,1],"458":[1,1,86,0,25,0,10,1],"459":[1,1,86,0,25,0,10,1],"460":[1,1,86,0,25,0,10,1],"461":[1,1,86,0,25,0,10,1],"462":[1,1,86,0,25,0,10,1],"463":[1,1,86,0,25,0,10,1],"464":[1,1,86,0,25,0,10,1],"465":[1,1,86,0,25,0,10,1],"466":[1,1,86,0,25,0,10,1],"467":[1,1,86,0,25,0,10,1],"468":[1,1,86,0,25,0,10,1],"469":[1,1,86,0,25,0,10,1],"470":[1,1,86,0,25,0,10,1],"471":[1,1,86,0,25,0,10,1],"472":[1,1,86,0,25,0,10,1],"473":[1,1,86,0,25,0,10,1],"474":[1,1,86,0,25,0,10,1],"475":[1,1,86,0,25,0,10,1],"476":[1,1,86,0,25,0,10,1],"477":[1,1,86,0,25,0,10,1],"478":[1,1,86,0,25,0,10,1],"479":[1,1,86,0,25,0,10,1],"480":[1,1,86,0,25,0,10,1],"481":[1,1,86,0,25,0,10,1],"482":[1,1,86,0,25,0,10,1],"483":[1,1,86,0,25,0,10,1],"484":[1,1,86,0,25,0,10,1],"485":[1,1,86,0,25,0,10,1],"486":[1,1,86,0,25,0,10,1],"487":[1,1,86,0,25,0,10,1],"488":[1,1,86,0,25,0,10,1],"489":[1,1,86,0,25,0,10,1],"490":[1,1,86,0,25,0,10,1],"491":[1,1,86,0,25,0,10,1],"492":[1,1,86,0,25,0,10,1],"493":[1,1,86,0,25,0,10,1]}},"time":{"fi":[0,80],"fl":[0,30],"fc":1,"w1":[0,1,17,18,19,20,21,22,23],"w2":[0,9,10,11,14,15,16,17,18,19,20,21,22,23]},"more":{"l":"g"}}"#,
            r#"{"locs":{"h":["45.557703","12.433947"],"p":["45.551693","12.359082","7"],"r":["45.552174","12.359382","7"],"i":["45.551903","12.359103","7"],"t_p":["0","0","0"],"t_r":["0","0","0"]},"raid":{"u":1,"s":1,"l":[4,5],"p":[149,176,210,131,105,384,386]},"pkmn":{"l":{"1":[0,0,80,0,25,0,10,1],"2":[0,0,80,0,25,0,10,1],"3":[0,0,80,0,25,0,10,1],"4":[0,0,80,0,25,0,10,1],"5":[0,0,80,0,25,0,10,1],"6":[0,0,80,0,25,0,10,1],"7":[0,0,80,0,25,0,10,1],"8":[0,0,80,0,25,0,10,1],"9":[0,0,80,0,25,0,10,1],"10":[0,2,100,0,25,0,10,1],"11":[0,0,80,0,25,0,10,1],"12":[0,0,80,0,25,0,10,1],"13":[0,2,100,0,25,0,10,1],"14":[0,0,80,0,25,0,10,1],"15":[0,0,80,0,25,0,10,1],"16":[0,2,100,0,25,0,10,1],"17":[0,0,80,0,25,0,10,1],"18":[0,0,80,0,25,0,10,1],"19":[0,2,100,0,25,0,10,1],"20":[0,0,80,0,25,0,10,1],"21":[0,2,100,0,25,0,10,1],"22":[0,0,80,0,25,0,10,1],"23":[0,2,100,0,25,0,10,1],"24":[0,0,80,0,25,0,10,1],"25":[0,2,100,0,25,0,10,1],"26":[0,0,80,0,25,0,10,1],"27":[0,0,80,0,25,0,10,1],"28":[0,0,80,0,25,0,10,1],"29":[0,2,100,0,25,0,10,1],"30":[0,0,80,0,25,0,10,1],"31":[0,0,80,0,25,0,10,1],"32":[0,2,100,0,25,0,10,1],"33":[0,0,80,0,25,0,10,1],"34":[0,0,80,0,25,0,10,1],"35":[0,0,80,0,25,0,10,1],"36":[0,0,80,0,25,0,10,1],"37":[0,2,100,0,25,0,10,1],"38":[0,0,80,0,25,0,10,1],"39":[0,0,80,0,25,0,10,1],"40":[0,0,80,0,25,0,10,1],"41":[0,2,100,0,25,0,10,1],"42":[0,0,80,0,25,0,10,1],"43":[0,0,80,0,25,0,10,1],"44":[0,0,80,0,25,0,10,1],"45":[0,0,80,0,25,0,10,1],"46":[0,2,100,0,25,0,10,1],"47":[0,0,80,0,25,0,10,1],"48":[0,2,100,0,25,0,10,1],"49":[0,0,80,0,25,0,10,1],"50":[0,0,80,0,25,0,10,1],"51":[0,0,80,0,25,0,10,1],"52":[0,0,80,0,25,0,10,1],"53":[0,0,80,0,25,0,10,1],"54":[0,0,80,0,25,0,10,1],"55":[0,0,80,0,25,0,10,1],"56":[0,0,80,0,25,0,10,1],"57":[0,0,80,0,25,0,10,1],"58":[0,0,80,0,25,0,10,1],"59":[0,0,80,0,25,0,10,1],"60":[0,0,80,0,25,0,10,1],"61":[0,0,80,0,25,0,10,1],"62":[0,0,80,0,25,0,10,1],"63":[0,0,80,0,25,0,10,1],"64":[0,0,80,0,25,0,10,1],"65":[0,0,80,0,25,0,10,1],"66":[0,0,80,0,25,0,10,1],"67":[0,0,80,0,25,0,10,1],"68":[0,0,80,0,25,0,10,1],"69":[0,0,80,0,25,0,10,1],"70":[0,0,80,0,25,0,10,1],"71":[0,0,80,0,25,0,10,1],"72":[0,0,80,0,25,0,10,1],"73":[0,0,80,0,25,0,10,1],"74":[0,2,100,0,25,0,10,1],"75":[0,0,80,0,25,0,10,1],"76":[0,0,80,0,25,0,10,1],"77":[0,0,80,0,25,0,10,1],"78":[0,0,80,0,25,0,10,1],"79":[0,0,80,0,25,0,10,1],"80":[0,0,80,0,25,0,10,1],"81":[0,0,80,0,25,0,10,1],"82":[0,0,80,0,25,0,10,1],"83":[0,0,80,0,25,0,10,1],"84":[0,0,80,0,25,0,10,1],"85":[0,0,80,0,25,0,10,1],"86":[0,0,80,0,25,0,10,1],"87":[0,0,80,0,25,0,10,1],"88":[0,0,80,0,25,0,10,1],"89":[0,0,80,0,25,0,10,1],"90":[0,2,100,0,25,0,10,1],"91":[0,0,80,0,25,0,10,1],"92":[0,0,80,0,25,0,10,1],"93":[0,0,80,0,25,0,10,1],"94":[0,0,80,0,25,0,10,1],"95":[0,0,80,0,25,0,10,1],"96":[0,0,80,0,25,0,10,1],"97":[0,0,80,0,25,0,10,1],"98":[0,0,80,0,25,0,10,1],"99":[0,0,80,0,25,0,10,1],"100":[0,0,80,0,25,0,10,1],"101":[0,0,80,0,25,0,10,1],"102":[0,0,80,0,25,0,10,1],"103":[0,0,80,0,25,0,10,1],"104":[0,0,80,0,25,0,10,1],"105":[0,0,80,0,25,0,10,1],"106":[0,0,80,0,25,0,10,1],"107":[0,0,80,0,25,0,10,1],"108":[0,0,80,0,25,0,10,1],"109":[0,0,80,0,25,0,10,1],"110":[0,0,80,0,25,0,10,1],"111":[0,0,80,0,25,0,10,1],"112":[0,0,80,0,25,0,10,1],"113":[0,0,80,0,25,0,10,1],"114":[0,0,80,0,25,0,10,1],"115":[0,0,80,0,25,0,10,1],"116":[0,0,80,0,25,0,10,1],"117":[0,0,80,0,25,0,10,1],"118":[0,0,80,0,25,0,10,1],"119":[0,0,80,0,25,0,10,1],"120":[0,0,80,0,25,0,10,1],"121":[0,0,80,0,25,0,10,1],"122":[0,0,80,0,25,0,10,1],"123":[0,0,80,0,25,0,10,1],"124":[0,0,80,0,25,0,10,1],"125":[0,0,80,0,25,0,10,1],"126":[0,0,80,0,25,0,10,1],"127":[0,0,80,0,25,0,10,1],"128":[0,0,80,0,25,0,10,1],"129":[0,2,100,0,25,0,10,1],"130":[0,0,80,0,25,0,10,1],"131":[0,0,80,0,25,0,10,1],"132":[0,0,80,0,25,0,10,1],"133":[0,2,100,0,25,0,10,1],"134":[0,0,80,0,25,0,10,1],"135":[0,0,80,0,25,0,10,1],"136":[0,0,80,0,25,0,10,1],"137":[0,0,80,0,25,0,10,1],"138":[0,0,80,0,25,0,10,1],"139":[0,0,80,0,25,0,10,1],"140":[0,0,80,0,25,0,10,1],"141":[0,0,80,0,25,0,10,1],"142":[0,0,80,0,25,0,10,1],"143":[0,0,80,0,25,0,10,1],"147":[0,0,80,0,25,0,10,1],"148":[0,0,80,0,25,0,10,1],"149":[0,0,80,0,25,0,10,1],"152":[0,2,100,0,25,0,10,1],"153":[0,0,80,0,25,0,10,1],"154":[0,0,80,0,25,0,10,1],"155":[0,2,100,0,25,0,10,1],"156":[0,0,80,0,25,0,10,1],"157":[0,0,80,0,25,0,10,1],"158":[0,2,100,0,25,0,10,1],"159":[0,0,80,0,25,0,10,1],"160":[0,0,80,0,25,0,10,1],"161":[0,2,100,0,25,0,10,1],"162":[0,0,80,0,25,0,10,1],"163":[0,2,100,0,25,0,10,1],"164":[0,0,80,0,25,0,10,1],"165":[0,2,100,0,25,0,10,1],"166":[0,0,80,0,25,0,10,1],"167":[0,2,100,0,25,0,10,1],"168":[0,0,80,0,25,0,10,1],"169":[0,0,80,0,25,0,10,1],"170":[0,0,80,0,25,0,10,1],"171":[0,0,80,0,25,0,10,1],"176":[0,0,80,0,25,0,10,1],"177":[0,2,100,0,25,0,10,1],"178":[0,0,80,0,25,0,10,1],"179":[0,0,80,0,25,0,10,1],"180":[0,0,80,0,25,0,10,1],"181":[0,0,80,0,25,0,10,1],"183":[0,0,80,0,25,0,10,1],"184":[0,0,80,0,25,0,10,1],"185":[0,0,80,0,25,0,10,1],"187":[0,2,100,0,25,0,10,1],"188":[0,0,80,0,25,0,10,1],"189":[0,0,80,0,25,0,10,1],"190":[0,2,100,0,25,0,10,1],"191":[0,0,80,0,25,0,10,1],"193":[0,0,80,0,25,0,10,1],"194":[0,2,100,0,25,0,10,1],"195":[0,0,80,0,25,0,10,1],"198":[0,2,100,0,25,0,10,1],"200":[0,0,80,0,25,0,10,1],"201":[0,0,80,0,25,0,10,1],"202":[0,0,80,0,25,0,10,1],"203":[0,0,80,0,25,0,10,1],"204":[0,0,80,0,25,0,10,1],"205":[0,0,80,0,25,0,10,1],"206":[0,0,80,0,25,0,10,1],"207":[0,0,80,0,25,0,10,1],"209":[0,0,80,0,25,0,10,1],"210":[0,0,80,0,25,0,10,1],"211":[0,0,80,0,25,0,10,1],"213":[0,0,80,0,25,0,10,1],"214":[0,0,80,0,25,0,10,1],"215":[0,0,80,0,25,0,10,1],"216":[0,2,100,0,25,0,10,1],"217":[0,0,80,0,25,0,10,1],"218":[0,0,80,0,25,0,10,1],"219":[0,0,80,0,25,0,10,1],"220":[0,0,80,0,25,0,10,1],"221":[0,0,80,0,25,0,10,1],"222":[0,0,80,0,25,0,10,1],"223":[0,0,80,0,25,0,10,1],"224":[0,0,80,0,25,0,10,1],"225":[0,2,100,0,25,0,10,1],"226":[0,0,80,0,25,0,10,1],"227":[0,0,80,0,25,0,10,1],"228":[0,0,80,0,25,0,10,1],"229":[0,0,80,0,25,0,10,1],"231":[0,0,80,0,25,0,10,1],"232":[0,0,80,0,25,0,10,1],"233":[0,0,80,0,25,0,10,1],"234":[0,0,80,0,25,0,10,1],"237":[0,0,80,0,25,0,10,1],"241":[0,0,80,0,25,0,10,1],"242":[0,0,80,0,25,0,10,1],"246":[0,0,80,0,25,0,10,1],"247":[0,0,80,0,25,0,10,1],"248":[0,0,80,0,25,0,10,1],"252":[0,2,100,0,25,0,10,1],"253":[0,0,80,0,25,0,10,1],"254":[0,0,80,0,25,0,10,1],"255":[0,2,100,0,25,0,10,1],"256":[0,0,80,0,25,0,10,1],"257":[0,0,80,0,25,0,10,1],"258":[0,2,100,0,25,0,10,1],"259":[0,0,80,0,25,0,10,1],"260":[0,0,80,0,25,0,10,1],"261":[0,2,100,0,25,0,10,1],"262":[0,0,80,0,25,0,10,1],"263":[0,2,100,0,25,0,10,1],"264":[0,0,80,0,25,0,10,1],"265":[0,2,100,0,25,0,10,1],"266":[0,0,80,0,25,0,10,1],"267":[0,0,80,0,25,0,10,1],"268":[0,0,80,0,25,0,10,1],"269":[0,0,80,0,25,0,10,1],"270":[0,0,80,0,25,0,10,1],"271":[0,0,80,0,25,0,10,1],"272":[0,0,80,0,25,0,10,1],"273":[0,2,100,0,25,0,10,1],"274":[0,0,80,0,25,0,10,1],"275":[0,0,80,0,25,0,10,1],"276":[0,0,80,0,25,0,10,1],"277":[0,0,80,0,25,0,10,1],"278":[0,0,80,0,25,0,10,1],"279":[0,0,80,0,25,0,10,1],"280":[0,0,80,0,25,0,10,1],"281":[0,0,80,0,25,0,10,1],"282":[0,0,80,0,25,0,10,1],"283":[0,0,80,0,25,0,10,1],"284":[0,0,80,0,25,0,10,1],"285":[0,2,100,0,25,0,10,1],"286":[0,0,80,0,25,0,10,1],"287":[0,0,80,0,25,0,10,1],"288":[0,0,80,0,25,0,10,1],"289":[0,0,80,0,25,0,10,1],"290":[0,0,80,0,25,0,10,1],"291":[0,0,80,0,25,0,10,1],"292":[0,0,80,0,25,0,10,1],"293":[0,2,100,0,25,0,10,1],"294":[0,0,80,0,25,0,10,1],"295":[0,0,80,0,25,0,10,1],"296":[0,2,100,0,25,0,10,1],"297":[0,0,80,0,25,0,10,1],"298":[0,0,80,0,25,0,10,1],"299":[0,2,100,0,25,0,10,1],"300":[0,2,100,0,25,0,10,1],"301":[0,0,80,0,25,0,10,1],"302":[0,2,100,0,25,0,10,1],"303":[0,0,80,0,25,0,10,1],"304":[0,2,100,0,25,0,10,1],"305":[0,0,80,0,25,0,10,1],"306":[0,0,80,0,25,0,10,1],"307":[0,2,100,0,25,0,10,1],"308":[0,0,80,0,25,0,10,1],"309":[0,2,100,0,25,0,10,1],"310":[0,0,80,0,25,0,10,1],"311":[0,0,80,0,25,0,10,1],"312":[0,2,100,0,25,0,10,1],"313":[0,0,80,0,25,0,10,1],"314":[0,0,80,0,25,0,10,1],"315":[0,2,100,0,25,0,10,1],"316":[0,2,100,0,25,0,10,1],"317":[0,0,80,0,25,0,10,1],"318":[0,0,80,0,25,0,10,1],"319":[0,0,80,0,25,0,10,1],"320":[0,2,100,0,25,0,10,1],"321":[0,0,80,0,25,0,10,1],"322":[0,2,100,0,25,0,10,1],"323":[0,0,80,0,25,0,10,1],"324":[0,0,80,0,25,0,10,1],"325":[0,2,100,0,25,0,10,1],"326":[0,0,80,0,25,0,10,1],"327":[0,0,80,0,25,0,10,1],"328":[0,0,80,0,25,0,10,1],"329":[0,0,80,0,25,0,10,1],"330":[0,0,80,0,25,0,10,1],"331":[0,2,100,0,25,0,10,1],"332":[0,0,80,0,25,0,10,1],"333":[0,0,80,0,25,0,10,1],"334":[0,0,80,0,25,0,10,1],"335":[0,2,100,0,25,0,10,1],"336":[0,2,100,0,25,0,10,1],"337":[0,0,80,0,25,0,10,1],"338":[0,0,80,0,25,0,10,1],"339":[0,2,100,0,25,0,10,1],"340":[0,0,80,0,25,0,10,1],"341":[0,0,80,0,25,0,10,1],"342":[0,0,80,0,25,0,10,1],"343":[0,2,100,0,25,0,10,1],"344":[0,0,80,0,25,0,10,1],"345":[0,0,80,0,25,0,10,1],"346":[0,0,80,0,25,0,10,1],"347":[0,0,80,0,25,0,10,1],"348":[0,0,80,0,25,0,10,1],"349":[0,0,80,0,25,0,10,1],"350":[0,0,80,0,25,0,10,1],"351":[0,0,80,0,25,0,10,1],"352":[0,0,80,0,25,0,10,1],"353":[0,2,100,0,25,0,10,1],"354":[0,0,80,0,25,0,10,1],"355":[0,2,100,0,25,0,10,1],"356":[0,0,80,0,25,0,10,1],"357":[0,0,80,0,25,0,10,1],"358":[0,0,80,0,25,0,10,1],"359":[0,0,80,0,25,0,10,1],"360":[0,0,80,0,25,0,10,1],"361":[0,0,80,0,25,0,10,1],"362":[0,0,80,0,25,0,10,1],"363":[0,2,100,0,25,0,10,1],"364":[0,0,80,0,25,0,10,1],"365":[0,0,80,0,25,0,10,1],"366":[0,0,80,0,25,0,10,1],"367":[0,0,80,0,25,0,10,1],"368":[0,0,80,0,25,0,10,1],"369":[0,0,80,0,25,0,10,1],"370":[0,0,80,0,25,0,10,1],"371":[0,0,80,0,25,0,10,1],"372":[0,0,80,0,25,0,10,1],"373":[0,0,80,0,25,0,10,1],"374":[0,0,80,0,25,0,10,1],"375":[0,0,80,0,25,0,10,1],"376":[0,0,80,0,25,0,10,1],"380":[0,0,80,0,25,0,10,1],"387":[0,0,80,0,25,0,10,1],"388":[0,0,80,0,25,0,10,1],"389":[0,0,80,0,25,0,10,1],"390":[0,0,80,0,25,0,10,1],"391":[0,0,80,0,25,0,10,1],"392":[0,0,80,0,25,0,10,1],"393":[0,0,80,0,25,0,10,1],"394":[0,0,80,0,25,0,10,1],"395":[0,0,80,0,25,0,10,1],"396":[0,0,80,0,25,0,10,1],"397":[0,0,80,0,25,0,10,1],"398":[0,0,80,0,25,0,10,1],"399":[0,0,80,0,25,0,10,1],"400":[0,0,80,0,25,0,10,1],"401":[0,0,80,0,25,0,10,1],"402":[0,0,80,0,25,0,10,1],"403":[0,0,80,0,25,0,10,1],"404":[0,0,80,0,25,0,10,1],"405":[0,0,80,0,25,0,10,1],"406":[0,0,80,0,25,0,10,1],"407":[0,0,80,0,25,0,10,1],"408":[1,0,80,0,25,0,10,1],"409":[1,0,80,0,25,0,10,1],"410":[1,0,80,0,25,0,10,1],"411":[1,0,80,0,25,0,10,1],"412":[1,0,80,0,25,0,10,1],"415":[1,0,80,0,25,0,10,1],"416":[1,0,80,0,25,0,10,1],"418":[0,0,80,0,25,0,10,1],"419":[0,0,80,0,25,0,10,1],"420":[1,0,80,0,25,0,10,1],"421":[1,0,80,0,25,0,10,1],"422":[1,0,80,0,25,0,10,1],"423":[1,0,80,0,25,0,10,1],"425":[0,0,80,0,25,0,10,1],"426":[0,0,80,0,25,0,10,1],"427":[0,0,80,0,25,0,10,1],"428":[0,0,80,0,25,0,10,1],"429":[0,0,80,0,25,0,10,1],"430":[0,0,80,0,25,0,10,1],"431":[1,0,80,0,25,0,10,1],"432":[1,0,80,0,25,0,10,1],"433":[0,0,80,0,25,0,10,1],"434":[1,0,80,0,25,0,10,1],"435":[1,0,80,0,25,0,10,1],"436":[1,0,80,0,25,0,10,1],"437":[1,0,80,0,25,0,10,1],"442":[0,0,80,0,25,0,10,1],"443":[1,0,80,0,25,0,10,1],"444":[1,0,80,0,25,0,10,1],"446":[0,0,80,0,25,0,10,1],"447":[0,0,80,0,25,0,10,1],"448":[0,0,80,0,25,0,10,1],"449":[1,0,80,0,25,0,10,1],"450":[1,0,80,0,25,0,10,1],"451":[1,0,80,0,25,0,10,1],"452":[1,0,80,0,25,0,10,1],"453":[0,0,80,0,25,0,10,1],"454":[0,0,80,0,25,0,10,1],"455":[0,0,80,0,25,0,10,1],"456":[0,0,80,0,25,0,10,1],"457":[0,0,80,0,25,0,10,1],"458":[0,0,80,0,25,0,10,1],"459":[0,0,80,0,25,0,10,1],"460":[0,0,80,0,25,0,10,1],"464":[0,0,80,0,25,0,10,1],"466":[0,0,80,0,25,0,10,1],"467":[0,0,80,0,25,0,10,1],"468":[0,0,80,0,25,0,10,1],"480":[0,1,80,0,25,0,10,1],"481":[0,1,80,0,25,0,10,1],"482":[0,1,80,0,25,0,10,1],"485":[0,0,80,0,25,0,10,1],"487":[0,0,80,0,25,0,10,1],"488":[0,0,80,0,25,0,10,1]}},"time":{"fi":[0,80],"fl":[0,30],"fc":0,"w1":[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23],"w2":[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23]},"more":{"l":"g"}}"#,
            r#"{"locs":{"h":["45.558235","12.433863"],"p":["45.564914","12.37436","10"],"r":["45.54964","12.43515","5"],"i":["45.557889","12.433863","0"],"t_p":["45.596287","12.325765","1579454562"],"t_r":["45.596287","12.325765","1579454562"],"t_i":["45.605552","9.514846","1623186317"]},"raid":{"u":1,"s":1,"x":1,"l":[5,6],"p":[-5,-6,79]},"pkmn":{"p1":1,"p0":1,"l":[]},"time":{"fi":[0,80],"fl":[0,30],"fc":0,"w1":[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23],"w2":[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23]},"lure":{"n":0,"f":0,"l":[]},"invs":{"n":1,"f":0,"l":[]},"more":{"l":"g"},"debug":true}"#,
        ];
        for s in &data {
            match serde_json::from_str::<BotConfig>(s) {
                Ok(conf) => println!("{} => {:?}", s, conf.pkmn.l),
                Err(e) => panic!("{} {}", e, s),
            }
        }
    }

    #[tokio::test]
    async fn pokemon_err() {
        tracing_subscriber::fmt::try_init().ok();

        let config = serde_json::from_str::<BotConfig>(r#"{"locs":{"h":["45.577350","12.367318"],"p":["45.576508","12.367384","10"],"r":["45.576989","12.366138","10"],"i":["45.575699","12.362555","10"],"t_p":["0","0","0"],"t_r":["0","0","0"],"t_i":["","",""]},"raid":{"u":1,"s":0,"x":0,"l":[5],"p":[599,403,129,639]},"pkmn":{"l":{"1":[1,1,94,0,0,0,0,1],"4":[1,1,94,0,0,0,0,1],"7":[1,1,94,0,0,0,0,1],"60":[1,1,94,0,0,0,0,1],"63":[1,1,94,0,0,0,0,1],"66":[1,1,94,0,0,0,0,1],"74":[1,1,94,0,0,0,0,1],"81":[1,1,94,0,0,0,0,1],"111":[1,1,94,0,0,0,0,1],"116":[1,1,94,0,0,0,0,1],"123":[1,1,94,0,0,0,0,1],"125":[1,1,94,0,0,0,0,1],"126":[1,1,94,0,0,0,0,1],"129":[1,1,94,0,0,0,0,1],"147":[1,1,94,0,0,0,0,1],"152":[1,1,94,0,0,0,0,1],"155":[1,1,94,0,0,0,0,1],"158":[1,1,94,0,0,0,0,1],"228":[1,1,95,0,0,0,0,1],"246":[1,1,94,0,0,0,0,1],"252":[1,1,94,0,0,0,0,1],"255":[1,1,94,0,0,0,0,1],"258":[1,1,94,0,0,0,0,1],"270":[1,1,94,0,0,0,0,1],"273":[1,1,94,0,0,0,0,1],"280":[1,1,94,0,0,0,0,1],"296":[1,1,94,0,0,0,0,1],"304":[1,1,94,0,0,0,0,1],"315":[1,1,95,0,0,0,0,1],"328":[1,1,94,0,0,0,0,1],"333":[1,1,94,0,0,0,0,1],"355":[1,1,94,0,0,0,0,1],"371":[1,1,94,0,0,0,0,1],"374":[1,1,94,0,0,0,0,1],"387":[1,1,94,0,0,0,0,1],"390":[1,1,94,0,0,0,0,1],"393":[1,1,94,0,0,0,0,1],"408":[1,1,94,0,0,0,0,1],"436":[1,1,94,0,0,0,0,1],"495":[1,1,94,0,0,0,0,1],"498":[1,1,94,0,0,0,0,1],"501":[1,1,94,0,0,0,0,1],"633":[1,1,94,0,0,0,0,1]}},"time":{"fi":[0,90],"fl":[0,30],"fc":0,"w1":[0,1,10,11,12,13,14,15,16,17,18,19,20,21,22,23],"w2":[0,9,10,11,14,15,16,17,18,19,20,21,22,23]},"qest":{"n":0,"l":[]},"invs":{"n":0,"f":0,"l":[]},"more":{"l":"g"}}"#).unwrap();
        let input: Pokemon = serde_json::from_str(r#"{"pokestop_id":"3d716717cc65421490684ef9b213a382.16","disappear_time":1571079918,"cp":null,"form":0,"move_1":null,"longitude":12.19398,"costume":0,"pokemon_id":280,"disappear_time_verified":false,"gender":1,"individual_attack":9,"spawnpoint_id":"None","latitude":45.35340,"pokemon_level":15,"move_2":null,"individual_defense":3,"weight":null,"encounter_id":"12661125248363616471","height":null,"weather":1,"first_seen":1571078718,"individual_stamina":15,"last_modified_time":1571078718}"#).unwrap();
        assert!(config
            .submit(
                &Utc::now(),
                &Platform::Unknown,
                &Request::Pokemon(Box::new(input.into()))
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn pokemon_ok() {
        tracing_subscriber::fmt::try_init().ok();

        let config = serde_json::from_str::<BotConfig>(r#"{"locs":{"h":["43.787206","11.252832"],"p":["43.781134","11.259613","7"],"r":["43.778035","11.259184","7"],"i":["43.778159","11.259098","7"],"t_p":["0","0","0"],"t_r":["0","0","0"],"t_i":["","",""]},"raid":{"u":0,"s":0,"x":0,"l":[5],"p":[-5]},"pkmn":{"l":{"1":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"2":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"3":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"4":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"5":[1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,1,99,1,99],"6":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"7":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"8":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"9":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"10":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"11":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"12":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"15":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"18":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"19":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"20":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"21":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"22":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"23":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"24":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"25":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"26":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"27":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"28":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"29":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"30":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"31":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"32":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"33":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"34":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"35":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"36":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"39":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"40":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"41":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"42":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"45":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"46":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"47":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"48":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"49":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"50":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"51":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"52":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"53":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"54":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"55":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"56":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"57":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"58":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"59":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"61":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"62":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"65":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"66":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"67":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"68":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"69":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"70":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"71":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"72":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"73":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"74":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"75":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"76":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"77":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"78":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"79":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"80":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"83":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"84":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"85":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"88":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"89":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"90":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"91":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"92":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"93":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"94":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"95":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"96":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"97":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"98":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"99":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"102":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"103":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"104":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"105":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"106":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"107":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"108":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"109":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"110":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"111":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"112":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"113":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"114":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"116":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"117":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"118":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"119":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"120":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"121":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"122":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"123":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"124":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"125":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"126":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"127":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"130":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"131":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"132":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"133":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"134":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"135":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"136":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"137":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"138":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"139":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"140":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"141":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"143":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"149":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"152":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"153":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"154":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"155":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"156":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"157":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"158":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"159":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"160":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"163":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"164":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"167":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"168":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"169":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"170":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"171":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"176":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"177":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"178":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"179":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"180":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"181":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"183":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"184":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"185":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"187":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"188":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"189":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"190":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"191":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"194":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"195":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"198":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"200":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"201":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"202":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"203":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"204":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"205":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"206":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"207":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"209":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"210":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"211":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"213":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"215":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"216":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"217":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"219":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"220":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"221":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"222":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"223":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"224":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"225":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"226":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"227":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"229":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"231":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"232":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"234":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"237":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"241":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"242":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"246":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"247":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"248":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"252":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"253":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"254":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"255":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"256":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"257":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"258":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"259":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"260":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"261":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"262":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"263":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"264":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"265":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"270":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"271":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"272":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"273":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"274":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"275":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"276":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"277":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"278":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"279":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"280":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"281":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"283":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"284":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"285":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"287":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"288":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"289":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"290":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"295":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"296":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"297":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"299":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"301":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"302":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"304":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"305":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"306":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"308":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"309":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"310":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"311":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"312":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"313":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"314":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"318":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"319":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"320":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"322":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"323":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"325":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"326":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"328":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"329":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"330":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"331":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"332":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"333":[1,0,0,0,0,0,0,0,0,0,2,3,2,11,2,11,1,0,0,1,99,1,99],"335":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"336":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"337":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"339":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"340":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"341":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"342":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"343":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"345":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"346":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"347":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"348":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"349":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"353":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"354":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"356":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"357":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"358":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"359":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"361":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"362":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"364":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"366":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"370":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"371":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"372":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"373":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"374":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"375":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"376":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"387":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"388":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"391":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"393":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"394":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"397":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"399":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"400":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"401":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"402":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"408":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"410":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"412":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"415":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"418":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"419":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"420":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"421":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"422":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"426":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"427":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"428":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"431":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"432":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"434":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"435":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"436":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"437":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"441":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"443":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"444":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"449":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"450":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"452":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"453":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"456":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"457":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"459":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"460":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"481":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"495":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"496":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"498":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"499":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"501":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"502":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"506":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"507":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"509":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"513":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"515":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"520":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"522":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"524":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"529":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"531":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"535":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"538":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"540":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"543":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"546":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"548":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"550":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"554":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"557":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"562":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"564":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"566":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"568":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"572":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"574":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"577":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"580":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"585":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"587":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"588":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"594":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"597":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"605":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"607":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"608":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"610":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"611":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"613":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"615":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"616":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"618":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"622":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"631":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"632":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"633":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"634":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"650":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"653":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"656":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"661":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"667":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99],"714":[1,0,0,0,0,0,0,0,0,0,2,0,2,0,2,0,1,0,0,1,99,1,99]}},"time":{"fi":[0,80],"fl":[0,30],"fc":0,"w1":[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23],"w2":[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23]},"invs":{"n":0,"f":0,"l":[41,42,43,44]},"more":{"l":"g"},"debug":true}"#).unwrap();
        let input: Pokemon = serde_json::from_str(r#"{"pokestop_id":"3d716717cc65421490684ef9b213a382.16","disappear_time":1571079918,"cp":null,"form":0,"move_1":null,"longitude":11.241531239206385,"costume":0,"pokemon_id":656,"disappear_time_verified":false,"gender":1,"individual_attack":0,"spawnpoint_id":"None","latitude":43.771913285587665,"pokemon_level":15,"move_2":null,"individual_defense":15,"weight":null,"encounter_id":"12661125248363616471","height":null,"weather":1,"first_seen":1571078718,"individual_stamina":15,"last_modified_time":1571078718,"pvp_rankings_great_league":[{"rank":809,"percentage":0.8700643398554556,"level":40.0,"form":0,"cp":989,"pokemon":255},{"rank":351,"percentage":0.9907850542156611,"level":38.5,"pokemon":256,"form":0,"cp":1490},{"pokemon":257,"form":0,"percentage":null,"cp":null,"level":null,"rank":null}]}"#).unwrap();
        assert!(config
            .submit(
                &Utc::now(),
                &Platform::Unknown,
                &Request::Pokemon(Box::new(input.into()))
            )
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn raid_err() {
        tracing_subscriber::fmt::try_init().ok();

        let config = serde_json::from_str::<BotConfig>(r#"{"locs":{"h":["39.243490","9.121891"],"p":["39.243673","9.121859","10"],"r":["39.243035","9.121345","0"],"i":["39.243493","9.121624","0"],"t_p":["0","0","0"],"t_r":["0","0","0"],"t_i":["","",""]},"raid":{"u":0,"s":1,"x":1,"l":[5],"p":[]},"pkmn":{"l":{"1":[1,1,100,0,0,0,0,1],"2":[1,1,100],"3":[1,1,100],"4":[1,1,100],"5":[1,1,100],"6":[1,1,100],"7":[1,1,100],"8":[1,1,100],"9":[1,1,100],"10":[1,1,100],"11":[1,1,100],"12":[1,1,100],"13":[1,1,100],"14":[1,1,100],"15":[1,1,100],"16":[1,1,100],"17":[1,1,100],"18":[1,1,100],"19":[1,1,100],"20":[1,1,100],"21":[1,1,100],"22":[1,1,100],"23":[1,1,100],"24":[1,1,100],"25":[1,1,100],"26":[1,1,100],"27":[1,1,100],"28":[1,1,100],"29":[1,1,100],"30":[1,1,100],"31":[1,1,100],"32":[1,1,100],"33":[1,1,100],"34":[1,1,100],"35":[1,1,100],"36":[1,1,100],"37":[1,1,100],"38":[1,1,100],"39":[1,1,100],"40":[1,1,100],"41":[1,1,100],"42":[1,1,100],"43":[1,1,100],"44":[1,1,100],"45":[1,1,100],"46":[1,1,100],"47":[1,1,100],"48":[1,1,100],"49":[1,1,100],"50":[1,1,100],"51":[1,1,100],"52":[1,1,100],"53":[1,1,100],"54":[1,1,100],"55":[1,1,100],"56":[1,1,100],"57":[1,1,100],"58":[1,1,100],"59":[1,1,100],"60":[1,1,100],"61":[1,1,100],"62":[1,1,100],"63":[1,1,100],"64":[1,1,100],"65":[1,1,100],"66":[1,1,100],"67":[1,1,100],"68":[1,1,100],"69":[1,1,100],"70":[1,1,100],"71":[1,1,100],"72":[1,1,100],"73":[1,1,100],"74":[1,1,100],"75":[1,1,100],"76":[1,1,100],"77":[1,1,100],"78":[1,1,100],"79":[1,1,100],"80":[1,1,100],"81":[1,1,100],"82":[1,1,100],"83":[1,1,80],"84":[1,1,100],"85":[1,1,100],"86":[1,1,100],"87":[1,1,100],"88":[1,1,100],"89":[1,1,100],"90":[1,1,100],"91":[1,1,100],"92":[1,1,100],"93":[1,1,100],"94":[1,1,100],"95":[1,1,100],"96":[1,1,100],"97":[1,1,100],"98":[1,1,100],"99":[1,1,100],"100":[1,1,100],"101":[1,1,100],"102":[1,1,100],"103":[1,1,100],"104":[1,1,100],"105":[1,1,100],"106":[1,1,100],"107":[1,1,100],"108":[1,1,100],"109":[1,1,100],"110":[1,1,100],"111":[1,1,100],"112":[1,1,100],"113":[1,0,0,0,0,0,0,1],"114":[1,1,100],"116":[1,1,100],"117":[1,1,100],"118":[1,1,100],"119":[1,1,100],"120":[1,1,100],"121":[1,1,100],"122":[1,1,100],"123":[1,1,100],"124":[1,1,100],"125":[1,1,100],"126":[1,1,100],"127":[1,1,100],"129":[1,1,100,0,0,0,0,1],"130":[1,1,100,0,0,0,0,1],"131":[1,1,96,0,0,0,0,1],"132":[1,1,100],"133":[1,1,100],"134":[1,1,100],"135":[1,1,100],"136":[1,1,100],"137":[1,1,100],"138":[1,1,100],"139":[1,1,100],"140":[1,1,100],"141":[1,1,100],"142":[1,1,100],"143":[1,1,90,0,0,0,0,1],"147":[1,1,98,0,0,0,0,1],"148":[1,1,90,0,0,0,0,1],"149":[1,0,0,0,0,0,0,1],"152":[1,1,100],"153":[1,1,100],"154":[1,1,100],"155":[1,1,100],"156":[1,1,100],"157":[1,1,100],"158":[1,1,100],"159":[1,1,100],"160":[1,1,100],"161":[1,1,100],"162":[1,1,100],"163":[1,1,100],"164":[1,1,100],"165":[1,1,100],"166":[1,1,100],"167":[1,1,100],"168":[1,1,100],"169":[1,1,100],"170":[1,1,100],"171":[1,1,100],"176":[1,1,80],"177":[1,1,100],"178":[1,1,100],"179":[1,1,100],"180":[1,1,100],"181":[1,1,100],"183":[1,1,100],"184":[1,1,100],"185":[1,1,100],"187":[1,1,100],"188":[1,1,100],"189":[1,1,100],"190":[1,1,100],"191":[1,1,100],"193":[1,1,100],"194":[1,1,100],"195":[1,1,100],"198":[1,1,100,0,0,0,0,1],"200":[1,1,100],"201":[1,0,0,0,0,0,0,1],"202":[1,1,100],"203":[1,1,100],"204":[1,1,100],"205":[1,1,100],"206":[1,1,100],"207":[1,1,100],"209":[1,1,100],"210":[1,1,100],"211":[1,1,100],"213":[1,1,100],"215":[1,1,100,0,0,0,0,1],"216":[1,1,100],"217":[1,1,100],"218":[1,1,100],"219":[1,1,100],"220":[1,1,100],"221":[1,1,100],"222":[1,1,100],"223":[1,1,100],"224":[1,1,100],"225":[1,1,100],"226":[1,1,100],"227":[1,1,100],"228":[1,1,100],"229":[1,1,100],"231":[1,1,100],"232":[1,1,100],"234":[1,1,100],"237":[1,1,100],"241":[1,1,100],"242":[1,1,0,0,0,0,0,1],"246":[1,1,96,0,0,0,0,1],"247":[1,1,4,0,0,0,0,1],"248":[1,0,0,0,0,0,0,1],"252":[1,1,100],"253":[1,1,100],"254":[1,1,100],"255":[1,1,100],"256":[1,1,100],"257":[1,1,100],"258":[1,1,100],"259":[1,1,100],"260":[1,1,100],"261":[1,1,100],"262":[1,1,100],"263":[1,1,100],"264":[1,1,100],"265":[1,1,100],"270":[1,1,100],"271":[1,1,100],"272":[1,1,100],"273":[1,1,100],"274":[1,1,100],"275":[1,1,100],"276":[1,1,100],"277":[1,1,100],"278":[1,1,100],"279":[1,1,100],"280":[1,1,100,0,0,0,0,1],"281":[1,1,96,0,0,0,0,1],"283":[1,1,100],"284":[1,1,100],"285":[1,1,100],"286":[1,1,100],"287":[1,1,100],"288":[1,1,100],"289":[1,1,100],"293":[1,1,100],"294":[1,1,100],"295":[1,1,100],"296":[1,1,100],"297":[1,1,100],"299":[1,1,100],"300":[1,1,100],"301":[1,1,100],"302":[1,1,100],"304":[1,1,100],"305":[1,1,100],"306":[1,1,100],"307":[1,1,100],"308":[1,1,100],"309":[1,1,100],"310":[1,1,100],"311":[1,1,100],"312":[1,1,100],"313":[1,1,100],"314":[1,1,100],"315":[1,1,100,0,0,0,0,1],"316":[1,1,100],"317":[1,1,100],"318":[1,1,100],"319":[1,1,100],"320":[1,1,100],"322":[1,1,100],"323":[1,1,100],"325":[1,1,100],"326":[1,1,100],"328":[1,1,100],"329":[1,1,100],"330":[1,1,100],"331":[1,1,100],"332":[1,1,100],"333":[1,1,100],"335":[1,1,100],"336":[1,1,100],"337":[1,1,100],"338":[1,1,100],"339":[1,1,100],"340":[1,1,100],"341":[1,1,100],"342":[1,1,100],"343":[1,1,100],"344":[1,1,100],"345":[1,1,100],"346":[1,1,100],"347":[1,1,100],"348":[1,1,100],"349":[1,1,100,0,0,0,0,1],"351":[1,1,100],"353":[1,1,100],"354":[1,1,100],"355":[1,1,100],"356":[1,1,100],"357":[1,1,100],"358":[1,1,100],"359":[1,1,100],"361":[1,1,100],"362":[1,1,100],"363":[1,1,100],"364":[1,1,100],"366":[1,1,100],"370":[1,1,100],"371":[1,1,96,0,0,0,0,1],"372":[1,1,4,0,0,0,0,1],"373":[1,1,4,0,0,0,0,1],"374":[1,1,98,0,0,0,0,1],"375":[1,1,4,0,0,0,0,1],"376":[1,0,0,0,0,0,0,1],"387":[1,1,100],"388":[1,1,100],"390":[1,1,100],"391":[1,1,100],"393":[1,1,100],"394":[1,1,100],"396":[1,1,100],"397":[1,1,100],"399":[1,1,100],"400":[1,1,100],"401":[1,1,100],"402":[1,1,100],"408":[1,1,96],"410":[1,1,100],"412":[1,1,100],"415":[1,1,100],"418":[1,1,100],"419":[1,1,100],"420":[1,1,100],"421":[1,1,100],"422":[1,1,100],"425":[1,1,100],"426":[1,1,100],"427":[1,1,100],"428":[1,1,100],"431":[1,1,100],"432":[1,1,100],"434":[1,1,100],"435":[1,1,100],"436":[1,1,100],"437":[1,1,100],"443":[1,0,0,0,0,0,0,1],"444":[1,1,0,0,0,0,0,1],"449":[1,1,100],"450":[1,1,100],"451":[1,1,100],"452":[1,1,100],"453":[1,1,100],"454":[1,1,100],"456":[1,1,100],"457":[1,1,100],"459":[1,1,100],"460":[1,1,100],"481":[1,0,0,0,0,0,0,1],"495":[1,1,100],"496":[1,1,100],"498":[1,1,100],"499":[1,1,100],"501":[1,1,100],"502":[1,1,100],"504":[1,1,100],"509":[1,1,100],"513":[1,1,100],"515":[1,1,100],"519":[1,1,100],"520":[1,1,100],"522":[1,1,100],"524":[1,1,100],"527":[1,1,100],"529":[1,1,100,0,0,0,0,1],"531":[1,1,100],"535":[1,1,100],"539":[1,1,100],"540":[1,1,80],"543":[1,1,100],"546":[1,1,100],"548":[1,1,100],"550":[1,1,100],"554":[1],"557":[1,1,100],"562":[1,1,100],"564":[1,1,100],"566":[1,1,100],"568":[1,1,100],"572":[1,1,100],"574":[1,1,94],"577":[1,1,100],"580":[1,1,100],"585":[1,1,100],"587":[1,1,100],"588":[1,1,100],"590":[1,1,100],"594":[1,1,100],"595":[1,1,100],"597":[1,1,100],"605":[1,1,100],"607":[1,1,96,0,0,0,0,1],"608":[1,0,0,0,0,0,0,1],"610":[1],"611":[1],"613":[1,1,100],"615":[1,1,100],"616":[1,1,100],"618":[1,1,100],"622":[1,1,100],"631":[1,1,100],"632":[1,1,100],"633":[1,0,0,0,0,0,0,1],"634":[1,0,0,0,0,0,0,1],"650":[1,1,100],"653":[1,1,100],"656":[1,1,100],"659":[1,1,100],"661":[1,1,100],"667":[1,1,100],"714":[1,1,94]}},"time":{"fi":[0,80],"fl":[0,30],"fc":0,"w1":[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23],"w2":[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23]},"invs":{"n":0,"f":0,"l":[4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,37,38,39,47,48,49,50]},"more":{"l":"g"}}"#).unwrap();
        let input: Raid = serde_json::from_str(r#"{"gym_name":"","gym_url":"","end":1564338495,"form":0,"is_exclusive":false,"longitude":9.121235,"cp":0,"team_id":1,"pokemon_id":0,"gym_id":"fe20dd37398341a4b83751c5c050aaec.16","move_2":0,"level":4,"move_1":0,"start":1564335795,"ex_raid_eligible":false,"spawn":1564332195,"latitude":39.243042,"gender":0}"#).unwrap();
        assert!(config
            .submit(
                &Utc::now(),
                &Platform::Unknown,
                &Request::Raid(Box::new(input))
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn invasion_err() {
        tracing_subscriber::fmt::try_init().ok();

        let config = serde_json::from_str::<BotConfig>(r#"{"locs":{"h":["45.610849","8.842825"],"p":["45.603492","8.823278","15"],"r":["45.611302","8.828447","15"],"i":["45.605751","8.810241","15"],"t_p":["0","0","0"],"t_r":["0","0","0"],"t_i":["","",""]},"raid":{"u":0,"s":1,"x":0,"l":[5],"p":[]},"pkmn":{"p1":0,"p0":0,"l":{"1":[1,1,25,0,0,0,0,0,0,0,1,2,3,11,3,10,1],"4":[1,1,19,0,0,0,0,0,0,0,1,1,3,11,3,11],"7":[1,1,95],"13":[1,1,96,0,0,0,0,0,0,0,2,0,2,11,3,14,1],"27":[1,1,32,0,0,0,0,0,0,0,1,2,3,10,3,10],"35":[1,1,24,0,0,0,0,0,0,0,1,1,3,13,3,10],"39":[1,1,24,0,0,0,0,0,0,0,1,1,3,13,3,12],"56":[1,1,24,0,0,0,0,0,0,0,1,2,3,10,3,8],"60":[1,1,24,0,0,0,0,0,0,0,2,8,3,13,3,13],"66":[1,1,19,0,0,0,0,0,0,0,2,0,3,10,3,8,1],"72":[1,1,35,0,0,0,0,0,0,0,2,0,3,11,3,11],"74":[1,1,24,0,0,0,0,0,0,0,1,3,3,10,3,7],"79":[1,1,25,0,0,0,0,0,0,0,1,2,3,7,3,6],"81":[1,1,97,0,0,0,0,0,0,0,1,2,3,10,3,10,1],"83":[1,1,26,0,0,0,0,0,0,0,1,4,3,9,3,9],"86":[1,1,98,0,0,0,0,0,0,0,0,0,0,0,0,0,1],"88":[1,1,24,0,0,0,0,0,0,0,1,1,3,13,3,11],"92":[1,1,97],"95":[1,1,25,0,0,0,0,0,0,0,1,2,3,11,3,10],"96":[1,1,25,0,0,0,0,0,0,0,1,2,3,10,3,9],"98":[1,1,95],"102":[1,1,24,0,0,0,0,0,0,0,1,1,3,10,3,9],"105":[1,1,80,0,0,0,0,0,0,0,1,2,3,11,3,11],"108":[1,1,24,0,0,0,0,0,0,0,1,8,3,13,3,11],"114":[1,1,24,0,0,0,0,0,0,0,1,3,3,9,3,8],"123":[1,1,26,0,0,0,0,0,0,0,2,0,3,10,3,9,1],"124":[1,1,98],"125":[1,1,97],"127":[1,1,97],"131":[1,1,25,0,0,0,0,0,0,0,1,2,3,10,3,10],"133":[1,1,25,0,0,0,0,0,0,0,1,1,3,6,3,8,1],"143":[1,1,95,0,0,0,0,0,0,0,0,0,0,0,0,0,1],"147":[1,1,24,0,0,0,0,0,0,0,1,2,3,10,3,11],"152":[1,1,25,0,0,0,0,0,0,0,1,2,3,7,3,9],"153":[1,1,24,0,0,0,0,0,0,0,1,2,3,7,3,9],"158":[1,1,97],"170":[1,1,25,0,0,0,0,0,0,0,1,2,3,10,3,8],"179":[1,1,97],"180":[1,1,98],"183":[1,1,26,0,0,0,0,0,0,0,1,1,3,11,3,11],"184":[1,1,26,0,0,0,0,0,0,0,1,1,3,11,3,11],"187":[1,1,24,0,0,0,0,0,0,0,2,4,3,14,3,13],"193":[1,1,96],"194":[1,1,20,0,0,0,0,0,0,0,2,0,3,10,3,12],"201":[1,1,6],"207":[1,1,24,0,0,0,0,0,0,0,1,3,3,6,3,7],"215":[1,1,98,0,0,0,0,0,0,0,0,0,0,0,0,0,1],"216":[1,1,95],"220":[1,1,100,0,0,0,0,0,0,0,0,0,0,0,0,0,1],"226":[1,1,25,0,0,0,0,0,0,0,2,0,3,8,3,7],"227":[1,1,96],"228":[1,1,95],"246":[1,1,97],"255":[1,1,25,0,0,0,0,0,0,0,1,4,3,11,3,10],"258":[1,1,24,0,0,0,0,0,0,0,1,2,3,10,3,9],"278":[1,1,22,0,0,0,0,0,0,0,2,0,3,10,3,8],"280":[1,1,100,0,0,0,0,0,0,0,0,0,0,0,0,0,1],"285":[1,1,24,0,0,0,0,0,0,0,1,4,3,9,3,9,1],"287":[1,1,23,0,0,0,0,0,0,0,1,2,3,10,3,10],"296":[1,1,96],"297":[1,1,96],"302":[1,1,96,0,0,0,0,0,0,0,1,1,3,15,3,12],"307":[1,1,24,0,0,0,0,0,0,0,1,5,3,13,3,12],"309":[1,1,96],"318":[1,1,95],"328":[1,1,24,0,0,0,0,0,0,0,1,3,3,11,3,7],"333":[1,1,98],"335":[1,1,95],"339":[1,1,24,0,0,0,0,0,0,0,1,2,3,8,3,9],"341":[1,1,96],"349":[1,1,98],"359":[1,1,95],"361":[1,1,25,0,0,0,0,0,0,2,1,2,3,12,3,13,1],"371":[1,1,100],"374":[1,1,100],"390":[1,1,96],"408":[1,1,96],"410":[1,1,26,0,0,0,0,1,0,0,1,2,3,10,3,10],"425":[1,1,95,0,0,0,0,0,0,0,1,1,3,13,3,11],"443":[1,1,97],"449":[1,1,98],"451":[1,1,24,0,0,0,0,0,0,0,1,1,3,10,3,10],"453":[1,1,26,0,0,0,0,0,0,0,1,2,3,13,3,11],"459":[1,1,38,0,0,0,0,0,0,0,1,5,3,12,3,12,1],"460":[1,1,22,0,0,0,0,1,0,0,1,4,3,12,3,12],"495":[1,1,95],"498":[1,1,95],"501":[1,1,95],"529":[1,1,97],"531":[1,1,95],"540":[1,1,10],"546":[1,1,24,0,0,0,0,0,0,0,2,0,3,7,3,8],"548":[1,1,10],"554":[1,1,95],"557":[1,1,24,0,0,0,0,0,0,0,1,1,3,9,3,10],"566":[1,1,98],"587":[1,1,15],"588":[1,1,24,0,0,0,0,0,0,0,1,2,3,10,3,10],"595":[1,1,22,0,0,0,0,0,0,0,1,15,3,13,3,14],"597":[1,1,24,0,0,0,0,0,0,0,1,3,3,9,3,8,1],"607":[1,1,97],"610":[1,1,95],"618":[1,1,20,0,0,0,0,0,0,0,2,0,2,12,3,15,1],"622":[1,1,24,0,0,0,0,0,0,0,1,3,3,9,3,8],"633":[1,1,95],"650":[1,1,26,0,0,0,0,0,0,0,1,3,3,6,3,7],"653":[1,1,96],"656":[1,1,24,0,0,0,0,0,0,0,2,0,3,10,3,11],"659":[1,1,24,0,0,0,0,0,0,0,2,0,2,15,2,11,1],"661":[1,1,96],"714":[1,1,24]}},"time":{"fi":[1,94],"fl":[0,25],"fc":0,"w1":[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23],"w2":[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23]},"lure":{"n":0,"f":0,"l":[]},"invs":{"n":1,"f":1,"l":[9,11,12,16,26,49]},"more":{"l":"g"},"debug":true}"#).unwrap();
        let input: Pokestop = serde_json::from_str(r#"{"lure_expiration":0,"enabled":true,"updated":1564332327,"url":"http://lh6.ggpht.com/ga78DsEtufPUGu0H0oE2ZOeagwxe8aQ4k4-kBLEDdSfeFVshH8gHhQN1GMcw1OFd_n94NpwTkOa16zR5DqUL","pokestop_display":1,"longitude":8.845038,"lure_id":501,"last_modified":1564329955,"pokestop_id":"54e0ee4c0e7a42ca93d2e93ee720dc90.16","name":"Macello Civico - Sede Scout","incident_expire_timestamp":1564333601,"grunt_type":48,"latitude":45.606137}"#).unwrap();
        assert!(config
            .submit(
                &Utc::now(),
                &Platform::Unknown,
                &Request::Invasion(Box::new(input))
            )
            .await
            .is_err());
    }
}

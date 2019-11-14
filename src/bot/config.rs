use std::f64::consts::PI;
use std::collections::HashMap;

use serde::{Serialize, Deserialize};

use serde_json::Value as JsonValue;

use chrono::{Local, DateTime};

use geo::Point;

use geo_raycasting::RayCasting;

use log::error;

use log::info;

// use crate::lists::COMMON;
use crate::entities::{Pokemon, Pokestop, Raid, Request, Gender, Quest};
use crate::lists::CITIES;
// use crate::telegram::Image;

use super::message::{Message, PokemonMessage, RaidMessage, InvasionMessage, QuestMessage};

const MAX_DISTANCE: f64 = 15f64;
// const MIN_IV_LIMIT: f32 = 36f32;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotConfig {
    pub scadenza: Option<i64>,
    pub debug: Option<bool>,
    pub locs: BotLocs,
    pub raid: BotRaid,
    pub pkmn: BotPkmn,
    pub time: BotTime,
    pub qest: Option<BotQest>,
    pub invs: Option<BotInvs>,
    pub more: BotMore,
}

impl BotConfig {
    pub async fn validate(&self, user_id: &str, city_id: u16) -> bool {
        let polygon = match CITIES.read().await.get(&city_id) {
            Some(c) => c.coordinates.clone(),
            None => {
                info!("{} is associated to disabled city {}", user_id, city_id);
                return false;
            },
        };

        // match (BotLocs::convert_to_f64(&self.locs.h[0]), BotLocs::convert_to_f64(&self.locs.h[1])) {
        //     (Ok(x), Ok(y)) => {
        //         let p: Point<f64> = (x, y).into();
        //         if !polygon.within(&p) {
        //             info!("{} has home pointer out of city {}", user_id, city_id);
        //             return false;
        //         }
        //     },
        //     _ => {},
        // }

        match (BotLocs::convert_to_f64(&self.locs.p[0]), BotLocs::convert_to_f64(&self.locs.p[1])) {
            (Ok(x), Ok(y)) => {
                let p: Point<f64> = (x, y).into();
                if !polygon.within(&p) {
                    info!("{} has pokemon pointer out of city {}", user_id, city_id);
                    return false;
                }
            },
            _ => {},
        }

        match (BotLocs::convert_to_f64(&self.locs.r[0]), BotLocs::convert_to_f64(&self.locs.r[1])) {
            (Ok(x), Ok(y)) => {
                let p: Point<f64> = (x, y).into();
                if !polygon.within(&p) {
                    info!("{} has raid pointer out of city {}", user_id, city_id);
                    return false;
                }
            },
            _ => {},
        }

        if let Some(pos) = self.locs.i.as_ref() {
            match (BotLocs::convert_to_f64(&pos[0]), BotLocs::convert_to_f64(&pos[1])) {
                (Ok(x), Ok(y)) => {
                    let p: Point<f64> = (x, y).into();
                    if !polygon.within(&p) {
                        info!("{} has pokestop pointer out of city {}", user_id, city_id);
                        return false;
                    }
                },
                _ => {},
            }
        }

        let now = Local::now().timestamp();

        if BotLocs::convert_to_i64(&self.locs.t_p[2]).map(|i| i > now) == Ok(true) {
            match (BotLocs::convert_to_f64(&self.locs.t_p[0]), BotLocs::convert_to_f64(&self.locs.t_p[1])) {
                (Ok(x), Ok(y)) => {
                    let p: Point<f64> = (x, y).into();
                    let mut not_found = true;
                    for (_, city) in CITIES.read().await.iter() {
                        if city.coordinates.within(&p) {
                            not_found = false;
                            break;
                        }
                    }
                    if not_found {
                        info!("{} has temp pokemon pointer out of any city", user_id);
                        return false;
                    }
                },
                _ => {},
            }
        }

        if BotLocs::convert_to_i64(&self.locs.t_r[2]).map(|i| i > now) == Ok(true) {
            match (BotLocs::convert_to_f64(&self.locs.t_r[0]), BotLocs::convert_to_f64(&self.locs.t_r[1])) {
                (Ok(x), Ok(y)) => {
                    let p: Point<f64> = (x, y).into();
                    let mut not_found = true;
                    for (_, city) in CITIES.read().await.iter() {
                        if city.coordinates.within(&p) {
                            not_found = false;
                            break;
                        }
                    }
                    if not_found {
                        info!("{} has temp raid pointer out of any city", user_id);
                        return false;
                    }
                },
                _ => {},
            }
        }

        if let Some(pos) = self.locs.t_i.as_ref() {
            if BotLocs::convert_to_i64(&pos[2]).map(|i| i > now) == Ok(true) {
                match (BotLocs::convert_to_f64(&pos[0]), BotLocs::convert_to_f64(&pos[1])) {
                    (Ok(x), Ok(y)) => {
                        let p: Point<f64> = (x, y).into();
                        let mut not_found = true;
                        for (_, city) in CITIES.read().await.iter() {
                            if city.coordinates.within(&p) {
                                not_found = false;
                                break;
                            }
                        }
                        if not_found {
                            info!("{} has temp pokestop pointer out of any city", user_id);
                            return false;
                        }
                    },
                    _ => {},
                }
            }
        }

        true
    }

    pub fn submit(&self, now: &DateTime<Local>, input: &Request) -> Result<Box<dyn Message + Send + Sync>, ()> {
        if !self.time.is_active()? && self.time.fi[0] == 0 && self.time.fl[0] == 0 {
            #[cfg(test)]
            info!("Webhook discarded for time configs");

            Err(())
        }
        else {
            match input {
                Request::Pokemon(p) => Ok(Box::new(self.submit_pokemon(now, p)?)),
                Request::Raid(r) => Ok(Box::new(self.submit_raid(now, r)?)),
                Request::Quest(q) => Ok(Box::new(self.submit_quest(now, q)?)),
                Request::Invasion(i) => Ok(Box::new(self.submit_invasion(now, i)?)),
                _ => Err(()),
            }
        }
    }

    fn submit_pokemon(&self, now: &DateTime<Local>, input: &Box<Pokemon>) -> Result<PokemonMessage, ()> {
        let pokemon_id = input.pokemon_id.to_string();
        let filter = self.pkmn.l.get(&pokemon_id).ok_or_else(|| ())?;
        if filter.get(0) == Some(&0) {
            return Err(());
        }

        let loc = self.locs.get_pokemon_settings()?;
        let pos = (input.latitude, input.longitude);

        let mut debug = format!("Scansione avvenuta alle {}\n", now.format("%T").to_string());
        let rad = if filter.get(5) == Some(&1) {
            // $pkmn_rad = ValMinMax($filter[6], 0.1, MAX_DISTANCE);
            let rad = MAX_DISTANCE.min(f64::from(*(filter.get(6).ok_or_else(|| ())?))).max(0.1);
            debug.push_str(&format!("Distanza personalizzata per Pokémon inferiore a {:.2} km", rad));
            rad
        }
        else {
            // $pkmn_rad = ValMinMax($locs["p"][2], 0.1, MAX_DISTANCE);
            let rad = MAX_DISTANCE.min(BotLocs::convert_to_f64(&self.locs.p[2])?).max(0.1);
            debug.push_str(&format!("Distanza standard per Pokémon inferiore a {:.2} km", rad));
            rad
        };

        let dist = BotLocs::calc_dist(loc, pos)?;
        if dist > rad {
            #[cfg(test)]
            info!("Pokémon discarded for distance: loc {:?} pos {:?} dist {} rad {}", loc, pos, dist, rad);

            return Err(());
        }
        else {
            debug.push_str(&format!(" ({:.2} km)", dist));
        }

        let iv = match (input.individual_attack, input.individual_defense, input.individual_stamina) {
            (Some(atk), Some(def), Some(sta)) => Some((f32::from(atk + def + sta) / 45f32) * 100f32),
            _ => None,
        };

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

        if !self.time.is_active()? {
            if !self.time.bypass(iv, input.pokemon_level) {
                #[cfg(test)]
                info!("Pokémon discarded for time config: pokemon_id {} iv {:?} level {:?}", pokemon_id, iv, input.pokemon_level);

                return Err(());
            }
            else {
                debug.push_str(&format!("\nFiltro orario non attivo ma eccezione per {}", self.time.describe()));
            }
        }
        else {
            if badge {
                debug.push_str("\nEccezione per medaglia");
            }
            else if (filter.get(1) >= Some(&1) || filter.get(3) == Some(&1)) && !BotPkmn::filter(filter, iv, input.pokemon_level) {
                #[cfg(test)]
                info!("Pokémon discarded for IV-Level config: pokemon_id {} iv {:?} level {:?}", pokemon_id, iv, input.pokemon_level);

                return Err(());
            }
            else {
                debug.push_str(&format!("\nFiltro orario attivo e {}", BotPkmn::describe(filter)));
            }
        }

        if !BotPkmn::advanced_filters(filter, input) {
            #[cfg(test)]
            info!("Pokémon discarded for Advanced Filters config");

            return Err(());
        }

        Ok(PokemonMessage {
            pokemon: input.clone(),
            iv,
            distance: BotLocs::calc_dist(&self.locs.h, pos)?,
            direction: BotLocs::get_direction(&self.locs.h, pos)?,
            debug: if self.debug == Some(true) { Some(debug) } else { None },
        })
    }

    fn submit_raid(&self, now: &DateTime<Local>, input: &Raid) -> Result<RaidMessage, ()> {
        let pokemon_id = input.pokemon_id.and_then(|id| if id > 0 { Some(id.to_string()) } else { None });
        let loc = self.locs.get_raid_settings()?;
        let pos = (input.latitude, input.longitude);
        if self.raid.x != Some(1) || !input.ex_raid_eligible {
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
        let rad = MAX_DISTANCE.min(BotLocs::convert_to_f64(&self.locs.r[2])?).max(0.1);

        let mut debug = format!("Scansione avvenuta alle {}\n", now.format("%T").to_string());
        let dist = BotLocs::calc_dist(loc, pos)?;
        if dist > rad {
            #[cfg(test)]
            info!("Raid discarded for distance: loc {:?} pos {:?} dist {} rad {}", loc, pos, dist, rad);

            return Err(());
        }
        else {
            debug.push_str(&format!("Distanza per Raid inferiore a {:.2} km ({:.2} km)", rad, dist));
        }

        if !self.time.is_active()? {
            #[cfg(test)]
            info!("Raid discarded for time config");

            return Err(());
        }

        if self.raid.x == Some(1) && input.ex_raid_eligible {
            debug.push_str(&"\nBypass Palestre EX abilitato");
        }
        else {
            match input.pokemon_id {
                Some(pkmn_id) if pkmn_id > 0 => {
                    if !self.raid.p.contains(&pkmn_id) {
                        #[cfg(test)]
                        info!("Raid discarded for disabled raidboss: raidboss {} config {:?}", pkmn_id, self.raid.p);

                        return Err(());
                    }
                    else {
                        debug.push_str(&"\nPokémon presente nella lista raidboss abilitati");
                    }
                },
                _ => {
                    if !self.raid.l.contains(&input.level) {
                        #[cfg(test)]
                        info!("Raid discarded for disabled egg level: level {} config {:?}", input.level, self.raid.l);

                        return Err(());
                    }
                    else {
                        debug.push_str(&"\nLivello uovo abilitato");
                    }
                },
            }
        }

        Ok(RaidMessage {
            raid: input.clone(),
            distance: BotLocs::calc_dist(&self.locs.h, pos)?,
            debug: if self.debug == Some(true) { Some(debug) } else { None },
        })
    }

    fn submit_quest(&self, now: &DateTime<Local>, input: &Quest) -> Result<QuestMessage, ()> {
        self._submit_quest(now, input).map(|debug| {
            QuestMessage {
                quest: input.clone(),
                debug: if self.debug == Some(true) { Some(debug) } else { None },
            }
        })
    }

    fn _submit_quest(&self, now: &DateTime<Local>, input: &Quest) -> Result<String, ()> {
        let qest = self.qest.as_ref().ok_or_else(|| ())?;
        if qest.n == 0 {
            return Err(());
        }

        let loc = self.locs.get_invs_settings()?;
        let pos = (input.latitude, input.longitude);

        let rad = MAX_DISTANCE.min(BotLocs::convert_to_f64(&loc[2])?).max(0.1);

        let mut debug = format!("Scansione avvenuta alle {}\n", now.format("%T").to_string());
        let dist = BotLocs::calc_dist(loc, pos)?;
        if dist > rad {
            return Err(());
        }
        else {
            debug.push_str(&format!("Distanza per Pokéstop inferiore a {:.2} km ({:.2} km)", rad, dist));
        }

        for s in &qest.l {
            let parts: Vec<&str> = s.split('-').collect();
            match parts.len() {
                3 => {
                    if input.template == parts[1] && input.target.to_string().as_str() == parts[2] {
                        return Ok(debug);
                    }
                },
                2 => {
                    // lures, invasions, etc...
                    // not managed by the bot
                },
                _ => {
                    let group: Vec<&str> = parts[0].split('/').collect();
                    match group[0] {
                        "pkmns" => {
                            // [{"type":7,"info":{"gender_id":0,"shiny":false,"costume_id":0,"pokemon_id":215,"form_id":0}}]
                            if input.rewards[0]["type"].as_u64() == Some(7) && input.rewards[0]["info"]["pokemon_id"].as_u64().map(|i| i.to_string()).as_ref().map(|s| s.as_str()) == Some(group[2]) {
                                return Ok(debug);
                            }
                        },
                        "items" => {
                            if parts[1] == "0" {
                                if input.rewards[0]["type"].as_u64() == Some(3) {
                                    return Ok(debug);
                                }
                            }
                            else {
                                if input.rewards[0]["type"].as_u64() == Some(2) && input.rewards[0]["info"]["item_id"].as_u64().map(|i| i.to_string()).as_ref().map(|s| s.as_str()) == Some(group[1]) {
                                    return Ok(debug);
                                }
                            }
                        },
                        _ => {},
                    }
                }
            }
        }

        Err(())
    }

    fn submit_invasion(&self, now: &DateTime<Local>, input: &Pokestop) -> Result<InvasionMessage, ()> {
        let invs = self.invs.as_ref().ok_or_else(|| ())?;
        if invs.n == 0 {
            return Err(());
        }

        let loc = self.locs.get_invs_settings()?;
        let pos = (input.latitude, input.longitude);

        let rad = MAX_DISTANCE.min(BotLocs::convert_to_f64(&loc[2])?).max(0.1);

        let mut debug = format!("Scansione avvenuta alle {}\n", now.format("%T").to_string());
        let dist = BotLocs::calc_dist(loc, pos)?;
        if dist > rad {
            return Err(());
        }
        else {
            debug.push_str(&format!("Distanza per Invasioni inferiore a {:.2} km ({:.2} km)", rad, dist));
        }

        if invs.f == 1 {
            if !invs.l.contains(input.grunt_type.as_ref().ok_or_else(|| ())?) {
                return Err(());
            }
            else {
                debug.push_str("\nScagnozzo presente nella lista degli scagnozzi abilitati");
            }
        }
        else {
            debug.push_str("\nNessun filtro scagnozzi attivo");
        }

        Ok(InvasionMessage {
            invasion: input.clone(),
            debug: if self.debug == Some(true) { Some(debug) } else { None },
        })
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
                }
                else {
                    s.parse().map_err(|e| error!("json value convert_to_i64 error: {}", e))
                }
            },
            JsonValue::Number(n) => n.as_i64().ok_or_else(|| error!("json value convert_to_i64 error: json element isn't an integer")),
            _ => {
                error!("json value convert_to_i64 format not recognized: {:?}", input);
                Err(())
            },
        }
    }

    fn convert_to_f64(input: &JsonValue) -> Result<f64, ()> {
        match input {
            JsonValue::String(s) => {
                if s.is_empty() {
                    Err(())
                }
                else {
                    s.parse().map_err(|e| error!("json value convert_to_f64 error: {}", e))
                }
            },
            JsonValue::Number(n) => n.as_f64().ok_or_else(|| error!("json value convert_to_f64 error: json element isn't a float")),
            _ => {
                error!("json value convert_to_f64 format not recognized: {:?}", input);
                Err(())
            },
        }
    }

    fn get_pokemon_settings(&self) -> Result<&Vec<JsonValue>, ()> {
        if Self::convert_to_i64(&self.t_p[2]).map(|i| i > Local::now().timestamp()) == Ok(true) {
            Ok(&self.t_p)
        }
        else {
            Ok(&self.p)
        }
    }

    fn get_raid_settings(&self) -> Result<&Vec<JsonValue>, ()> {
        if Self::convert_to_i64(&self.t_r[2]).map(|i| i > Local::now().timestamp()) == Ok(true) {
            Ok(&self.t_r)
        }
        else {
            Ok(&self.r)
        }
    }

    fn get_invs_settings(&self) -> Result<&Vec<JsonValue>, ()> {
        match self.t_i {
            Some(ref t_i) if !t_i[2].is_null() && Self::convert_to_i64(&t_i[2]).map(|i| i > Local::now().timestamp()) == Ok(true) => Ok(t_i),
            _ => self.i.as_ref().ok_or_else(|| ())
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
        let angle = 2f64 * ((lat_delta / 2f64).sin().powi(2) + lat_from.cos() * lat_to.cos() * (lon_delta / 2f64).sin().powi(2)).sqrt().asin();
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
        let d_phi = ((pos.0.to_radians() / 2f64 + PI / 4f64).tan() / (lat1.to_radians() / 2f64 + PI / 4f64).tan()).ln();

        if d_lon.abs() > PI {
            if d_lon > 0f64 {
                // $dLon = (2 * pi() - $dLon) * -1;
                d_lon = (2f64 * PI - d_lon) * -1f64;
            }
            else {
                // $dLon = 2 * pi() + $dLon;
                d_lon += 2f64 * PI;
            }
        }

        // $tmp = round(((rad2deg(atan2($dLon, $dPhi)) + 360) % 360) / 45);
        let tmp = (((d_lon.atan2(d_phi).to_degrees() + 360f64) % 360f64) / 45f64).round() as usize;
        Ok(match tmp {
            1 => String::from_utf8(vec![0xe2, 0x86, 0x97, 0xef, 0xb8, 0x8f]),
            2 => String::from_utf8(vec![0xe2, 0x9e, 0xa1, 0xef, 0xb8, 0x8f]),
            3 => String::from_utf8(vec![0xe2, 0x86, 0x98, 0xef, 0xb8, 0x8f]),
            4 => String::from_utf8(vec![0xe2, 0xac, 0x87, 0xef, 0xb8, 0x8f]),
            5 => String::from_utf8(vec![0xe2, 0x86, 0x99, 0xef, 0xb8, 0x8f]),
            6 => String::from_utf8(vec![0xe2, 0xac, 0x85, 0xef, 0xb8, 0x8f]),
            7 => String::from_utf8(vec![0xe2, 0x86, 0x96, 0xef, 0xb8, 0x8f]),
            _ => String::from_utf8(vec![0xe2, 0xac, 0x86, 0xef, 0xb8, 0x8f]),
        }.map_err(|e| error!("direction gliph error: {}", e))?)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotRaid {
    pub u: u8,
    pub s: u8,
    pub l: Vec<u8>,
    pub p: Vec<u16>,
    pub x: Option<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotPkmn {
    pub l: HashMap<String, Vec<u8>>,
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
     */
    fn describe(filter: &[u8]) -> String {
        if filter.get(1) >= Some(&1) && filter.get(3) == Some(&1) { // IV e PL attivi
            format!("IV >= {} {} LVL >= {}",
                filter.get(2).unwrap_or_else(|| &0),
                if filter.get(7) == Some(&1) { "O" } else { "E" },
                filter.get(4).unwrap_or_else(|| &0))
        }
        else {
            if filter.get(1) >= Some(&1) {
                format!("IV >= {}", filter.get(2).unwrap_or_else(|| &0))
            }
            else if filter.get(3) == Some(&1) {
                format!("LVL >= {}", filter.get(4).unwrap_or_else(|| &0))
            }
            else {
                String::from("nessun filtro IV/LVL attivo")
            }
        }
    }

    fn filter(filter: &[u8], iv: Option<f32>, lvl: Option<u8>) -> bool {
        if filter.get(1) >= Some(&1) && filter.get(3) == Some(&1) { // IV e PL attivi
            if filter.get(7) == Some(&1) {
                if iv >= filter.get(2).map(|i| f32::from(*i)).or_else(|| Some(0_f32)) {
                    return true;
                }
                if lvl.as_ref() >= filter.get(4).or_else(|| Some(&0)) {
                    return true;
                }
                false
            }
            else {
                if iv.is_some() {
                    if iv < filter.get(2).map(|i| f32::from(*i)) {
                        return false;
                    }
                }
                else {
                    return false;
                }
                if lvl.is_some() {
                    if lvl.as_ref() < filter.get(4) {
                        return false;
                    }
                }
                else {
                    return false;
                }
                true
            }
        }
        else {
            if filter.get(1) >= Some(&1) {
                if iv.is_some() {
                    if iv >= filter.get(2).map(|i| f32::from(*i)) {
                        return true;
                    }
                }
            }
            if filter.get(3) == Some(&1) {
                if lvl.is_some() {
                    if lvl.as_ref() >= filter.get(4) {
                        return true;
                    }
                }
            }
            false
        }
    }

    fn check_badge(filter: &[u8], input: &Box<Pokemon>) -> bool {
        if filter.get(8) == Some(&1) {
            match input.pokemon_id {
                // rattata
                19 => {
                    match (input.height, input.weight) {
                        (Some(h), Some(w)) => (h/0.3f64 + w/3.5f64) < 1.5f64,
                        _ => false,
                    }
                },
                // magikarp
                129 => {
                    match (input.height, input.weight) {
                        (Some(h), Some(w)) => (h/0.9f64 + w/10f64) > 2.5f64,
                        _ => false,
                    }
                },
                _ => false,
            }
        }
        else {
            false
        }
    }

    /*
     * 9: gender (1: Male, 2: Female)
     * 10: atk filter (1: <, 2: =, 3: >)
     * 11: atk value
     * 12: def filter (1: <, 2: =, 3: >)
     * 13: def value
     * 14: sta filter (1: <, 2: =, 3: >)
     * 15: sta value
     */
    fn advanced_filters(filter: &[u8], input: &Box<Pokemon>) -> bool {
        match filter.get(9) {
            Some(&1) => if input.gender != Gender::Male {
                return false;
            },
            Some(&2) => if input.gender != Gender::Female {
                return false;
            },
            _ => {},
        }

        fn filter_iv(filter: Option<&u8>, filter_value: Option<&u8>, value: Option<&u8>) -> bool {
            match filter {
                Some(&1) => value < filter_value,
                Some(&2) => value == filter_value,
                Some(&3) => value > filter_value,
                _ => true,
            }
        }

        filter_iv(filter.get(10), filter.get(11), input.individual_attack.as_ref()) &&
            filter_iv(filter.get(12), filter.get(13), input.individual_defense.as_ref()) &&
            filter_iv(filter.get(14), filter.get(15), input.individual_stamina.as_ref())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotTime {
    pub fi: [u8; 2],
    pub fl: [u8; 2],
    pub fc: u8,
    pub w1: Vec<u8>,
    pub w2: Vec<u8>,
}

impl BotTime {
    fn is_active(&self) -> Result<bool, ()> {
        let now = Local::now();
        let hour: u8 = now.format("%H").to_string().parse().map_err(|e| error!("current hour retrieve error: {}", e))?;
        Ok(match now.format("%w").to_string().as_str() {
            "0" | "6" => self.w2.contains(&hour),
            _ => self.w1.contains(&hour),
        })
    }

    fn describe(&self) -> String {
        if self.fi[0] == 1 && self.fl[0] == 1 {
            format!("IV >= {} {} LVL >= {}", self.fi[1], if self.fc == 1 { "O" } else { "E" }, self.fl[1])
        }
        else {
            if self.fi[0] == 1 {
                format!("IV >= {}", self.fi[1])
            }
            else if self.fl[0] == 1 {
                format!("LVL >= {}", self.fl[1])
            }
            else {
                String::from("nessun filtro IV/LVL attivo")
            }
        }
    }

    fn bypass(&self, iv: Option<f32>, lvl: Option<u8>) -> bool {
        if self.fi[0] == 1 && self.fl[0] == 1 {
            if self.fc == 1 {
                if let Some(i) = iv {
                    if i >= f32::from(self.fi[1]) {
                        #[cfg(test)]
                        info!("Pokémon approved because of fi[0] == 1 && fl[0] == 1 && fc == 1 && IV >= fi[1]");

                        return true;
                    }
                }
                if let Some(i) = lvl {
                    if i >= self.fl[1] {
                        #[cfg(test)]
                        info!("Pokémon approved because of fi[0] == 1 && fl[0] == 1 && fc == 1 && LVL >= fl[1]");

                        return true;
                    }
                }

                #[cfg(test)]
                info!("Pokémon discarded because of fi[0] == 1 && fl[0] == 1 && fc == 1 && IV < fi[1] && LVL < fl[1]");

                false
            }
            else {
                if let Some(i) = iv {
                    if i < f32::from(self.fi[1]) {
                        #[cfg(test)]
                        info!("Pokémon discarded because of fi[0] == 1 && fl[0] == 1 && fc != 1 && IV < fi[1]");

                        return false;
                    }
                }
                else {
                    return false;
                }
                if let Some(i) = lvl {
                    if i < self.fl[1] {
                        #[cfg(test)]
                        info!("Pokémon discarded because of fi[0] == 1 && fl[0] == 1 && fc != 1 && LVL < fl[1]");

                        return false;
                    }
                }
                else {
                    return false;
                }

                #[cfg(test)]
                info!("Pokémon approved because of fi[0] == 1 && fl[0] == 1 && fc != 1 && IV IS NULL && LVL IS NULL");

                true
            }
        }
        else {
            if self.fi[0] == 1 {
                if let Some(i) = iv {
                    if i >= f32::from(self.fi[1]) {
                        #[cfg(test)]
                        info!("Pokémon approved because of fi[0] == 1 && fl[0] != 1 && IV >= fi[1]");

                        return true;
                    }
                }
            }
            if self.fl[0] == 1 {
                if let Some(i) = lvl {
                    if i >= self.fl[1] {
                        #[cfg(test)]
                        info!("Pokémon approved because of fi[0] != 1 && fl[0] == 1 && LVL >= fl[1]");

                        return true;
                    }
                }
            }

            #[cfg(test)]
            info!("Pokémon discarded because of (fi[0] != 1 || fl[0] != 1) && IV IS NULL && LVL IS NULL");

            false
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
pub struct BotInvs {
    pub n: u8,
    pub f: u8,
    pub l: Vec<u8>,
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
    // use chrono::Local;

    use super::BotConfig;

    // use crate::entities::{Request, Pokemon};

    #[test]
    fn bot_config() {
        let data = [
            r#"{"locs":{"h":["45.653978","8.787760"],"p":["45.653968","8.787805","9"],"r":["45.655640","8.788785","1"],"t_p":["0","0","0"],"t_r":["0","0","0"]},"raid":{"u":1,"s":1,"l":[5],"p":[403,303,26,359,105,488]},"pkmn":{"l":{"1":[1,1,86,0,25,0,10,1],"2":[1,1,86,0,25,0,10,1],"3":[1,1,86,0,25,0,10,1],"4":[1,1,86,0,25,0,10,1],"5":[1,1,86,0,25,0,10,1],"6":[1,1,86,0,25,0,10,1],"7":[1,1,86,0,25,0,10,1],"8":[1,1,86,0,25,0,10,1],"9":[1,1,86,0,25,0,10,1],"10":[1,2,100,0,25,0,10,1],"11":[1,1,86,0,25,0,10,1],"12":[1,1,86,0,25,0,10,1],"13":[1,2,100,0,25,0,10,1],"14":[1,1,86,0,25,0,10,1],"15":[1,1,86,0,25,0,10,1],"16":[1,2,100,0,25,0,10,1],"17":[1,1,86,0,25,0,10,1],"18":[1,1,86,0,25,0,10,1],"19":[1,2,100,0,25,0,10,1],"20":[1,1,86,0,25,0,10,1],"21":[1,2,100,0,25,0,10,1],"22":[1,1,86,0,25,0,10,1],"23":[1,2,100,0,25,0,10,1],"24":[1,1,86,0,25,0,10,1],"25":[1,2,100,0,25,0,10,1],"26":[1,1,86,0,25,0,10,1],"27":[1,1,86,0,25,0,10,1],"28":[1,1,86,0,25,0,10,1],"29":[1,2,100,0,25,0,10,1],"30":[1,1,86,0,25,0,10,1],"31":[1,1,86,0,25,0,10,1],"32":[1,2,100,0,25,0,10,1],"33":[1,1,86,0,25,0,10,1],"34":[1,1,86,0,25,0,10,1],"35":[1,1,86,0,25,0,10,1],"36":[1,1,86,0,25,0,10,1],"37":[1,2,100,0,25,0,10,1],"38":[1,1,86,0,25,0,10,1],"39":[1,1,86,0,25,0,10,1],"40":[1,1,86,0,25,0,10,1],"41":[1,2,100,0,25,0,10,1],"42":[1,1,86,0,25,0,10,1],"43":[1,1,86,0,25,0,10,1],"44":[1,1,86,0,25,0,10,1],"45":[1,1,86,0,25,0,10,1],"46":[1,2,100,0,25,0,10,1],"47":[1,1,86,0,25,0,10,1],"48":[1,2,100,0,25,0,10,1],"49":[1,1,86,0,25,0,10,1],"50":[1,1,86,0,25,0,10,1],"51":[1,1,86,0,25,0,10,1],"52":[1,1,86,0,25,0,10,1],"53":[1,1,86,0,25,0,10,1],"54":[1,1,86,0,25,0,10,1],"55":[1,1,86,0,25,0,10,1],"56":[1,1,86,0,25,0,10,1],"57":[1,1,86,0,25,0,10,1],"58":[1,1,86,0,25,0,10,1],"59":[1,1,86,0,25,0,10,1],"60":[1,1,86,0,25,0,10,1],"61":[1,1,86,0,25,0,10,1],"62":[1,1,86,0,25,0,10,1],"63":[1,1,86,0,25,0,10,1],"64":[1,1,86,0,25,0,10,1],"65":[1,1,86,0,25,0,10,1],"66":[1,1,86,0,25,0,10,1],"67":[1,1,86,0,25,0,10,1],"68":[1,1,86,0,25,0,10,1],"69":[1,1,86,0,25,0,10,1],"70":[1,1,86,0,25,0,10,1],"71":[1,1,86,0,25,0,10,1],"72":[1,1,86,0,25,0,10,1],"73":[1,1,86,0,25,0,10,1],"74":[1,2,100,0,25,0,10,1],"75":[1,1,86,0,25,0,10,1],"76":[1,1,86,0,25,0,10,1],"77":[1,1,86,0,25,0,10,1],"78":[1,1,86,0,25,0,10,1],"79":[1,1,86,0,25,0,10,1],"80":[1,1,86,0,25,0,10,1],"81":[1,1,86,0,25,0,10,1],"82":[1,1,86,0,25,0,10,1],"83":[1,1,86,0,25,0,10,1],"84":[1,1,86,0,25,0,10,1],"85":[1,1,86,0,25,0,10,1],"86":[1,1,86,0,25,0,10,1],"87":[1,1,86,0,25,0,10,1],"88":[1,1,86,0,25,0,10,1],"89":[1,1,86,0,25,0,10,1],"90":[1,2,100,0,25,0,10,1],"91":[1,1,86,0,25,0,10,1],"92":[1,1,86,0,25,0,10,1],"93":[1,1,86,0,25,0,10,1],"94":[1,1,86,0,25,0,10,1],"95":[1,1,86,0,25,0,10,1],"96":[1,1,86,0,25,0,10,1],"97":[1,1,86,0,25,0,10,1],"98":[1,1,86,0,25,0,10,1],"99":[1,1,86,0,25,0,10,1],"100":[1,1,86,0,25,0,10,1],"101":[1,1,86,0,25,0,10,1],"102":[1,1,86,0,25,0,10,1],"103":[1,1,86,0,25,0,10,1],"104":[1,0,86,0,25,0,10,1],"105":[1,1,86,0,25,0,10,1],"106":[1,1,86,0,25,0,10,1],"107":[1,1,86,0,25,0,10,1],"108":[1,1,86,0,25,0,10,1],"109":[1,1,86,0,25,0,10,1],"110":[1,1,86,0,25,0,10,1],"111":[1,1,86,0,25,0,10,1],"112":[1,1,86,0,25,0,10,1],"113":[1,1,86,0,25,0,10,1],"114":[1,1,86,0,25,0,10,1],"115":[1,1,86,0,25,0,10,1],"116":[1,1,86,0,25,0,10,1],"117":[1,1,86,0,25,0,10,1],"118":[1,1,86,0,25,0,10,1],"119":[1,1,86,0,25,0,10,1],"120":[1,1,86,0,25,0,10,1],"121":[1,1,86,0,25,0,10,1],"122":[1,1,86,0,25,0,10,1],"123":[1,1,86,0,25,0,10,1],"124":[1,1,86,0,25,0,10,1],"125":[1,1,86,0,25,0,10,1],"126":[1,1,86,0,25,0,10,1],"127":[1,1,86,0,25,0,10,1],"128":[1,1,86,0,25,0,10,1],"129":[1,2,100,0,25,0,10,1],"130":[1,1,86,0,25,0,10,1],"131":[1,1,86,0,25,0,10,1],"132":[1,1,86,0,25,0,10,1],"133":[1,2,100,0,25,0,10,1],"134":[1,1,86,0,25,0,10,1],"135":[1,1,86,0,25,0,10,1],"136":[1,1,86,0,25,0,10,1],"137":[1,1,86,0,25,0,10,1],"138":[1,1,86,0,25,0,10,1],"139":[1,1,86,0,25,0,10,1],"140":[1,1,86,0,25,0,10,1],"141":[1,1,86,0,25,0,10,1],"142":[1,1,86,0,25,0,10,1],"143":[1,1,86,0,25,0,10,1],"147":[1,1,86,0,25,0,10,1],"148":[1,1,86,0,25,0,10,1],"149":[1,1,86,0,25,0,10,1],"152":[1,2,100,0,25,0,10,1],"153":[1,1,86,0,25,0,10,1],"154":[1,1,86,0,25,0,10,1],"155":[1,2,100,0,25,0,10,1],"156":[1,1,86,0,25,0,10,1],"157":[1,1,86,0,25,0,10,1],"158":[1,2,100,0,25,0,10,1],"159":[1,1,86,0,25,0,10,1],"160":[1,1,86,0,25,0,10,1],"161":[1,2,100,0,25,0,10,1],"162":[1,1,86,0,25,0,10,1],"163":[1,2,100,0,25,0,10,1],"164":[1,1,86,0,25,0,10,1],"165":[1,2,100,0,25,0,10,1],"166":[1,1,86,0,25,0,10,1],"167":[1,2,100,0,25,0,10,1],"168":[1,1,86,0,25,0,10,1],"169":[1,1,86,0,25,0,10,1],"170":[1,1,86,0,25,0,10,1],"171":[1,1,86,0,25,0,10,1],"176":[1,1,86,0,25,0,10,1],"177":[1,2,100,0,25,0,10,1],"178":[1,1,86,0,25,0,10,1],"179":[1,1,86,0,25,0,10,1],"180":[1,1,86,0,25,0,10,1],"181":[1,1,86,0,25,0,10,1],"183":[1,1,86,0,25,0,10,1],"184":[1,1,86,0,25,0,10,1],"185":[1,1,86,0,25,0,10,1],"187":[1,2,100,0,25,0,10,1],"188":[1,1,86,0,25,0,10,1],"189":[1,1,86,0,25,0,10,1],"190":[1,2,100,0,25,0,10,1],"191":[1,1,86,0,25,0,10,1],"193":[1,1,86,0,25,0,10,1],"194":[1,2,100,0,25,0,10,1],"195":[1,1,86,0,25,0,10,1],"198":[1,2,100,0,25,0,10,1],"200":[1,1,86,0,25,0,10,1],"201":[1,1,86,0,25,0,10,1],"202":[1,1,86,0,25,0,10,1],"203":[1,1,86,0,25,0,10,1],"204":[1,1,86,0,25,0,10,1],"205":[1,1,86,0,25,0,10,1],"206":[1,1,86,0,25,0,10,1],"207":[1,1,86,0,25,0,10,1],"209":[1,1,86,0,25,0,10,1],"210":[1,1,86,0,25,0,10,1],"211":[1,1,86,0,25,0,10,1],"213":[1,1,86,0,25,0,10,1],"214":[1,1,86,0,25,0,10,1],"215":[1,1,86,0,25,0,10,1],"216":[1,2,100,0,25,0,10,1],"217":[1,1,86,0,25,0,10,1],"218":[1,1,86,0,25,0,10,1],"219":[1,1,86,0,25,0,10,1],"220":[1,1,86,0,25,0,10,1],"221":[1,1,86,0,25,0,10,1],"222":[1,1,86,0,25,0,10,1],"223":[1,1,86,0,25,0,10,1],"224":[1,1,86,0,25,0,10,1],"225":[1,2,100,0,25,0,10,1],"226":[1,1,86,0,25,0,10,1],"227":[1,1,86,0,25,0,10,1],"228":[1,1,86,0,25,0,10,1],"229":[1,1,86,0,25,0,10,1],"231":[1,1,86,0,25,0,10,1],"232":[1,1,86,0,25,0,10,1],"233":[1,1,86,0,25,0,10,1],"234":[1,1,86,0,25,0,10,1],"237":[1,1,86,0,25,0,10,1],"241":[1,1,86,0,25,0,10,1],"242":[1,1,86,0,25,0,10,1],"246":[1,1,86,0,25,0,10,1],"247":[1,1,86,0,25,0,10,1],"248":[1,1,86,0,25,0,10,1],"252":[1,2,100,0,25,0,10,1],"253":[1,1,86,0,25,0,10,1],"254":[1,1,86,0,25,0,10,1],"255":[1,2,100,0,25,0,10,1],"256":[1,1,86,0,25,0,10,1],"257":[1,1,86,0,25,0,10,1],"258":[1,2,100,0,25,0,10,1],"259":[1,1,86,0,25,0,10,1],"260":[1,1,86,0,25,0,10,1],"261":[1,2,100,0,25,0,10,1],"262":[1,1,86,0,25,0,10,1],"263":[1,2,100,0,25,0,10,1],"264":[1,1,86,0,25,0,10,1],"265":[1,2,100,0,25,0,10,1],"266":[1,1,86,0,25,0,10,1],"267":[1,1,86,0,25,0,10,1],"268":[1,1,86,0,25,0,10,1],"269":[1,1,86,0,25,0,10,1],"270":[1,1,86,0,25,0,10,1],"271":[1,1,86,0,25,0,10,1],"272":[1,1,86,0,25,0,10,1],"273":[1,2,100,0,25,0,10,1],"274":[1,1,86,0,25,0,10,1],"275":[1,1,86,0,25,0,10,1],"276":[1,1,86,0,25,0,10,1],"277":[1,1,86,0,25,0,10,1],"278":[1,1,86,0,25,0,10,1],"279":[1,1,86,0,25,0,10,1],"280":[1,2,100,0,25,0,10,1],"281":[1,1,86,0,25,0,10,1],"282":[1,1,86,0,25,0,10,1],"283":[1,1,86,0,25,0,10,1],"284":[1,1,86,0,25,0,10,1],"285":[1,2,100,0,25,0,10,1],"286":[1,1,86,0,25,0,10,1],"287":[1,1,86,0,25,0,10,1],"288":[1,1,86,0,25,0,10,1],"289":[1,1,86,0,25,0,10,1],"290":[1,1,86,0,25,0,10,1],"291":[1,1,86,0,25,0,10,1],"292":[1,1,86,0,25,0,10,1],"293":[1,2,100,0,25,0,10,1],"294":[1,1,86,0,25,0,10,1],"295":[1,1,86,0,25,0,10,1],"296":[1,2,100,0,25,0,10,1],"297":[1,1,86,0,25,0,10,1],"298":[1,1,86,0,25,0,10,1],"299":[1,2,100,0,25,0,10,1],"300":[1,2,100,0,25,0,10,1],"301":[1,1,86,0,25,0,10,1],"302":[1,2,100,0,25,0,10,1],"303":[1,1,86,0,25,0,10,1],"304":[1,2,100,0,25,0,10,1],"305":[1,1,86,0,25,0,10,1],"306":[1,1,86,0,25,0,10,1],"307":[1,2,100,0,25,0,10,1],"308":[1,1,86,0,25,0,10,1],"309":[1,2,100,0,25,0,10,1],"310":[1,1,86,0,25,0,10,1],"311":[1,1,86,0,25,0,10,1],"312":[1,2,100,0,25,0,10,1],"313":[1,1,86,0,25,0,10,1],"314":[1,1,86,0,25,0,10,1],"315":[1,2,100,0,25,0,10,1],"316":[1,2,100,0,25,0,10,1],"317":[1,1,86,0,25,0,10,1],"318":[1,1,86,0,25,0,10,1],"319":[1,1,86,0,25,0,10,1],"320":[1,2,100,0,25,0,10,1],"321":[1,1,86,0,25,0,10,1],"322":[1,2,100,0,25,0,10,1],"323":[1,1,86,0,25,0,10,1],"324":[1,1,86,0,25,0,10,1],"325":[1,2,100,0,25,0,10,1],"326":[1,1,86,0,25,0,10,1],"327":[1,1,86,0,25,0,10,1],"328":[1,1,86,0,25,0,10,1],"329":[1,1,86,0,25,0,10,1],"330":[1,1,86,0,25,0,10,1],"331":[1,2,100,0,25,0,10,1],"332":[1,1,86,0,25,0,10,1],"333":[1,1,86,0,25,0,10,1],"334":[1,1,86,0,25,0,10,1],"335":[1,2,100,0,25,0,10,1],"336":[1,2,100,0,25,0,10,1],"337":[1,1,86,0,25,0,10,1],"338":[1,1,86,0,25,0,10,1],"339":[1,2,100,0,25,0,10,1],"340":[1,1,86,0,25,0,10,1],"341":[1,1,86,0,25,0,10,1],"342":[1,1,86,0,25,0,10,1],"343":[1,2,100,0,25,0,10,1],"344":[1,1,86,0,25,0,10,1],"345":[1,1,86,0,25,0,10,1],"346":[1,1,86,0,25,0,10,1],"347":[1,1,86,0,25,0,10,1],"348":[1,1,86,0,25,0,10,1],"349":[1,1,86,0,25,0,10,1],"350":[1,1,86,0,25,0,10,1],"351":[1,1,86,0,25,0,10,1],"352":[1,1,86,0,25,0,10,1],"353":[1,2,100,0,25,0,10,1],"354":[1,1,86,0,25,0,10,1],"355":[1,2,100,0,25,0,10,1],"356":[1,1,86,0,25,0,10,1],"357":[1,1,86,0,25,0,10,1],"358":[1,1,86,0,25,0,10,1],"359":[1,1,86,0,25,0,10,1],"360":[1,1,86,0,25,0,10,1],"361":[1,1,86,0,25,0,10,1],"362":[1,1,86,0,25,0,10,1],"363":[1,2,100,0,25,0,10,1],"364":[1,1,86,0,25,0,10,1],"365":[1,1,86,0,25,0,10,1],"366":[1,1,86,0,25,0,10,1],"367":[1,1,86,0,25,0,10,1],"368":[1,1,86,0,25,0,10,1],"369":[1,1,86,0,25,0,10,1],"370":[1,1,86,0,25,0,10,1],"371":[1,1,86,0,25,0,10,1],"372":[1,1,86,0,25,0,10,1],"373":[1,1,86,0,25,0,10,1],"374":[1,1,86,0,25,0,10,1],"375":[1,1,86,0,25,0,10,1],"376":[1,1,86,0,25,0,10,1],"387":[1,1,86,0,25,0,10,1],"388":[1,1,86,0,25,0,10,1],"389":[1,1,86,0,25,0,10,1],"390":[1,1,86,0,25,0,10,1],"391":[1,1,86,0,25,0,10,1],"392":[1,1,86,0,25,0,10,1],"393":[1,1,86,0,25,0,10,1],"394":[1,1,86,0,25,0,10,1],"395":[1,1,86,0,25,0,10,1],"396":[1,1,86,0,25,0,10,1],"397":[1,1,86,0,25,0,10,1],"398":[1,1,86,0,25,0,10,1],"399":[1,1,86,0,25,0,10,1],"400":[1,1,86,0,25,0,10,1],"401":[1,1,86,0,25,0,10,1],"402":[1,1,86,0,25,0,10,1],"403":[1,1,86,0,25,0,10,1],"404":[1,1,86,0,25,0,10,1],"405":[1,1,86,0,25,0,10,1],"406":[1,1,86,0,25,0,10,1],"407":[1,1,86,0,25,0,10,1],"408":[1,1,86,0,25,0,10,1],"409":[1,1,86,0,25,0,10,1],"410":[1,1,86,0,25,0,10,1],"411":[1,1,86,0,25,0,10,1],"412":[1,1,86,0,25,0,10,1],"413":[1,1,86,0,25,0,10,1],"414":[1,1,86,0,25,0,10,1],"415":[1,1,86,0,25,0,10,1],"416":[1,1,86,0,25,0,10,1],"417":[1,1,86,0,25,0,10,1],"418":[1,1,86,0,25,0,10,1],"419":[1,1,86,0,25,0,10,1],"420":[1,1,86,0,25,0,10,1],"421":[1,1,86,0,25,0,10,1],"422":[1,1,86,0,25,0,10,1],"423":[1,1,86,0,25,0,10,1],"424":[1,1,86,0,25,0,10,1],"425":[1,1,86,0,25,0,10,1],"426":[1,1,86,0,25,0,10,1],"427":[1,1,86,0,25,0,10,1],"428":[1,1,86,0,25,0,10,1],"429":[1,1,86,0,25,0,10,1],"430":[1,1,86,0,25,0,10,1],"431":[1,1,86,0,25,0,10,1],"432":[1,1,86,0,25,0,10,1],"433":[1,1,86,0,25,0,10,1],"434":[1,1,86,0,25,0,10,1],"435":[1,1,86,0,25,0,10,1],"436":[1,1,86,0,25,0,10,1],"437":[1,1,86,0,25,0,10,1],"438":[1,1,86,0,25,0,10,1],"439":[1,1,86,0,25,0,10,1],"440":[1,1,86,0,25,0,10,1],"441":[1,1,86,0,25,0,10,1],"442":[1,1,86,0,25,0,10,1],"443":[1,1,86,0,25,0,10,1],"444":[1,1,86,0,25,0,10,1],"445":[1,1,86,0,25,0,10,1],"446":[1,1,86,0,25,0,10,1],"447":[1,1,86,0,25,0,10,1],"448":[1,1,86,0,25,0,10,1],"449":[1,1,86,0,25,0,10,1],"450":[1,1,86,0,25,0,10,1],"451":[1,1,86,0,25,0,10,1],"452":[1,1,86,0,25,0,10,1],"453":[1,1,86,0,25,0,10,1],"454":[1,1,86,0,25,0,10,1],"455":[1,1,86,0,25,0,10,1],"456":[1,1,86,0,25,0,10,1],"457":[1,1,86,0,25,0,10,1],"458":[1,1,86,0,25,0,10,1],"459":[1,1,86,0,25,0,10,1],"460":[1,1,86,0,25,0,10,1],"461":[1,1,86,0,25,0,10,1],"462":[1,1,86,0,25,0,10,1],"463":[1,1,86,0,25,0,10,1],"464":[1,1,86,0,25,0,10,1],"465":[1,1,86,0,25,0,10,1],"466":[1,1,86,0,25,0,10,1],"467":[1,1,86,0,25,0,10,1],"468":[1,1,86,0,25,0,10,1],"469":[1,1,86,0,25,0,10,1],"470":[1,1,86,0,25,0,10,1],"471":[1,1,86,0,25,0,10,1],"472":[1,1,86,0,25,0,10,1],"473":[1,1,86,0,25,0,10,1],"474":[1,1,86,0,25,0,10,1],"475":[1,1,86,0,25,0,10,1],"476":[1,1,86,0,25,0,10,1],"477":[1,1,86,0,25,0,10,1],"478":[1,1,86,0,25,0,10,1],"479":[1,1,86,0,25,0,10,1],"480":[1,1,86,0,25,0,10,1],"481":[1,1,86,0,25,0,10,1],"482":[1,1,86,0,25,0,10,1],"483":[1,1,86,0,25,0,10,1],"484":[1,1,86,0,25,0,10,1],"485":[1,1,86,0,25,0,10,1],"486":[1,1,86,0,25,0,10,1],"487":[1,1,86,0,25,0,10,1],"488":[1,1,86,0,25,0,10,1],"489":[1,1,86,0,25,0,10,1],"490":[1,1,86,0,25,0,10,1],"491":[1,1,86,0,25,0,10,1],"492":[1,1,86,0,25,0,10,1],"493":[1,1,86,0,25,0,10,1]}},"time":{"fi":[0,80],"fl":[0,30],"fc":1,"w1":[0,1,17,18,19,20,21,22,23],"w2":[0,9,10,11,14,15,16,17,18,19,20,21,22,23]},"more":{"l":"g"}}"#,
            r#"{"locs":{"h":["45.557703","12.433947"],"p":["45.551693","12.359082","7"],"r":["45.552174","12.359382","7"],"i":["45.551903","12.359103","7"],"t_p":["0","0","0"],"t_r":["0","0","0"]},"raid":{"u":1,"s":1,"l":[4,5],"p":[149,176,210,131,105,384,386]},"pkmn":{"l":{"1":[0,0,80,0,25,0,10,1],"2":[0,0,80,0,25,0,10,1],"3":[0,0,80,0,25,0,10,1],"4":[0,0,80,0,25,0,10,1],"5":[0,0,80,0,25,0,10,1],"6":[0,0,80,0,25,0,10,1],"7":[0,0,80,0,25,0,10,1],"8":[0,0,80,0,25,0,10,1],"9":[0,0,80,0,25,0,10,1],"10":[0,2,100,0,25,0,10,1],"11":[0,0,80,0,25,0,10,1],"12":[0,0,80,0,25,0,10,1],"13":[0,2,100,0,25,0,10,1],"14":[0,0,80,0,25,0,10,1],"15":[0,0,80,0,25,0,10,1],"16":[0,2,100,0,25,0,10,1],"17":[0,0,80,0,25,0,10,1],"18":[0,0,80,0,25,0,10,1],"19":[0,2,100,0,25,0,10,1],"20":[0,0,80,0,25,0,10,1],"21":[0,2,100,0,25,0,10,1],"22":[0,0,80,0,25,0,10,1],"23":[0,2,100,0,25,0,10,1],"24":[0,0,80,0,25,0,10,1],"25":[0,2,100,0,25,0,10,1],"26":[0,0,80,0,25,0,10,1],"27":[0,0,80,0,25,0,10,1],"28":[0,0,80,0,25,0,10,1],"29":[0,2,100,0,25,0,10,1],"30":[0,0,80,0,25,0,10,1],"31":[0,0,80,0,25,0,10,1],"32":[0,2,100,0,25,0,10,1],"33":[0,0,80,0,25,0,10,1],"34":[0,0,80,0,25,0,10,1],"35":[0,0,80,0,25,0,10,1],"36":[0,0,80,0,25,0,10,1],"37":[0,2,100,0,25,0,10,1],"38":[0,0,80,0,25,0,10,1],"39":[0,0,80,0,25,0,10,1],"40":[0,0,80,0,25,0,10,1],"41":[0,2,100,0,25,0,10,1],"42":[0,0,80,0,25,0,10,1],"43":[0,0,80,0,25,0,10,1],"44":[0,0,80,0,25,0,10,1],"45":[0,0,80,0,25,0,10,1],"46":[0,2,100,0,25,0,10,1],"47":[0,0,80,0,25,0,10,1],"48":[0,2,100,0,25,0,10,1],"49":[0,0,80,0,25,0,10,1],"50":[0,0,80,0,25,0,10,1],"51":[0,0,80,0,25,0,10,1],"52":[0,0,80,0,25,0,10,1],"53":[0,0,80,0,25,0,10,1],"54":[0,0,80,0,25,0,10,1],"55":[0,0,80,0,25,0,10,1],"56":[0,0,80,0,25,0,10,1],"57":[0,0,80,0,25,0,10,1],"58":[0,0,80,0,25,0,10,1],"59":[0,0,80,0,25,0,10,1],"60":[0,0,80,0,25,0,10,1],"61":[0,0,80,0,25,0,10,1],"62":[0,0,80,0,25,0,10,1],"63":[0,0,80,0,25,0,10,1],"64":[0,0,80,0,25,0,10,1],"65":[0,0,80,0,25,0,10,1],"66":[0,0,80,0,25,0,10,1],"67":[0,0,80,0,25,0,10,1],"68":[0,0,80,0,25,0,10,1],"69":[0,0,80,0,25,0,10,1],"70":[0,0,80,0,25,0,10,1],"71":[0,0,80,0,25,0,10,1],"72":[0,0,80,0,25,0,10,1],"73":[0,0,80,0,25,0,10,1],"74":[0,2,100,0,25,0,10,1],"75":[0,0,80,0,25,0,10,1],"76":[0,0,80,0,25,0,10,1],"77":[0,0,80,0,25,0,10,1],"78":[0,0,80,0,25,0,10,1],"79":[0,0,80,0,25,0,10,1],"80":[0,0,80,0,25,0,10,1],"81":[0,0,80,0,25,0,10,1],"82":[0,0,80,0,25,0,10,1],"83":[0,0,80,0,25,0,10,1],"84":[0,0,80,0,25,0,10,1],"85":[0,0,80,0,25,0,10,1],"86":[0,0,80,0,25,0,10,1],"87":[0,0,80,0,25,0,10,1],"88":[0,0,80,0,25,0,10,1],"89":[0,0,80,0,25,0,10,1],"90":[0,2,100,0,25,0,10,1],"91":[0,0,80,0,25,0,10,1],"92":[0,0,80,0,25,0,10,1],"93":[0,0,80,0,25,0,10,1],"94":[0,0,80,0,25,0,10,1],"95":[0,0,80,0,25,0,10,1],"96":[0,0,80,0,25,0,10,1],"97":[0,0,80,0,25,0,10,1],"98":[0,0,80,0,25,0,10,1],"99":[0,0,80,0,25,0,10,1],"100":[0,0,80,0,25,0,10,1],"101":[0,0,80,0,25,0,10,1],"102":[0,0,80,0,25,0,10,1],"103":[0,0,80,0,25,0,10,1],"104":[0,0,80,0,25,0,10,1],"105":[0,0,80,0,25,0,10,1],"106":[0,0,80,0,25,0,10,1],"107":[0,0,80,0,25,0,10,1],"108":[0,0,80,0,25,0,10,1],"109":[0,0,80,0,25,0,10,1],"110":[0,0,80,0,25,0,10,1],"111":[0,0,80,0,25,0,10,1],"112":[0,0,80,0,25,0,10,1],"113":[0,0,80,0,25,0,10,1],"114":[0,0,80,0,25,0,10,1],"115":[0,0,80,0,25,0,10,1],"116":[0,0,80,0,25,0,10,1],"117":[0,0,80,0,25,0,10,1],"118":[0,0,80,0,25,0,10,1],"119":[0,0,80,0,25,0,10,1],"120":[0,0,80,0,25,0,10,1],"121":[0,0,80,0,25,0,10,1],"122":[0,0,80,0,25,0,10,1],"123":[0,0,80,0,25,0,10,1],"124":[0,0,80,0,25,0,10,1],"125":[0,0,80,0,25,0,10,1],"126":[0,0,80,0,25,0,10,1],"127":[0,0,80,0,25,0,10,1],"128":[0,0,80,0,25,0,10,1],"129":[0,2,100,0,25,0,10,1],"130":[0,0,80,0,25,0,10,1],"131":[0,0,80,0,25,0,10,1],"132":[0,0,80,0,25,0,10,1],"133":[0,2,100,0,25,0,10,1],"134":[0,0,80,0,25,0,10,1],"135":[0,0,80,0,25,0,10,1],"136":[0,0,80,0,25,0,10,1],"137":[0,0,80,0,25,0,10,1],"138":[0,0,80,0,25,0,10,1],"139":[0,0,80,0,25,0,10,1],"140":[0,0,80,0,25,0,10,1],"141":[0,0,80,0,25,0,10,1],"142":[0,0,80,0,25,0,10,1],"143":[0,0,80,0,25,0,10,1],"147":[0,0,80,0,25,0,10,1],"148":[0,0,80,0,25,0,10,1],"149":[0,0,80,0,25,0,10,1],"152":[0,2,100,0,25,0,10,1],"153":[0,0,80,0,25,0,10,1],"154":[0,0,80,0,25,0,10,1],"155":[0,2,100,0,25,0,10,1],"156":[0,0,80,0,25,0,10,1],"157":[0,0,80,0,25,0,10,1],"158":[0,2,100,0,25,0,10,1],"159":[0,0,80,0,25,0,10,1],"160":[0,0,80,0,25,0,10,1],"161":[0,2,100,0,25,0,10,1],"162":[0,0,80,0,25,0,10,1],"163":[0,2,100,0,25,0,10,1],"164":[0,0,80,0,25,0,10,1],"165":[0,2,100,0,25,0,10,1],"166":[0,0,80,0,25,0,10,1],"167":[0,2,100,0,25,0,10,1],"168":[0,0,80,0,25,0,10,1],"169":[0,0,80,0,25,0,10,1],"170":[0,0,80,0,25,0,10,1],"171":[0,0,80,0,25,0,10,1],"176":[0,0,80,0,25,0,10,1],"177":[0,2,100,0,25,0,10,1],"178":[0,0,80,0,25,0,10,1],"179":[0,0,80,0,25,0,10,1],"180":[0,0,80,0,25,0,10,1],"181":[0,0,80,0,25,0,10,1],"183":[0,0,80,0,25,0,10,1],"184":[0,0,80,0,25,0,10,1],"185":[0,0,80,0,25,0,10,1],"187":[0,2,100,0,25,0,10,1],"188":[0,0,80,0,25,0,10,1],"189":[0,0,80,0,25,0,10,1],"190":[0,2,100,0,25,0,10,1],"191":[0,0,80,0,25,0,10,1],"193":[0,0,80,0,25,0,10,1],"194":[0,2,100,0,25,0,10,1],"195":[0,0,80,0,25,0,10,1],"198":[0,2,100,0,25,0,10,1],"200":[0,0,80,0,25,0,10,1],"201":[0,0,80,0,25,0,10,1],"202":[0,0,80,0,25,0,10,1],"203":[0,0,80,0,25,0,10,1],"204":[0,0,80,0,25,0,10,1],"205":[0,0,80,0,25,0,10,1],"206":[0,0,80,0,25,0,10,1],"207":[0,0,80,0,25,0,10,1],"209":[0,0,80,0,25,0,10,1],"210":[0,0,80,0,25,0,10,1],"211":[0,0,80,0,25,0,10,1],"213":[0,0,80,0,25,0,10,1],"214":[0,0,80,0,25,0,10,1],"215":[0,0,80,0,25,0,10,1],"216":[0,2,100,0,25,0,10,1],"217":[0,0,80,0,25,0,10,1],"218":[0,0,80,0,25,0,10,1],"219":[0,0,80,0,25,0,10,1],"220":[0,0,80,0,25,0,10,1],"221":[0,0,80,0,25,0,10,1],"222":[0,0,80,0,25,0,10,1],"223":[0,0,80,0,25,0,10,1],"224":[0,0,80,0,25,0,10,1],"225":[0,2,100,0,25,0,10,1],"226":[0,0,80,0,25,0,10,1],"227":[0,0,80,0,25,0,10,1],"228":[0,0,80,0,25,0,10,1],"229":[0,0,80,0,25,0,10,1],"231":[0,0,80,0,25,0,10,1],"232":[0,0,80,0,25,0,10,1],"233":[0,0,80,0,25,0,10,1],"234":[0,0,80,0,25,0,10,1],"237":[0,0,80,0,25,0,10,1],"241":[0,0,80,0,25,0,10,1],"242":[0,0,80,0,25,0,10,1],"246":[0,0,80,0,25,0,10,1],"247":[0,0,80,0,25,0,10,1],"248":[0,0,80,0,25,0,10,1],"252":[0,2,100,0,25,0,10,1],"253":[0,0,80,0,25,0,10,1],"254":[0,0,80,0,25,0,10,1],"255":[0,2,100,0,25,0,10,1],"256":[0,0,80,0,25,0,10,1],"257":[0,0,80,0,25,0,10,1],"258":[0,2,100,0,25,0,10,1],"259":[0,0,80,0,25,0,10,1],"260":[0,0,80,0,25,0,10,1],"261":[0,2,100,0,25,0,10,1],"262":[0,0,80,0,25,0,10,1],"263":[0,2,100,0,25,0,10,1],"264":[0,0,80,0,25,0,10,1],"265":[0,2,100,0,25,0,10,1],"266":[0,0,80,0,25,0,10,1],"267":[0,0,80,0,25,0,10,1],"268":[0,0,80,0,25,0,10,1],"269":[0,0,80,0,25,0,10,1],"270":[0,0,80,0,25,0,10,1],"271":[0,0,80,0,25,0,10,1],"272":[0,0,80,0,25,0,10,1],"273":[0,2,100,0,25,0,10,1],"274":[0,0,80,0,25,0,10,1],"275":[0,0,80,0,25,0,10,1],"276":[0,0,80,0,25,0,10,1],"277":[0,0,80,0,25,0,10,1],"278":[0,0,80,0,25,0,10,1],"279":[0,0,80,0,25,0,10,1],"280":[0,0,80,0,25,0,10,1],"281":[0,0,80,0,25,0,10,1],"282":[0,0,80,0,25,0,10,1],"283":[0,0,80,0,25,0,10,1],"284":[0,0,80,0,25,0,10,1],"285":[0,2,100,0,25,0,10,1],"286":[0,0,80,0,25,0,10,1],"287":[0,0,80,0,25,0,10,1],"288":[0,0,80,0,25,0,10,1],"289":[0,0,80,0,25,0,10,1],"290":[0,0,80,0,25,0,10,1],"291":[0,0,80,0,25,0,10,1],"292":[0,0,80,0,25,0,10,1],"293":[0,2,100,0,25,0,10,1],"294":[0,0,80,0,25,0,10,1],"295":[0,0,80,0,25,0,10,1],"296":[0,2,100,0,25,0,10,1],"297":[0,0,80,0,25,0,10,1],"298":[0,0,80,0,25,0,10,1],"299":[0,2,100,0,25,0,10,1],"300":[0,2,100,0,25,0,10,1],"301":[0,0,80,0,25,0,10,1],"302":[0,2,100,0,25,0,10,1],"303":[0,0,80,0,25,0,10,1],"304":[0,2,100,0,25,0,10,1],"305":[0,0,80,0,25,0,10,1],"306":[0,0,80,0,25,0,10,1],"307":[0,2,100,0,25,0,10,1],"308":[0,0,80,0,25,0,10,1],"309":[0,2,100,0,25,0,10,1],"310":[0,0,80,0,25,0,10,1],"311":[0,0,80,0,25,0,10,1],"312":[0,2,100,0,25,0,10,1],"313":[0,0,80,0,25,0,10,1],"314":[0,0,80,0,25,0,10,1],"315":[0,2,100,0,25,0,10,1],"316":[0,2,100,0,25,0,10,1],"317":[0,0,80,0,25,0,10,1],"318":[0,0,80,0,25,0,10,1],"319":[0,0,80,0,25,0,10,1],"320":[0,2,100,0,25,0,10,1],"321":[0,0,80,0,25,0,10,1],"322":[0,2,100,0,25,0,10,1],"323":[0,0,80,0,25,0,10,1],"324":[0,0,80,0,25,0,10,1],"325":[0,2,100,0,25,0,10,1],"326":[0,0,80,0,25,0,10,1],"327":[0,0,80,0,25,0,10,1],"328":[0,0,80,0,25,0,10,1],"329":[0,0,80,0,25,0,10,1],"330":[0,0,80,0,25,0,10,1],"331":[0,2,100,0,25,0,10,1],"332":[0,0,80,0,25,0,10,1],"333":[0,0,80,0,25,0,10,1],"334":[0,0,80,0,25,0,10,1],"335":[0,2,100,0,25,0,10,1],"336":[0,2,100,0,25,0,10,1],"337":[0,0,80,0,25,0,10,1],"338":[0,0,80,0,25,0,10,1],"339":[0,2,100,0,25,0,10,1],"340":[0,0,80,0,25,0,10,1],"341":[0,0,80,0,25,0,10,1],"342":[0,0,80,0,25,0,10,1],"343":[0,2,100,0,25,0,10,1],"344":[0,0,80,0,25,0,10,1],"345":[0,0,80,0,25,0,10,1],"346":[0,0,80,0,25,0,10,1],"347":[0,0,80,0,25,0,10,1],"348":[0,0,80,0,25,0,10,1],"349":[0,0,80,0,25,0,10,1],"350":[0,0,80,0,25,0,10,1],"351":[0,0,80,0,25,0,10,1],"352":[0,0,80,0,25,0,10,1],"353":[0,2,100,0,25,0,10,1],"354":[0,0,80,0,25,0,10,1],"355":[0,2,100,0,25,0,10,1],"356":[0,0,80,0,25,0,10,1],"357":[0,0,80,0,25,0,10,1],"358":[0,0,80,0,25,0,10,1],"359":[0,0,80,0,25,0,10,1],"360":[0,0,80,0,25,0,10,1],"361":[0,0,80,0,25,0,10,1],"362":[0,0,80,0,25,0,10,1],"363":[0,2,100,0,25,0,10,1],"364":[0,0,80,0,25,0,10,1],"365":[0,0,80,0,25,0,10,1],"366":[0,0,80,0,25,0,10,1],"367":[0,0,80,0,25,0,10,1],"368":[0,0,80,0,25,0,10,1],"369":[0,0,80,0,25,0,10,1],"370":[0,0,80,0,25,0,10,1],"371":[0,0,80,0,25,0,10,1],"372":[0,0,80,0,25,0,10,1],"373":[0,0,80,0,25,0,10,1],"374":[0,0,80,0,25,0,10,1],"375":[0,0,80,0,25,0,10,1],"376":[0,0,80,0,25,0,10,1],"380":[0,0,80,0,25,0,10,1],"387":[0,0,80,0,25,0,10,1],"388":[0,0,80,0,25,0,10,1],"389":[0,0,80,0,25,0,10,1],"390":[0,0,80,0,25,0,10,1],"391":[0,0,80,0,25,0,10,1],"392":[0,0,80,0,25,0,10,1],"393":[0,0,80,0,25,0,10,1],"394":[0,0,80,0,25,0,10,1],"395":[0,0,80,0,25,0,10,1],"396":[0,0,80,0,25,0,10,1],"397":[0,0,80,0,25,0,10,1],"398":[0,0,80,0,25,0,10,1],"399":[0,0,80,0,25,0,10,1],"400":[0,0,80,0,25,0,10,1],"401":[0,0,80,0,25,0,10,1],"402":[0,0,80,0,25,0,10,1],"403":[0,0,80,0,25,0,10,1],"404":[0,0,80,0,25,0,10,1],"405":[0,0,80,0,25,0,10,1],"406":[0,0,80,0,25,0,10,1],"407":[0,0,80,0,25,0,10,1],"408":[1,0,80,0,25,0,10,1],"409":[1,0,80,0,25,0,10,1],"410":[1,0,80,0,25,0,10,1],"411":[1,0,80,0,25,0,10,1],"412":[1,0,80,0,25,0,10,1],"415":[1,0,80,0,25,0,10,1],"416":[1,0,80,0,25,0,10,1],"418":[0,0,80,0,25,0,10,1],"419":[0,0,80,0,25,0,10,1],"420":[1,0,80,0,25,0,10,1],"421":[1,0,80,0,25,0,10,1],"422":[1,0,80,0,25,0,10,1],"423":[1,0,80,0,25,0,10,1],"425":[0,0,80,0,25,0,10,1],"426":[0,0,80,0,25,0,10,1],"427":[0,0,80,0,25,0,10,1],"428":[0,0,80,0,25,0,10,1],"429":[0,0,80,0,25,0,10,1],"430":[0,0,80,0,25,0,10,1],"431":[1,0,80,0,25,0,10,1],"432":[1,0,80,0,25,0,10,1],"433":[0,0,80,0,25,0,10,1],"434":[1,0,80,0,25,0,10,1],"435":[1,0,80,0,25,0,10,1],"436":[1,0,80,0,25,0,10,1],"437":[1,0,80,0,25,0,10,1],"442":[0,0,80,0,25,0,10,1],"443":[1,0,80,0,25,0,10,1],"444":[1,0,80,0,25,0,10,1],"446":[0,0,80,0,25,0,10,1],"447":[0,0,80,0,25,0,10,1],"448":[0,0,80,0,25,0,10,1],"449":[1,0,80,0,25,0,10,1],"450":[1,0,80,0,25,0,10,1],"451":[1,0,80,0,25,0,10,1],"452":[1,0,80,0,25,0,10,1],"453":[0,0,80,0,25,0,10,1],"454":[0,0,80,0,25,0,10,1],"455":[0,0,80,0,25,0,10,1],"456":[0,0,80,0,25,0,10,1],"457":[0,0,80,0,25,0,10,1],"458":[0,0,80,0,25,0,10,1],"459":[0,0,80,0,25,0,10,1],"460":[0,0,80,0,25,0,10,1],"464":[0,0,80,0,25,0,10,1],"466":[0,0,80,0,25,0,10,1],"467":[0,0,80,0,25,0,10,1],"468":[0,0,80,0,25,0,10,1],"480":[0,1,80,0,25,0,10,1],"481":[0,1,80,0,25,0,10,1],"482":[0,1,80,0,25,0,10,1],"485":[0,0,80,0,25,0,10,1],"487":[0,0,80,0,25,0,10,1],"488":[0,0,80,0,25,0,10,1]}},"time":{"fi":[0,80],"fl":[0,30],"fc":0,"w1":[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23],"w2":[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23]},"more":{"l":"g"}}"#,
        ];
        for s in &data {
            serde_json::from_str::<BotConfig>(s).unwrap();
        }
    }

    // #[test]
    // fn sassari() {
    //     let config = serde_json::from_str::<BotConfig>(r#"{"locs":{"h":["40.835140","8.400128"],"p":["40.779602","8.499307","15"],"r":["40.780382","8.500980","15"],"i":["40.777717","8.500486","15"],"t_p":["0","0","0"],"t_r":["0","0","0"],"t_i":["","",""]},"raid":{"u":1,"s":1,"l":[1,2,3,4,5],"p":[599,403,215,26,94,68,176,359,105,386,487]},"pkmn":{"l":{"1":[0,0,80,0,25,0,10,1],"2":[0,0,80,0,25,0,10,1],"3":[0,0,80,0,25,0,10,1],"4":[0,0,80,0,25,0,10,1],"5":[0,0,80,0,25,0,10,1],"6":[0,0,80,0,25,0,10,1],"7":[0,0,80,0,25,0,10,1],"8":[0,0,80,0,25,0,10,1],"9":[0,0,80,0,25,0,10,1],"10":[0,2,100,0,25,0,10,1],"11":[0,0,80,0,25,0,10,1],"12":[0,0,80,0,25,0,10,1],"13":[0,2,100,0,25,0,10,1],"14":[0,0,80,0,25,0,10,1],"15":[0,0,80,0,25,0,10,1],"16":[0,2,100,0,25,0,10,1],"17":[0,0,80,0,25,0,10,1],"18":[0,0,80,0,25,0,10,1],"19":[0,2,100,0,25,0,10,1,1],"20":[0,0,80,0,25,0,10,1],"21":[0,2,100,0,25,0,10,1],"22":[0,0,80,0,25,0,10,1],"23":[0,2,100,0,25,0,10,1],"24":[0,0,80,0,25,0,10,1],"25":[0,2,100,0,25,0,10,1],"26":[0,0,80,0,25,0,10,1],"27":[0,0,80,0,25,0,10,1],"28":[0,0,80,0,25,0,10,1],"29":[0,2,100,0,25,0,10,1],"30":[0,0,80,0,25,0,10,1],"31":[0,0,80,0,25,0,10,1],"32":[0,2,100,0,25,0,10,1],"33":[0,0,80,0,25,0,10,1],"34":[0,0,80,0,25,0,10,1],"35":[0,0,80,0,25,0,10,1],"36":[0,0,80,0,25,0,10,1],"37":[0,2,100,0,25,0,10,1],"38":[0,0,80,0,25,0,10,1],"39":[0,0,80,0,25,0,10,1],"40":[0,0,80,0,25,0,10,1],"41":[0,2,100,0,25,0,10,1],"42":[0,0,80,0,25,0,10,1],"43":[0,0,80,0,25,0,10,1],"44":[0,0,80,0,25,0,10,1],"45":[0,0,80,0,25,0,10,1],"46":[0,2,100,0,25,0,10,1],"47":[0,0,80,0,25,0,10,1],"48":[0,2,100,0,25,0,10,1],"49":[0,0,80,0,25,0,10,1],"50":[0,0,80,0,25,0,10,1],"51":[0,0,80,0,25,0,10,1],"52":[0,0,80,0,25,0,10,1],"53":[0,0,80,0,25,0,10,1],"54":[0,0,80,0,25,0,10,1],"55":[0,0,80,0,25,0,10,1],"56":[0,0,80,0,25,0,10,1],"57":[0,0,80,0,25,0,10,1],"58":[0,0,80,0,25,0,10,1],"59":[0,0,80,0,25,0,10,1],"60":[0,0,80,0,25,0,10,1],"61":[0,0,80,0,25,0,10,1],"62":[0,0,80,0,25,0,10,1],"63":[0,0,80,0,25,0,10,1],"64":[0,0,80,0,25,0,10,1],"65":[0,0,80,0,25,0,10,1],"66":[0,0,80,0,25,0,10,1],"67":[0,0,80,0,25,0,10,1],"68":[0,0,80,0,25,0,10,1],"69":[0,0,80,0,25,0,10,1],"70":[0,0,80,0,25,0,10,1],"71":[0,0,80,0,25,0,10,1],"72":[0,0,80,0,25,0,10,1],"73":[0,0,80,0,25,0,10,1],"74":[0,2,100,0,25,0,10,1],"75":[0,0,80,0,25,0,10,1],"76":[0,0,80,0,25,0,10,1],"77":[0,0,80,0,25,0,10,1],"78":[0,0,80,0,25,0,10,1],"79":[0,0,80,0,25,0,10,1],"80":[0,0,80,0,25,0,10,1],"81":[0,0,80,0,25,0,10,1],"82":[0,0,80,0,25,0,10,1],"83":[0,0,80,0,25,0,10,1],"84":[0,0,80,0,25,0,10,1],"85":[0,0,80,0,25,0,10,1],"86":[0,0,80,0,25,0,10,1],"87":[0,0,80,0,25,0,10,1],"88":[0,0,80,0,25,0,10,1],"89":[0,0,80,0,25,0,10,1],"90":[0,2,100,0,25,0,10,1],"91":[0,0,80,0,25,0,10,1],"92":[0,0,80,0,25,0,10,1],"93":[0,0,80,0,25,0,10,1],"94":[0,0,80,0,25,0,10,1],"95":[0,0,80,0,25,0,10,1],"96":[0,0,80,0,25,0,10,1],"97":[0,0,80,0,25,0,10,1],"98":[0,0,80,0,25,0,10,1],"99":[0,0,80,0,25,0,10,1],"100":[0,0,80,0,25,0,10,1],"101":[0,0,80,0,25,0,10,1],"102":[0,0,80,0,25,0,10,1],"103":[0,0,80,0,25,0,10,1],"104":[0,0,80,0,25,0,10,1],"105":[0,0,80,0,25,0,10,1],"106":[0,0,80,0,25,0,10,1],"107":[0,0,80,0,25,0,10,1],"108":[0,0,80,0,25,0,10,1],"109":[0,0,80,0,25,0,10,1],"110":[0,0,80,0,25,0,10,1],"111":[0,0,80,0,25,0,10,1],"112":[0,0,80,0,25,0,10,1],"113":[1,0,80,0,25,0,10,1],"114":[0,0,80,0,25,0,10,1],"115":[0,0,80,0,25,0,10,1],"116":[0,0,80,0,25,0,10,1],"117":[0,0,80,0,25,0,10,1],"118":[0,0,80,0,25,0,10,1],"119":[0,0,80,0,25,0,10,1],"120":[0,0,80,0,25,0,10,1],"121":[0,0,80,0,25,0,10,1],"122":[0,0,80,0,25,0,10,1],"123":[0,0,80,0,25,0,10,1],"124":[0,0,80,0,25,0,10,1],"125":[0,0,80,0,25,0,10,1],"126":[0,0,80,0,25,0,10,1],"127":[0,0,80,0,25,0,10,1],"128":[0,0,80,0,25,0,10,1],"129":[0,2,100,0,25,0,10,1,0],"130":[0,0,80,0,25,0,10,1],"131":[0,0,80,0,25,0,10,1],"132":[0,0,80,0,25,0,10,1],"133":[0,2,100,0,25,0,10,1],"134":[0,0,80,0,25,0,10,1],"135":[0,0,80,0,25,0,10,1],"136":[0,0,80,0,25,0,10,1],"137":[0,0,80,0,25,0,10,1],"138":[0,0,80,0,25,0,10,1],"139":[0,0,80,0,25,0,10,1],"140":[0,0,80,0,25,0,10,1],"141":[0,0,80,0,25,0,10,1],"142":[0,0,80,0,25,0,10,1],"143":[0,0,80,0,25,0,10,1],"147":[1,0,80,0,25,0,10,1],"148":[1,0,80,0,25,0,10,1],"149":[1,0,80,0,25,0,10,1],"152":[0,2,100,0,25,0,10,1],"153":[0,0,80,0,25,0,10,1],"154":[0,0,80,0,25,0,10,1],"155":[0,2,100,0,25,0,10,1],"156":[0,0,80,0,25,0,10,1],"157":[0,0,80,0,25,0,10,1],"158":[0,2,100,0,25,0,10,1],"159":[0,0,80,0,25,0,10,1],"160":[0,0,80,0,25,0,10,1],"161":[0,2,100,0,25,0,10,1],"162":[0,0,80,0,25,0,10,1],"163":[0,2,100,0,25,0,10,1],"164":[0,0,80,0,25,0,10,1],"165":[0,2,100,0,25,0,10,1],"166":[0,0,80,0,25,0,10,1],"167":[0,2,100,0,25,0,10,1],"168":[0,0,80,0,25,0,10,1],"169":[0,0,80,0,25,0,10,1],"170":[0,0,80,0,25,0,10,1],"171":[0,0,80,0,25,0,10,1],"176":[0,0,80,0,25,0,10,1],"177":[0,2,100,0,25,0,10,1],"178":[0,0,80,0,25,0,10,1],"179":[0,0,80,0,25,0,10,1],"180":[0,0,80,0,25,0,10,1],"181":[0,0,80,0,25,0,10,1],"183":[0,0,80,0,25,0,10,1],"184":[0,0,80,0,25,0,10,1],"185":[0,0,80,0,25,0,10,1],"187":[0,2,100,0,25,0,10,1],"188":[0,0,80,0,25,0,10,1],"189":[0,0,80,0,25,0,10,1],"190":[0,2,100,0,25,0,10,1],"191":[0,0,80,0,25,0,10,1],"193":[0,0,80,0,25,0,10,1],"194":[0,2,100,0,25,0,10,1],"195":[0,0,80,0,25,0,10,1],"198":[0,2,100,0,25,0,10,1],"200":[0,0,80,0,25,0,10,1],"201":[1,0,80,0,25,0,10,1],"202":[0,0,80,0,25,0,10,1],"203":[0,0,80,0,25,0,10,1],"204":[0,0,80,0,25,0,10,1],"205":[0,0,80,0,25,0,10,1],"206":[0,0,80,0,25,0,10,1],"207":[0,0,80,0,25,0,10,1],"209":[0,0,80,0,25,0,10,1],"210":[0,0,80,0,25,0,10,1],"211":[0,0,80,0,25,0,10,1],"213":[0,0,80,0,25,0,10,1],"214":[1,0,80,0,25,0,10,1],"215":[0,0,80,0,25,0,10,1],"216":[0,2,100,0,25,0,10,1],"217":[0,0,80,0,25,0,10,1],"218":[0,0,80,0,25,0,10,1],"219":[0,0,80,0,25,0,10,1],"220":[0,0,80,0,25,0,10,1],"221":[0,0,80,0,25,0,10,1],"222":[1,0,80,0,25,0,10,1],"223":[0,0,80,0,25,0,10,1],"224":[0,0,80,0,25,0,10,1],"225":[0,2,100,0,25,0,10,1],"226":[0,0,80,0,25,0,10,1],"227":[0,0,80,0,25,0,10,1],"228":[0,0,80,0,25,0,10,1],"229":[0,0,80,0,25,0,10,1],"231":[0,0,80,0,25,0,10,1],"232":[0,0,80,0,25,0,10,1],"233":[0,0,80,0,25,0,10,1],"234":[0,0,80,0,25,0,10,1],"237":[0,0,80,0,25,0,10,1],"241":[0,0,80,0,25,0,10,1],"242":[1,0,80,0,25,0,10,1],"246":[1,0,80,0,25,0,10,1],"247":[1,0,80,0,25,0,10,1],"248":[1,0,80,0,25,0,10,1],"252":[0,2,100,0,25,0,10,1],"253":[0,0,80,0,25,0,10,1],"254":[0,0,80,0,25,0,10,1],"255":[0,2,100,0,25,0,10,1],"256":[0,0,80,0,25,0,10,1],"257":[0,0,80,0,25,0,10,1],"258":[0,2,100,0,25,0,10,1],"259":[0,0,80,0,25,0,10,1],"260":[0,0,80,0,25,0,10,1],"261":[0,2,100,0,25,0,10,1],"262":[0,0,80,0,25,0,10,1],"263":[0,2,100,0,25,0,10,1],"264":[0,0,80,0,25,0,10,1],"265":[0,2,100,0,25,0,10,1],"266":[0,0,80,0,25,0,10,1],"267":[0,0,80,0,25,0,10,1],"268":[0,0,80,0,25,0,10,1],"269":[0,0,80,0,25,0,10,1],"270":[0,0,80,0,25,0,10,1],"271":[0,0,80,0,25,0,10,1],"272":[0,0,80,0,25,0,10,1],"273":[0,2,100,0,25,0,10,1],"274":[0,0,80,0,25,0,10,1],"275":[0,0,80,0,25,0,10,1],"276":[0,0,80,0,25,0,10,1],"277":[0,0,80,0,25,0,10,1],"278":[0,0,80,0,25,0,10,1],"279":[0,0,80,0,25,0,10,1],"280":[1,0,80,0,25,0,10,1],"281":[1,0,80,0,25,0,10,1],"282":[0,0,80,0,25,0,10,1],"283":[0,0,80,0,25,0,10,1],"284":[0,0,80,0,25,0,10,1],"285":[0,2,100,0,25,0,10,1],"286":[1,0,80,0,25,0,10,1],"287":[0,0,80,0,25,0,10,1],"288":[0,0,80,0,25,0,10,1],"289":[0,0,80,0,25,0,10,1],"290":[0,0,80,0,25,0,10,1],"291":[0,0,80,0,25,0,10,1],"292":[0,0,80,0,25,0,10,1],"293":[0,2,100,0,25,0,10,1],"294":[0,0,80,0,25,0,10,1],"295":[0,0,80,0,25,0,10,1],"296":[0,2,100,0,25,0,10,1],"297":[0,0,80,0,25,0,10,1],"298":[0,0,80,0,25,0,10,1],"299":[0,2,100,0,25,0,10,1],"300":[0,2,100,0,25,0,10,1],"301":[0,0,80,0,25,0,10,1],"302":[0,2,100,0,25,0,10,1],"303":[0,0,80,0,25,0,10,1],"304":[0,2,100,0,25,0,10,1],"305":[0,0,80,0,25,0,10,1],"306":[0,0,80,0,25,0,10,1],"307":[0,2,100,0,25,0,10,1],"308":[0,0,80,0,25,0,10,1],"309":[0,2,100,0,25,0,10,1],"310":[0,0,80,0,25,0,10,1],"311":[0,0,80,0,25,0,10,1],"312":[0,2,100,0,25,0,10,1],"313":[0,0,80,0,25,0,10,1],"314":[0,0,80,0,25,0,10,1],"315":[0,2,98,0,25,0,10,1],"316":[0,2,100,0,25,0,10,1],"317":[0,0,80,0,25,0,10,1],"318":[0,0,80,0,25,0,10,1],"319":[0,0,80,0,25,0,10,1],"320":[0,2,100,0,25,0,10,1],"321":[0,0,80,0,25,0,10,1],"322":[0,2,100,0,25,0,10,1],"323":[0,0,80,0,25,0,10,1],"324":[0,0,80,0,25,0,10,1],"325":[0,2,100,0,25,0,10,1],"326":[0,0,80,0,25,0,10,1],"327":[0,0,80,0,25,0,10,1],"328":[0,0,80,0,25,0,10,1],"329":[0,0,80,0,25,0,10,1],"330":[0,0,80,0,25,0,10,1],"331":[0,2,100,0,25,0,10,1],"332":[0,0,80,0,25,0,10,1],"333":[0,0,80,0,25,0,10,1],"334":[0,0,80,0,25,0,10,1],"335":[0,2,100,0,25,0,10,1],"336":[0,2,100,0,25,0,10,1],"337":[0,0,80,0,25,0,10,1],"338":[0,0,80,0,25,0,10,1],"339":[0,2,100,0,25,0,10,1],"340":[0,0,80,0,25,0,10,1],"341":[0,0,80,0,25,0,10,1],"342":[0,0,80,0,25,0,10,1],"343":[0,2,100,0,25,0,10,1],"344":[0,0,80,0,25,0,10,1],"345":[0,0,80,0,25,0,10,1],"346":[0,0,80,0,25,0,10,1],"347":[0,0,80,0,25,0,10,1],"348":[0,0,80,0,25,0,10,1],"349":[0,0,80,0,25,0,10,1],"350":[0,0,80,0,25,0,10,1],"351":[0,0,80,0,25,0,10,1],"352":[0,0,80,0,25,0,10,1],"353":[0,2,100,0,25,0,10,1],"354":[0,0,80,0,25,0,10,1],"355":[0,2,100,0,25,0,10,1],"356":[0,0,80,0,25,0,10,1],"357":[0,0,80,0,25,0,10,1],"358":[1,0,80,0,25,0,10,1],"359":[0,0,80,0,25,0,10,1],"360":[0,0,80,0,25,0,10,1],"361":[0,0,80,0,25,0,10,1],"362":[0,0,80,0,25,0,10,1],"363":[0,2,100,0,25,0,10,1],"364":[0,0,80,0,25,0,10,1],"365":[0,0,80,0,25,0,10,1],"366":[0,0,80,0,25,0,10,1],"367":[0,0,80,0,25,0,10,1],"368":[0,0,80,0,25,0,10,1],"369":[0,0,80,0,25,0,10,1],"370":[0,0,80,0,25,0,10,1],"371":[1,0,80,0,25,0,10,1],"372":[1,0,80,0,25,0,10,1],"373":[0,0,80,0,25,0,10,1],"374":[1,0,80,0,25,0,10,1],"375":[1,0,80,0,25,0,10,1],"376":[0,0,80,0,25,0,10,1],"380":[0,0,80,0,25,0,10,1],"387":[0,0,80,0,25,0,10,1],"388":[0,0,80,0,25,0,10,1],"389":[0,0,80,0,25,0,10,1],"390":[0,0,80,0,25,0,10,1],"391":[0,0,80,0,25,0,10,1],"392":[0,0,80,0,25,0,10,1],"393":[0,0,80,0,25,0,10,1],"394":[0,0,80,0,25,0,10,1],"395":[0,0,80,0,25,0,10,1],"396":[0,0,80,0,25,0,10,1],"397":[0,0,80,0,25,0,10,1],"398":[0,0,80,0,25,0,10,1],"399":[0,0,80,0,25,0,10,1],"400":[0,0,80,0,25,0,10,1],"401":[0,0,80,0,25,0,10,1],"402":[0,0,80,0,25,0,10,1],"403":[0,0,80,0,25,0,10,1],"404":[0,0,80,0,25,0,10,1],"405":[0,0,80,0,25,0,10,1],"406":[0,0,80,0,25,0,10,1],"407":[0,0,80,0,25,0,10,1],"408":[1,0,80,0,25,0,10,1],"409":[0,0,80,0,25,0,10,1],"410":[0,0,80,0,25,0,10,1],"411":[0,0,80,0,25,0,10,1],"412":[0,0,80,0,25,0,10,1],"415":[0,0,80,0,25,0,10,1],"416":[0,0,80,0,25,0,10,1],"418":[0,0,80,0,25,0,10,1],"419":[0,0,80,0,25,0,10,1],"420":[0,0,80,0,25,0,10,1],"421":[0,0,80,0,25,0,10,1],"422":[0,0,80,0,25,0,10,1],"423":[0,0,80,0,25,0,10,1],"425":[0,0,80,0,25,0,10,1],"426":[0,0,80,0,25,0,10,1],"427":[0,0,80,0,25,0,10,1],"428":[0,0,80,0,25,0,10,1],"429":[0,0,80,0,25,0,10,1],"430":[0,0,80,0,25,0,10,1],"431":[0,0,80,0,25,0,10,1],"432":[0,0,80,0,25,0,10,1],"433":[0,0,80,0,25,0,10,1],"434":[0,0,80,0,25,0,10,1],"435":[0,0,80,0,25,0,10,1],"436":[0,0,80,0,25,0,10,1],"437":[0,0,80,0,25,0,10,1],"442":[0,0,80,0,25,0,10,1],"443":[1,0,80,0,25,0,10,1],"444":[1,0,80,0,25,0,10,1],"446":[0,0,80,0,25,0,10,1],"447":[0,0,80,0,25,0,10,1],"448":[0,0,80,0,25,0,10,1],"449":[0,0,80,0,25,0,10,1],"450":[0,0,80,0,25,0,10,1],"451":[0,0,80,0,25,0,10,1],"452":[0,0,80,0,25,0,10,1],"453":[0,0,80,0,25,0,10,1],"454":[0,0,80,0,25,0,10,1],"455":[0,0,80,0,25,0,10,1],"456":[0,0,80,0,25,0,10,1],"457":[0,0,80,0,25,0,10,1],"458":[0,0,80,0,25,0,10,1],"459":[0,0,80,0,25,0,10,1],"460":[0,0,80,0,25,0,10,1],"464":[0,0,80,0,25,0,10,1],"466":[0,0,80,0,25,0,10,1],"467":[0,0,80,0,25,0,10,1],"468":[0,0,80,0,25,0,10,1],"480":[1,0,80,0,25,0,10,1],"481":[1,0,80,0,25,0,10,1],"482":[1,0,80,0,25,0,10,1],"485":[0,0,80,0,25,0,10,1],"487":[0,0,80,0,25,0,10,1],"488":[0,0,80,0,25,0,10,1],"495":[1,0,80,0,25,0,10,1],"496":[1,0,80,0,25,0,10,1],"497":[1,0,80,0,25,0,10,1],"498":[1,0,80,0,25,0,10,1],"499":[1,0,80,0,25,0,10,1],"500":[1,0,80,0,25,0,10,1],"501":[1,0,80,0,25,0,10,1],"502":[1,0,80,0,25,0,10,1],"503":[1,0,80,0,25,0,10,1],"504":[0,2,80,0,25,0,10,1],"505":[0,0,80,0,25,0,10,1],"506":[0,2,80,0,25,0,10,1],"507":[0,0,80,0,25,0,10,1],"508":[0,0,80,0,25,0,10,1],"509":[0,0,80,0,25,0,10,1],"510":[0,0,80,0,25,0,10,1],"511":[0,0,80,0,25,0,10,1],"512":[0,0,80,0,25,0,10,1],"513":[0,0,80,0,25,0,10,1],"514":[0,0,80,0,25,0,10,1],"515":[0,0,80,0,25,0,10,1],"516":[0,0,80,0,25,0,10,1],"517":[0,0,80,0,25,0,10,1],"518":[0,0,80,0,25,0,10,1],"519":[0,2,80,0,25,0,10,1],"520":[0,0,80,0,25,0,10,1],"521":[0,0,80,0,25,0,10,1],"522":[0,0,80,0,25,0,10,1],"523":[0,0,80,0,25,0,10,1],"524":[0,0,80,0,25,0,10,1],"525":[0,0,80,0,25,0,10,1],"526":[0,0,80,0,25,0,10,1],"527":[0,0,80,0,25,0,10,1],"528":[0,0,80,0,25,0,10,1],"529":[0,0,80,0,25,0,10,1],"530":[1,0,80,0,25,0,10,1],"531":[0,0,80,0,25,0,10,1],"532":[1,0,80,0,25,0,10,1],"533":[1,0,80,0,25,0,10,1],"534":[1,0,80,0,25,0,10,1],"535":[0,0,80,0,25,0,10,1],"536":[0,0,80,0,25,0,10,1],"537":[0,0,80,0,25,0,10,1],"538":[0,0,80,0,25,0,10,1],"539":[0,0,80,0,25,0,10,1],"540":[0,0,80,0,25,0,10,1],"541":[0,0,80,0,25,0,10,1],"542":[0,0,80,0,25,0,10,1],"543":[0,0,80,0,25,0,10,1],"544":[0,0,80,0,25,0,10,1],"545":[0,0,80,0,25,0,10,1],"546":[0,0,80,0,25,0,10,1],"547":[0,0,80,0,25,0,10,1],"548":[0,0,80,0,25,0,10,1],"549":[0,0,80,0,25,0,10,1],"550":[0,0,80,0,25,0,10,1],"551":[0,0,80,0,25,0,10,1],"552":[0,0,80,0,25,0,10,1],"553":[0,0,80,0,25,0,10,1],"554":[0,0,80,0,25,0,10,1],"555":[0,0,80,0,25,0,10,1],"556":[0,0,80,0,25,0,10,1],"557":[0,0,80,0,25,0,10,1],"558":[0,0,80,0,25,0,10,1],"559":[0,0,80,0,25,0,10,1],"560":[0,0,80,0,25,0,10,1],"561":[0,0,80,0,25,0,10,1],"562":[0,0,80,0,25,0,10,1],"563":[0,0,80,0,25,0,10,1],"564":[0,0,80,0,25,0,10,1],"565":[0,0,80,0,25,0,10,1],"566":[1,0,80,0,25,0,10,1],"567":[1,0,80,0,25,0,10,1],"568":[0,0,80,0,25,0,10,1],"569":[0,0,80,0,25,0,10,1],"570":[0,0,80,0,25,0,10,1],"571":[0,0,80,0,25,0,10,1],"572":[0,0,80,0,25,0,10,1],"573":[0,0,80,0,25,0,10,1],"574":[0,0,80,0,25,0,10,1],"575":[0,0,80,0,25,0,10,1],"576":[0,0,80,0,25,0,10,1],"577":[0,0,80,0,25,0,10,1],"578":[0,0,80,0,25,0,10,1],"579":[0,0,80,0,25,0,10,1],"580":[0,0,80,0,25,0,10,1],"581":[0,0,80,0,25,0,10,1],"582":[0,0,80,0,25,0,10,1],"583":[0,0,80,0,25,0,10,1],"584":[0,0,80,0,25,0,10,1],"585":[0,0,80,0,25,0,10,1],"586":[0,0,80,0,25,0,10,1],"587":[0,0,80,0,25,0,10,1],"588":[0,0,80,0,25,0,10,1],"589":[0,0,80,0,25,0,10,1],"590":[0,0,80,0,25,0,10,1],"591":[0,0,80,0,25,0,10,1],"592":[0,0,80,0,25,0,10,1],"593":[0,0,80,0,25,0,10,1],"594":[0,0,80,0,25,0,10,1],"595":[0,0,80,0,25,0,10,1],"596":[0,0,80,0,25,0,10,1],"597":[1,0,80,0,25,0,10,1],"598":[1,0,80,0,25,0,10,1],"599":[0,0,80,0,25,0,10,1],"600":[0,0,80,0,25,0,10,1],"601":[0,0,80,0,25,0,10,1],"602":[0,0,80,0,25,0,10,1],"603":[0,0,80,0,25,0,10,1],"604":[0,0,80,0,25,0,10,1],"605":[0,0,80,0,25,0,10,1],"606":[0,0,80,0,25,0,10,1],"607":[1,0,80,0,25,0,10,1],"608":[1,0,80,0,25,0,10,1],"609":[1,0,80,0,25,0,10,1],"610":[0,0,80,0,25,0,10,1],"611":[0,0,80,0,25,0,10,1],"612":[0,0,80,0,25,0,10,1],"613":[0,0,80,0,25,0,10,1],"614":[0,0,80,0,25,0,10,1],"615":[0,0,80,0,25,0,10,1],"616":[0,0,80,0,25,0,10,1],"617":[0,0,80,0,25,0,10,1],"618":[0,0,80,0,25,0,10,1],"619":[0,0,80,0,25,0,10,1],"620":[0,0,80,0,25,0,10,1],"621":[0,0,80,0,25,0,10,1],"622":[1,0,80,0,25,0,10,1],"623":[1,0,80,0,25,0,10,1],"624":[0,0,80,0,25,0,10,1],"625":[0,0,80,0,25,0,10,1],"626":[0,0,80,0,25,0,10,1],"627":[0,0,80,0,25,0,10,1],"628":[0,0,80,0,25,0,10,1],"629":[0,0,80,0,25,0,10,1],"630":[0,0,80,0,25,0,10,1],"631":[0,0,80,0,25,0,10,1],"632":[0,0,80,0,25,0,10,1],"633":[1,0,80,0,25,0,10,1],"634":[1,0,80,0,25,0,10,1],"635":[1,0,80,0,25,0,10,1],"636":[1,0,80,0,25,0,10,1],"637":[1,0,80,0,25,0,10,1]}},"time":{"fi":[0,80],"fl":[0,30],"fc":0,"w1":[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23],"w2":[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23]},"invs":{"n":1,"f":1,"l":[10,11,14,15]},"more":{"l":"g"}}"#).unwrap();
    //     let input: Pokemon = serde_json::from_str(r#"{"pokestop_id":"3d716717cc65421490684ef9b213a382.16","disappear_time":1571079918,"cp":null,"form":0,"move_1":null,"longitude":8.567366,"costume":0,"pokemon_id":498,"disappear_time_verified":false,"gender":1,"individual_attack":null,"spawnpoint_id":"None","latitude":40.722668,"pokemon_level":null,"move_2":null,"individual_defense":null,"weight":null,"encounter_id":"12661125248363616471","height":null,"weather":1,"first_seen":1571078718,"individual_stamina":null,"last_modified_time":1571078718}"#).unwrap();
    //     assert!(config.submit(&Local::now(), "122091018", &Request::Pokemon(Box::new(input))).is_err());
    // }
}

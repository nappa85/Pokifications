use std::cell::UnsafeCell;

use serde::{Deserialize, Deserializer};

use serde_json::value::Value;

use geo::{LineString, Point};

#[derive(Clone, Debug, Deserialize)]
// #[serde(deny_unknown_fields)]
#[serde(tag = "type", content = "message")]
pub enum Request {
    #[serde(rename = "pokemon")]
    Pokemon(Box<Pokemon>),
    #[serde(rename = "pokestop")]
    Pokestop(Pokestop),
    #[serde(rename = "gym")]
    Gym(Gym),
    #[serde(rename = "gym_details")]
    GymDetails(GymDetails),
    #[serde(rename = "raid")]
    Raid(Raid),
    #[serde(rename = "scheduler")]
    Scheduler(Scheduler),
    #[serde(rename = "captcha")]
    Captcha(Captcha),
    #[serde(rename = "invasion")]
    Invasion(Pokestop),
    #[serde(rename = "quest")]
    Quest(Quest),
    #[serde(rename = "weather")]
    Weather(Weather),
    #[serde(rename = "reload")]
    Reload(Vec<String>),
    #[serde(rename = "watch")]
    Watch(Watch),
}

#[derive(Clone, Debug, Deserialize)]
// #[serde(deny_unknown_fields)]
pub struct Pokemon {
    pub spawnpoint_id: String,
    pub encounter_id: String,
    pub pokemon_id: u16,
    #[serde(default)]
    pub pokestop_id: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub disappear_time: i64,
    pub disappear_time_verified: bool,
    // time_until_hidden_ms: i64,
    pub last_modified_time: i64,
    // seconds_until_despawn: i64,
    pub first_seen: i64,
    // spawn_start: i64,
    // spawn_end: i64,
    pub gender: Gender,
    #[serde(default)]
    pub cp: Option<usize>,
    #[serde(default)]
    pub form: Option<u16>,
    #[serde(default)]
    pub costume: Option<usize>,
    #[serde(default)]
    pub individual_attack: Option<u8>,
    #[serde(default)]
    pub individual_defense: Option<u8>,
    #[serde(default)]
    pub individual_stamina: Option<u8>,
    #[serde(default)]
    pub cp_multiplier: Option<f64>,
    #[serde(default)]
    pub move_1: Option<u16>,
    #[serde(default)]
    pub move_2: Option<u16>,
    #[serde(default)]
    pub weight: Option<f64>,
    #[serde(default)]
    pub height: Option<f64>,
    #[serde(default)]
    pub base_catch: Option<f64>,
    #[serde(default)]
    pub great_catch: Option<f64>,
    #[serde(default)]
    pub ultra_catch: Option<f64>,
    #[serde(default)]
    pub boosted_weather: Option<usize>,
    #[serde(default)]
    pub def_grade: Option<String>,
    #[serde(default)]
    pub atk_grade: Option<String>,
    #[serde(default)]
    pub rating_attack: Option<String>,
    #[serde(default)]
    pub rating_defense: Option<String>,
    #[serde(default)]
    pub catch_prob_1: Option<f64>,
    #[serde(default)]
    pub catch_prob_2: Option<f64>,
    #[serde(default)]
    pub catch_prob_3: Option<f64>,
    #[serde(default)]
    pub weather: Option<u8>,
    #[serde(default)]
    pub weather_boosted_condition: Option<usize>,
    #[serde(default)]
    pub pokemon_level: Option<u8>,
    #[serde(default)]
    pub s2_cell_id: Option<usize>,
    // player_level: u8,
    // verified: bool,
    pub username: Option<String>,
    pub shiny: Option<bool>,
    pub display_pokemon_id: Option<u16>,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Gender {
    Unset,
    Male,
    Female,
    Genderless, 
}

impl Gender {
    pub fn get_glyph(&self) -> String {
        match self {
            Gender::Male => unsafe { String::from_utf8_unchecked(vec![0xe2, 0x99, 0x82]) },
            Gender::Female => unsafe { String::from_utf8_unchecked(vec![0xe2, 0x99, 0x80]) },
            _ => String::new(),
        }
    }
}

impl ToString for Gender {
    fn to_string(&self) -> String {
        (match *self {
            Gender::Unset => "unset",
            Gender::Male => "male",
            Gender::Female => "female",
            Gender::Genderless => "genderless",
        }).to_owned()
    }
}

impl<'de> Deserialize<'de> for Gender {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        let value = usize::deserialize(deserializer)?;

        // Rust does not come with a simple way of converting a
        // number to an enum, so use a big `match`.
        match value {
            0 => Ok(Gender::Unset),
            1 => Ok(Gender::Male),
            2 => Ok(Gender::Female),
            3 => Ok(Gender::Genderless),
            _ => Err(serde::de::Error::custom(format!("unknown Gender value: {}", value))),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
// #[serde(deny_unknown_fields)]
pub struct Pokestop {
    pub pokestop_id: String,
    pub name: String,
    pub url: String,
    pub latitude: f64,
    pub longitude: f64,
    pub enabled: bool,
    pub last_modified: i64,
    #[serde(default)]
    pub active_fort_modifier: Option<bool>,
    #[serde(default)]
    pub lure_expiration: Option<i64>,
    pub pokestop_display: u8,
    #[serde(default)]
    pub incident_expire_timestamp: Option<i64>,
    pub updated: i64,
    pub lure_id: u16,
    pub grunt_type: Option<u8>,
}

#[derive(Clone, Debug, Deserialize)]
// #[serde(deny_unknown_fields)]
pub struct Gym {
    pub gym_id: String,
    pub gym_name: String,
    pub url: String,
    pub latitude: f64,
    pub longitude: f64,
    pub enabled: bool,
    pub team_id: Team,
    // pub occupied_since: i64,
    pub last_modified: i64,
    pub guard_pokemon_id: u16,
    // pub total_cp: usize,
    pub slots_available: u8,
    // pub lowest_pokemon_motivation: f64,
    pub raid_active_until: i64,
    pub ex_raid_eligible: bool,
}

#[derive(Clone, Debug)]
pub enum Team {
    Uncontested,
    Mystic,
    Valor,
    Instinct,
}

impl Team {
    pub fn get_id(&self) -> u8 {
        match self {
            Team::Uncontested => 0,
            Team::Mystic => 1,
            Team::Valor => 2,
            Team::Instinct => 3,
        }
    }
}

impl ToString for Team {
    fn to_string(&self) -> String {
        (match *self {
            Team::Uncontested => "uncontested",
            Team::Mystic => "mystic",
            Team::Valor => "valor",
            Team::Instinct => "instinct",
        }).to_owned()
    }
}

impl<'de> Deserialize<'de> for Team {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        let value = usize::deserialize(deserializer)?;

        // Rust does not come with a simple way of converting a
        // number to an enum, so use a big `match`.
        match value {
            0 => Ok(Team::Uncontested),
            1 => Ok(Team::Mystic),
            2 => Ok(Team::Valor),
            3 => Ok(Team::Instinct),
            _ => Err(serde::de::Error::custom(format!("unknown Team value: {}", value))),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
// #[serde(deny_unknown_fields)]
pub struct GymDetails {
    pub id: String,
    pub name: String,
    // description: String,
    pub url: String,
    pub latitude: f64,
    pub longitude: f64,
    pub team: Team,
    // pokemon: Vec<GymPokemon>,
    pub in_battle: bool,
    pub slots_available: u8,
    pub ex_raid_eligible: bool,
}

// #[derive(Clone, Debug, Deserialize)]
// // #[serde(deny_unknown_fields)]
// pub struct GymPokemon {
//     pokemon_uid: usize,
//     pokemon_id: u16,
//     cp: usize,
//     cp_decayed: usize,
//     stamina_max: usize,
//     stamina: usize,
//     move_1: u16,
//     move_2: u16,
//     height: f64,
//     weight: f64,
//     form: usize,
//     costume: usize,
//     iv_attack: usize,
//     iv_defense: usize,
//     iv_stamina: usize,
//     cp_multiplier: f64,
//     additional_cp_multiplier: f64,
//     num_upgrades: usize,
//     deployment_time: i64,
// }

#[derive(Clone, Debug, Deserialize)]
// #[serde(deny_unknown_fields)]
pub struct Raid {
    pub gym_id: String,
    pub gym_name: String,
    pub gym_url: String,
    pub team_id: Team,
    pub latitude: f64,
    pub longitude: f64,
    pub spawn: i64,
    pub start: i64,
    pub end: i64,
    pub level: u8,
    #[serde(default)]
    pub pokemon_id: Option<u16>,
    #[serde(default)]
    pub cp: Option<usize>,
    #[serde(default)]
    pub move_1: Option<u16>,
    #[serde(default)]
    pub move_2: Option<u16>,
    #[serde(default)]
    pub weather: Option<usize>,
    #[serde(default)]
    pub s2_cell_id: Option<usize>,
    pub ex_raid_eligible: bool,
    #[serde(default)]
    pub form: Option<u16>,
    pub is_exclusive: bool,
    pub gender: Gender,
}

#[derive(Clone, Debug, Deserialize)]
// #[serde(deny_unknown_fields)]
pub struct Scheduler {
    pub name: String,
    pub instance: String,
    pub tth_found: f64,
    pub spawns_found: usize,
}

#[derive(Clone, Debug, Deserialize)]
// #[serde(deny_unknown_fields)]
pub struct Captcha {
    pub status_name: String,
    pub account: String,
    pub status: CaptchaStatus,
    pub captcha: usize,
    pub time: i64,
    pub mode: String,
}

#[derive(Clone, Debug)]
pub enum CaptchaStatus {
    Encounter,
    Success,
    Failure,
    Error,
}

impl ToString for CaptchaStatus {
    fn to_string(&self) -> String {
        (match *self {
            CaptchaStatus::Encounter => "encounter",
            CaptchaStatus::Success => "success",
            CaptchaStatus::Failure => "failure",
            CaptchaStatus::Error => "error",
        }).to_owned()
    }
}

impl<'de> Deserialize<'de> for CaptchaStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        let value: &str = &String::deserialize(deserializer)?;

        // Rust does not come with a simple way of converting a
        // number to an enum, so use a big `match`.
        match value {
            "encounter" => Ok(CaptchaStatus::Encounter),
            "success" => Ok(CaptchaStatus::Success),
            "failure" => Ok(CaptchaStatus::Failure),
            "error" => Ok(CaptchaStatus::Error),
            _ => Err(serde::de::Error::custom(format!("unknown CaptchaStatus value: {}", value))),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
// #[serde(deny_unknown_fields)]
pub struct Quest {
    #[serde(rename = "type")]
    pub quest_type: u8,
    pub target: u8,
    pub template: String,
    pub rewards: Value,
    pub updated: i64,
    pub pokestop_url: String,
    pub latitude: f64,
    pub longitude: f64,
    pub pokestop_id: String,
    pub pokestop_name: String,
    pub conditions: Value,
}

#[derive(Clone, Debug, Deserialize)]
// #[serde(deny_unknown_fields)]
pub struct Weather {
    pub latitude: f64,
    pub cloud_level: u8,
    pub severity: u8,
    pub polygon: LineString<f64>,
    pub special_effect_level: u8,
    pub longitude: f64,
    pub id: u64,
    pub rain_level: u8,
    pub fog_level: u8,
    pub wind_direction: u16,
    pub snow_level: u8,
    pub warn_weather: bool,
    pub updated: u8,
    pub gameplay_condition: u8,
    pub wind_level: u8
}

impl Weather {
    pub fn diff(&self, other: &Weather) -> String {
        let mut diff = String::new();
        // if self.latitude != other.latitude {
        //     diff.push_str(&format!("latitude changed from {} to {}", self.latitude, other.latitude));
        // }
        if self.cloud_level != other.cloud_level {
            diff.push_str(&format!("cloud_level changed from {} to {}", self.cloud_level, other.cloud_level));
        }
        if self.severity != other.severity {
            diff.push_str(&format!("severity changed from {} to {}", self.severity, other.severity));
        }
        if self.polygon != other.polygon {
            diff.push_str(&format!("polygon changed from {:?} to {:?}", self.polygon, other.polygon));
        }
        if self.special_effect_level != other.special_effect_level {
            diff.push_str(&format!("special_effect_level changed from {} to {}", self.special_effect_level, other.special_effect_level));
        }
        // if self.longitude != other.longitude {
        //     diff.push_str(&format!("longitude changed from {} to {}", self.longitude, other.longitude));
        // }
        // if self.id != other.id {
        //     diff.push_str(&format!("id changed from {} to {}", self.id, other.id));
        // }
        if self.rain_level != other.rain_level {
            diff.push_str(&format!("rain_level changed from {} to {}", self.rain_level, other.rain_level));
        }
        if self.fog_level != other.fog_level {
            diff.push_str(&format!("fog_level changed from {} to {}", self.fog_level, other.fog_level));
        }
        // if self.wind_direction != other.wind_direction {
        //     diff.push_str(&format!("wind_direction changed from {} to {}", self.wind_direction, other.wind_direction));
        // }
        if self.snow_level != other.snow_level {
            diff.push_str(&format!("snow_level changed from {} to {}", self.snow_level, other.snow_level));
        }
        if self.warn_weather != other.warn_weather {
            diff.push_str(&format!("warn_weather changed from {} to {}", self.warn_weather, other.warn_weather));
        }
        if self.updated != other.updated {
            diff.push_str(&format!("updated changed from {} to {}", self.updated, other.updated));
        }
        if self.gameplay_condition != other.gameplay_condition {
            diff.push_str(&format!("gameplay_condition changed from {} to {}", self.gameplay_condition, other.gameplay_condition));
        }
        if self.wind_level != other.wind_level {
            diff.push_str(&format!("cloud_level changed from {} to {}", self.wind_level, other.wind_level));
        }
        diff
    }
}

impl PartialEq for Weather {
    fn eq(&self, other: &Weather) -> bool {
        // self.latitude == other.latitude &&
            self.cloud_level == other.cloud_level &&
            self.severity == other.severity &&
            self.polygon == other.polygon &&
            self.special_effect_level == other.special_effect_level &&
            // self.longitude == other.longitude &&
            // self.id == other.id &&
            self.rain_level == other.rain_level &&
            self.fog_level == other.fog_level &&
            // self.wind_direction == other.wind_direction &&
            self.snow_level == other.snow_level &&
            self.warn_weather == other.warn_weather &&
            self.updated == other.updated &&
            self.gameplay_condition == other.gameplay_condition &&
            self.wind_level == other.wind_level
    }
}

/// Meteo watch request
#[derive(Debug, Deserialize)]
pub struct Watch {
    pub user_id: String,
    pub point: Point<f64>,
    pub expire: i64,
    #[serde(skip_deserializing)]
    #[serde(default)]
    pub reference_weather: UnsafeCell<Option<Weather>>,
}

unsafe impl Sync for Watch {}

impl PartialEq for Watch {
    fn eq(&self, other: &Watch) -> bool {
        self.user_id == other.user_id &&
            self.point == other.point &&
            self.expire == other.expire
    }
}

impl Clone for Watch {
    fn clone(&self) -> Self {
        Watch {
            user_id: self.user_id.clone(),
            point: self.point,
            expire: self.expire,
            reference_weather: UnsafeCell::new(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Request, Weather};

    #[test]
    fn entities() {
        let strings = [
            r#"[{"type":"pokemon","message":{"latitude":45.434227,"pokemon_id":152,"last_modified_time":1571340934,"form":0,"username":null,"move_1":null,"gender":1,"individual_attack":null,"disappear_time":1571342134,"first_seen":1571340934,"shiny":null,"pokestop_id":"044826036eeb40759443a78f577cfdec.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":11.898616,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"9337502859261344419","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":39.244372,"pokemon_id":50,"last_modified_time":1571340933,"form":59,"username":null,"move_1":null,"gender":1,"individual_attack":null,"disappear_time":1571342133,"first_seen":1571340933,"shiny":null,"pokestop_id":"3345943e0b5b4be4b979f7de0a7e670e.16","display_pokemon_id":null,"weather":1,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":9.129608,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"13739501734267482213","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.614667,"pokemon_id":109,"last_modified_time":1571340931,"form":0,"username":null,"move_1":null,"gender":2,"individual_attack":null,"disappear_time":1571342131,"first_seen":1571340931,"shiny":null,"pokestop_id":"66a436769e454f7289b7757ec0f7c452.16","display_pokemon_id":null,"weather":4,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":8.750507,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"15459186372986583939","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.5353581362712,"pokemon_id":167,"last_modified_time":1571340929,"form":0,"username":"F01kAndMote1","move_1":201,"gender":1,"individual_attack":2,"disappear_time":1571342128,"first_seen":1571340928,"shiny":false,"pokestop_id":"None","display_pokemon_id":null,"weather":0,"disappear_time_verified":false,"individual_stamina":4,"height":0.483411848545074,"longitude":10.2265579859696,"weight":7.72620534896851,"move_2":99,"spawnpoint_id":"17613FC7","costume":0,"individual_defense":13,"encounter_id":"7937848107123624936","pokemon_level":5,"cp":93}},{"type":"pokemon","message":{"latitude":45.614667,"pokemon_id":504,"last_modified_time":1571340931,"form":0,"username":null,"move_1":null,"gender":2,"individual_attack":null,"disappear_time":1571342131,"first_seen":1571340931,"shiny":null,"pokestop_id":"66a436769e454f7289b7757ec0f7c452.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":8.750507,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"3871420704590710779","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":39.244476,"pokemon_id":16,"last_modified_time":1571340933,"form":0,"username":null,"move_1":null,"gender":1,"individual_attack":null,"disappear_time":1571342133,"first_seen":1571340933,"shiny":null,"pokestop_id":"fe02c360864d4ed7af2bfedc29adc34d.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":9.128108,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"1599962362552638786","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.6245392199906,"pokemon_id":23,"last_modified_time":1571340933,"form":0,"username":"mELVin44555470","move_1":null,"gender":2,"individual_attack":null,"disappear_time":1571344517,"first_seen":1571340933,"shiny":null,"pokestop_id":"None","display_pokemon_id":null,"weather":4,"disappear_time_verified":true,"individual_stamina":null,"height":null,"longitude":9.55718657596311,"weight":null,"move_2":null,"spawnpoint_id":"14D04487","costume":0,"individual_defense":null,"encounter_id":"3654645982118858632","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":43.798217,"pokemon_id":506,"last_modified_time":1571340932,"form":0,"username":null,"move_1":null,"gender":1,"individual_attack":null,"disappear_time":1571342132,"first_seen":1571340932,"shiny":null,"pokestop_id":"508a88cfb9614690ba5d1ebd4f10fdba.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":11.25377,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"6468067052861546441","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.428046,"pokemon_id":19,"last_modified_time":1571340934,"form":46,"username":null,"move_1":null,"gender":2,"individual_attack":null,"disappear_time":1571342134,"first_seen":1571340934,"shiny":null,"pokestop_id":"249c59918858472a9046371ba649f287.16","display_pokemon_id":null,"weather":3,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":11.9141,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"11299195651896064658","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.6297850619696,"pokemon_id":513,"last_modified_time":1571340933,"form":0,"username":null,"move_1":220,"gender":1,"individual_attack":14,"disappear_time":1571342322,"first_seen":1571340897,"shiny":false,"pokestop_id":"adf989574e3448409253878e1a68c5ac.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":true,"individual_stamina":7,"height":0.665367245674133,"longitude":8.77794617947646,"weight":13.4390907287598,"move_2":102,"spawnpoint_id":"68A0EC13","costume":0,"individual_defense":8,"encounter_id":"10079290544336428363","pokemon_level":6,"cp":147}},{"type":"pokemon","message":{"latitude":45.42264,"pokemon_id":335,"last_modified_time":1571340934,"form":0,"username":null,"move_1":null,"gender":2,"individual_attack":null,"disappear_time":1571342134,"first_seen":1571340934,"shiny":null,"pokestop_id":"30e853a8f2504dc5b4c312ac57253097.16","display_pokemon_id":null,"weather":3,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":11.907761,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"12627127150687356555","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.425588,"pokemon_id":19,"last_modified_time":1571340934,"form":45,"username":null,"move_1":null,"gender":1,"individual_attack":null,"disappear_time":1571342134,"first_seen":1571340934,"shiny":null,"pokestop_id":"047eab5ab370476283cb9443f161eec2.16","display_pokemon_id":null,"weather":3,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":11.89585,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"13607076099936033702","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.530376,"pokemon_id":23,"last_modified_time":1571340930,"form":0,"username":null,"move_1":null,"gender":1,"individual_attack":null,"disappear_time":1571342130,"first_seen":1571340930,"shiny":null,"pokestop_id":"27144282d0774a539c58a9510f3beceb.16","display_pokemon_id":null,"weather":4,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":9.29299,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"9026765524759176962","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.614667,"pokemon_id":296,"last_modified_time":1571340931,"form":0,"username":null,"move_1":null,"gender":1,"individual_attack":null,"disappear_time":1571342131,"first_seen":1571340931,"shiny":null,"pokestop_id":"66a436769e454f7289b7757ec0f7c452.16","display_pokemon_id":null,"weather":4,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":8.750507,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"1003861876549825228","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.5359826265233,"pokemon_id":263,"last_modified_time":1571340929,"form":0,"username":"F01kAndMote1","move_1":221,"gender":1,"individual_attack":10,"disappear_time":1571341528,"first_seen":1571339432,"shiny":false,"pokestop_id":"b6a67825a93d46a58c17445869b1a672.11","display_pokemon_id":null,"weather":3,"disappear_time_verified":false,"individual_stamina":10,"height":0.420394837856293,"longitude":10.2270625947265,"weight":20.1207485198975,"move_2":79,"spawnpoint_id":"1761401F","costume":0,"individual_defense":12,"encounter_id":"2934188249265032581","pokemon_level":7,"cp":89}},{"type":"pokemon","message":{"latitude":45.6296192282641,"pokemon_id":322,"last_modified_time":1571340933,"form":0,"username":null,"move_1":209,"gender":1,"individual_attack":9,"disappear_time":1571341899,"first_seen":1571340485,"shiny":false,"pokestop_id":"adf989574e3448409253878e1a68c5ac.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":true,"individual_stamina":15,"height":0.754267156124115,"longitude":8.7781669787064,"weight":26.1670875549316,"move_2":127,"spawnpoint_id":"68A0EC01","costume":0,"individual_defense":8,"encounter_id":"4797657064781675781","pokemon_level":30,"cp":833}},{"type":"pokemon","message":{"latitude":45.5353733220791,"pokemon_id":506,"last_modified_time":1571340929,"form":0,"username":"F01kAndMote1","move_1":221,"gender":1,"individual_attack":7,"disappear_time":1571341942,"first_seen":1571340742,"shiny":false,"pokestop_id":"f7246c3797c845e0bd6604b93909b206.11","display_pokemon_id":null,"weather":3,"disappear_time_verified":false,"individual_stamina":14,"height":0.431397616863251,"longitude":10.2272150190148,"weight":5.67432689666748,"move_2":79,"spawnpoint_id":"1761156B","costume":0,"individual_defense":15,"encounter_id":"2041069550696825550","pokemon_level":20,"cp":487}},{"type":"pokemon","message":{"latitude":43.712569,"pokemon_id":138,"last_modified_time":1571340929,"form":0,"username":"1zZ7vPu7282","move_1":null,"gender":2,"individual_attack":null,"disappear_time":1571342129,"first_seen":1571340929,"shiny":null,"pokestop_id":"915149f1ded54f31866ca32c409377e0.16","display_pokemon_id":null,"weather":3,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":10.39165,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"10583829943947231920","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":39.237269,"pokemon_id":228,"last_modified_time":1571340933,"form":229,"username":null,"move_1":null,"gender":2,"individual_attack":null,"disappear_time":1571342133,"first_seen":1571340933,"shiny":null,"pokestop_id":"603ff62b98b448e1847315818f7e5a66.16","display_pokemon_id":null,"weather":1,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":9.125565,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"13908330070378532891","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.434227,"pokemon_id":311,"last_modified_time":1571340934,"form":0,"username":null,"move_1":null,"gender":2,"individual_attack":null,"disappear_time":1571342134,"first_seen":1571340934,"shiny":null,"pokestop_id":"044826036eeb40759443a78f577cfdec.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":11.898616,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"16273246905926691201","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.6148640584045,"pokemon_id":102,"last_modified_time":1571340931,"form":0,"username":null,"move_1":null,"gender":2,"individual_attack":null,"disappear_time":1571342131,"first_seen":1571340931,"shiny":null,"pokestop_id":"None","display_pokemon_id":null,"weather":0,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":8.75440084613575,"weight":null,"move_2":null,"spawnpoint_id":"661CFE73","costume":0,"individual_defense":null,"encounter_id":"3345644973111627797","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.6297465786119,"pokemon_id":513,"last_modified_time":1571340933,"form":0,"username":null,"move_1":220,"gender":1,"individual_attack":14,"disappear_time":1571342612,"first_seen":1571340897,"shiny":false,"pokestop_id":"adf989574e3448409253878e1a68c5ac.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":true,"individual_stamina":6,"height":0.551975131034851,"longitude":8.77748091717895,"weight":10.0745096206665,"move_2":279,"spawnpoint_id":"68A0EC87","costume":0,"individual_defense":15,"encounter_id":"329433902305583244","pokemon_level":11,"cp":289}},{"type":"pokemon","message":{"latitude":45.624283,"pokemon_id":198,"last_modified_time":1571340931,"form":0,"username":"aUGUSt21816563","move_1":null,"gender":2,"individual_attack":null,"disappear_time":1571342131,"first_seen":1571340931,"shiny":null,"pokestop_id":"115c0d9d63c3478190363d452b5b399f.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":9.559576,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"13282595226536731834","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":39.232158,"pokemon_id":351,"last_modified_time":1571340933,"form":30,"username":null,"move_1":null,"gender":1,"individual_attack":null,"disappear_time":1571342133,"first_seen":1571340933,"shiny":null,"pokestop_id":"49244daaa6a34c61a45d8074193b961b.16","display_pokemon_id":null,"weather":1,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":9.130352,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"15868294529733383848","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.4279932204317,"pokemon_id":161,"last_modified_time":1571340934,"form":0,"username":null,"move_1":null,"gender":1,"individual_attack":null,"disappear_time":1571342688,"first_seen":1571340934,"shiny":null,"pokestop_id":"None","display_pokemon_id":null,"weather":3,"disappear_time_verified":true,"individual_stamina":null,"height":null,"longitude":11.9036936594722,"weight":null,"move_2":null,"spawnpoint_id":"EDA846B1","costume":0,"individual_defense":null,"encounter_id":"3386015148739198686","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.424588,"pokemon_id":52,"last_modified_time":1571340934,"form":63,"username":null,"move_1":null,"gender":1,"individual_attack":null,"disappear_time":1571342134,"first_seen":1571340934,"shiny":null,"pokestop_id":"8a919059f69a470482cf8b0257f105f7.16","display_pokemon_id":null,"weather":3,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":11.90707,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"17698878014123675416","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.630531,"pokemon_id":58,"last_modified_time":1571340933,"form":280,"username":null,"move_1":null,"gender":2,"individual_attack":null,"disappear_time":1571342133,"first_seen":1571340933,"shiny":null,"pokestop_id":"aa19a39454be409d8e186843c7975a81.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":8.778861,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"9277026429009062441","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.429989,"pokemon_id":504,"last_modified_time":1571340934,"form":0,"username":null,"move_1":null,"gender":2,"individual_attack":null,"disappear_time":1571342134,"first_seen":1571340934,"shiny":null,"pokestop_id":"38659e4fb3654f33b9bd56c20097df88.16","display_pokemon_id":null,"weather":3,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":11.89411,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"130108842469552560","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":43.7624837195591,"pokemon_id":353,"last_modified_time":1571340931,"form":908,"username":"1zZ7vPu7159","move_1":238,"gender":2,"individual_attack":9,"disappear_time":1571341038,"first_seen":1571339382,"shiny":false,"pokestop_id":"2f0f8d05e9274018940a3aa08e1644d9.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":true,"individual_stamina":1,"height":0.659983694553375,"longitude":11.2487040543162,"weight":2.62425994873047,"move_2":69,"spawnpoint_id":"A515692F","costume":0,"individual_defense":14,"encounter_id":"4506393899040844654","pokemon_level":14,"cp":369}},{"type":"pokemon","message":{"latitude":45.425067,"pokemon_id":10,"last_modified_time":1571340934,"form":0,"username":null,"move_1":null,"gender":1,"individual_attack":null,"disappear_time":1571342134,"first_seen":1571340934,"shiny":null,"pokestop_id":"bbe9740d5d554718990ab92a10be47ad.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":11.904112,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"16243784799904988806","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.530376,"pokemon_id":351,"last_modified_time":1571340930,"form":29,"username":null,"move_1":null,"gender":1,"individual_attack":null,"disappear_time":1571342130,"first_seen":1571340930,"shiny":null,"pokestop_id":"27144282d0774a539c58a9510f3beceb.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":9.29299,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"18350671066427288965","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.427997,"pokemon_id":311,"last_modified_time":1571340934,"form":0,"username":null,"move_1":null,"gender":2,"individual_attack":null,"disappear_time":1571342134,"first_seen":1571340934,"shiny":null,"pokestop_id":"e0c99a94e9a3490985ec344b41a328ff.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":11.912422,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"13282669476707636987","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.42264,"pokemon_id":415,"last_modified_time":1571340934,"form":0,"username":null,"move_1":null,"gender":1,"individual_attack":null,"disappear_time":1571342134,"first_seen":1571340934,"shiny":null,"pokestop_id":"30e853a8f2504dc5b4c312ac57253097.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":11.907761,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"8991254025708004506","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.6297055631789,"pokemon_id":337,"last_modified_time":1571340933,"form":0,"username":null,"move_1":227,"gender":3,"individual_attack":10,"disappear_time":1571342060,"first_seen":1571340485,"shiny":false,"pokestop_id":"adf989574e3448409253878e1a68c5ac.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":true,"individual_stamina":9,"height":0.957226991653442,"longitude":8.77801229741794,"weight":163.520324707031,"move_2":87,"spawnpoint_id":"68A0EC0F","costume":0,"individual_defense":1,"encounter_id":"17656903909260811475","pokemon_level":10,"cp":612}},{"type":"pokemon","message":{"latitude":45.5359419728579,"pokemon_id":299,"last_modified_time":1571340929,"form":0,"username":"F01kAndMote1","move_1":227,"gender":1,"individual_attack":10,"disappear_time":1571341429,"first_seen":1571340928,"shiny":false,"pokestop_id":"None","display_pokemon_id":null,"weather":3,"disappear_time_verified":true,"individual_stamina":15,"height":0.837561845779419,"longitude":10.2275133291557,"weight":79.2425231933594,"move_2":64,"spawnpoint_id":"17616AC1","costume":0,"individual_defense":8,"encounter_id":"14436814492498820926","pokemon_level":20,"cp":530}},{"type":"pokemon","message":{"latitude":45.5352209971958,"pokemon_id":255,"last_modified_time":1571340929,"form":0,"username":"F01kAndMote1","move_1":220,"gender":2,"individual_attack":10,"disappear_time":1571342384,"first_seen":1571340742,"shiny":false,"pokestop_id":"f7246c3797c845e0bd6604b93909b206.11","display_pokemon_id":null,"weather":0,"disappear_time_verified":true,"individual_stamina":4,"height":0.500354647636414,"longitude":10.2272531241706,"weight":3.54278707504272,"move_2":101,"spawnpoint_id":"1761150D","costume":0,"individual_defense":4,"encounter_id":"11142824888287159710","pokemon_level":1,"cp":13}},{"type":"pokemon","message":{"latitude":45.4284794928577,"pokemon_id":200,"last_modified_time":1571340934,"form":0,"username":null,"move_1":null,"gender":1,"individual_attack":null,"disappear_time":1571342631,"first_seen":1571340934,"shiny":null,"pokestop_id":"None","display_pokemon_id":null,"weather":0,"disappear_time_verified":true,"individual_stamina":null,"height":null,"longitude":11.903323376342,"weight":null,"move_2":null,"spawnpoint_id":"EDA840E1","costume":0,"individual_defense":null,"encounter_id":"12222187122069625456","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.4282266809273,"pokemon_id":316,"last_modified_time":1571340934,"form":0,"username":null,"move_1":null,"gender":1,"individual_attack":null,"disappear_time":1571342547,"first_seen":1571340934,"shiny":null,"pokestop_id":"None","display_pokemon_id":null,"weather":0,"disappear_time_verified":true,"individual_stamina":null,"height":null,"longitude":11.9036003255013,"weight":null,"move_2":null,"spawnpoint_id":"EDA84133","costume":0,"individual_defense":null,"encounter_id":"257970549060122030","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.427707,"pokemon_id":198,"last_modified_time":1571340934,"form":0,"username":null,"move_1":null,"gender":1,"individual_attack":null,"disappear_time":1571342134,"first_seen":1571340934,"shiny":null,"pokestop_id":"d2549fc034e049bbade1d4dc0199d1a2.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":11.912638,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"10875397485394718753","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":45.426202,"pokemon_id":401,"last_modified_time":1571340934,"form":0,"username":null,"move_1":null,"gender":2,"individual_attack":null,"disappear_time":1571342134,"first_seen":1571340934,"shiny":null,"pokestop_id":"793d2dfbbbaf460dbff1c862bb40c894.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":11.913314,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"13272982726927939395","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":39.240988,"pokemon_id":353,"last_modified_time":1571340933,"form":908,"username":null,"move_1":null,"gender":1,"individual_attack":null,"disappear_time":1571342133,"first_seen":1571340933,"shiny":null,"pokestop_id":"3d7153f74b8240a0a199ec98df25d81d.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":9.124766,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"11732361233280770422","pokemon_level":null,"cp":null}},{"type":"pokemon","message":{"latitude":39.237269,"pokemon_id":261,"last_modified_time":1571340933,"form":0,"username":null,"move_1":null,"gender":1,"individual_attack":null,"disappear_time":1571342133,"first_seen":1571340933,"shiny":null,"pokestop_id":"603ff62b98b448e1847315818f7e5a66.16","display_pokemon_id":null,"weather":0,"disappear_time_verified":false,"individual_stamina":null,"height":null,"longitude":9.125565,"weight":null,"move_2":null,"spawnpoint_id":"None","costume":0,"individual_defense":null,"encounter_id":"16096881351736935844","pokemon_level":null,"cp":null}},{"type":"invasion","message":{"pokestop_id":"a012a821325646ec9a40153941330361.16","lure_id":501,"latitude":43.907973,"url":"http://lh3.googleusercontent.com/Hk-rtkyQ6iVg1MB0RMC4NobmZeNwfPd3pKCObXBuwb8eenLuAFJQ_PzfLkfRB1BxhCcozNuqEtTZIgRHQZ4f8A","longitude":12.89026,"incident_expire_timestamp":1571342571,"grunt_type":32,"name":"Parco","updated":1571340930,"lure_expiration":0,"enabled":true,"pokestop_display":1,"last_modified":1571340930}},{"type":"invasion","message":{"pokestop_id":"84b96c094c1e44e2afd0d781ea4ac2ca.16","lure_id":0,"latitude":45.470302,"url":"http://lh4.ggpht.com/LVWcWjY0_0i5vM2DqrnJOupCpyjgx3jNJG4jsXky4cYtA7rJAtSJP_ltOHhYsO0PJonG3KnWVf8GFwQLfVo","longitude":9.163076,"incident_expire_timestamp":1571342554,"grunt_type":25,"name":"Palazzo Dei Bambini ","updated":1571340930,"lure_expiration":0,"enabled":true,"pokestop_display":1,"last_modified":1571340758}},{"type":"invasion","message":{"pokestop_id":"59ed04785e0b4878bb3218ccf248e139.16","lure_id":0,"latitude":43.718782,"url":"http://lh3.googleusercontent.com/gdEfUTiV6Ox-viclz10S1xvFfHaqdR6-fBLuN0Soz5fHe9Ie-TGvZUw7EDKExwKqQbKr1eAANnISJ0Nq_zKt","longitude":10.396305,"incident_expire_timestamp":1571342727,"grunt_type":4,"name":"Museo Botanico","updated":1571340930,"lure_expiration":0,"enabled":true,"pokestop_display":1,"last_modified":1571340928}},{"type":"invasion","message":{"pokestop_id":"c5b1e7abbad440d497c0c4eaf91d4fa3.16","lure_id":503,"latitude":45.242574,"url":"http://lh5.ggpht.com/as178Irx8ccN5AKjkGWqEQO0SVjuPd6KNZOn4aJPRPNGSRQN196Z5_nZuOnoRUIePQzPxGU2W1cBVzzMqPojVw","longitude":11.751994,"incident_expire_timestamp":1571341434,"grunt_type":31,"name":"Torre Biblioteca","updated":1571340931,"lure_expiration":0,"enabled":true,"pokestop_display":1,"last_modified":1571339635}},{"type":"invasion","message":{"pokestop_id":"f10143e1cac249b1b6be7a877271b4f7.16","lure_id":504,"latitude":45.23943,"url":"http://lh5.ggpht.com/499Wv_1d9U3F4-Npk-7zPagRP0I4LLUnRGuGp3fKVSuvPxyaSLShQyQ1j_SBsjgXa3LLxoJSwFF08-G9BveX","longitude":11.750895,"incident_expire_timestamp":1571342675,"grunt_type":38,"name":"Comune di Monselice","updated":1571340931,"lure_expiration":0,"enabled":true,"pokestop_display":1,"last_modified":1571340904}},{"type":"invasion","message":{"pokestop_id":"d75033772b174256be5c3f4cb5d14536.16","lure_id":501,"latitude":45.46566,"url":"http://lh3.ggpht.com/n7v_eiO8iJtaET-4KGvGu3kaX5PjX6k83LHJsbQkpmffSKEhPYKlPK51NNYi5waub2bYSlzyX8mipnFZmSqIAg","longitude":9.15578,"incident_expire_timestamp":1571342527,"grunt_type":4,"name":"Bocchin Del Tubo D'Elba","updated":1571340930,"lure_expiration":0,"enabled":true,"pokestop_display":1,"last_modified":1571340731}},{"type":"invasion","message":{"pokestop_id":"2183c59411a94711a35e9b8490c36253.16","lure_id":501,"latitude":45.466748,"url":"http://lh6.ggpht.com/gF1MjBll9kRkRW1_WX-0YQ3Sm82LPDKEjK4cV-RgDR8UFqFC4GdKTfDvT5Mzq7fCS7Tj34oaNavLztpy-uuhEA","longitude":9.151489,"incident_expire_timestamp":1571342393,"grunt_type":23,"name":"Mondadori Multicenter","updated":1571340930,"lure_expiration":0,"enabled":true,"pokestop_display":1,"last_modified":1571340602}},{"type":"invasion","message":{"pokestop_id":"5069f806b2a5498ea07103b5f8f53894.16","lure_id":501,"latitude":45.239771,"url":"http://lh6.ggpht.com/G3jjfKtxW31gP2fADt1mEDhaquHL3tPqqumvCIFR_g8zVgJX6rEFYjL6uOsfGkvm7X9k-qKSk_XUmEI-WO0uyQwg7PzPRJxpAdRSt9gmIqetXdFG","longitude":11.750674,"incident_expire_timestamp":1571342624,"grunt_type":31,"name":"Memoriale ai caduti","updated":1571340931,"lure_expiration":0,"enabled":true,"pokestop_display":1,"last_modified":1571340833}},{"type":"invasion","message":{"pokestop_id":"ee9ecb98a061482d9fd8e3fd7115244c.16","lure_id":502,"latitude":45.464673,"url":"http://lh6.ggpht.com/DW28T1Fm_VPh0vKiayWBsRF9XOSa2ajR45uLyvRLIzRWwmQzpmRJXFnQ5nfwh-PpTdSkNURGrClUYBm0vPc","longitude":9.164507,"incident_expire_timestamp":1571342724,"grunt_type":38,"name":"Al Rifugio Di P. Baracca ","updated":1571340933,"lure_expiration":0,"enabled":true,"pokestop_display":1,"last_modified":1571340924}},{"type":"quest","message":{"pokestop_id":"92396690130041ea9954c69c1a7036ce.16","rewards":[{"type":7,"info":{"form_id":0,"costume_id":0,"pokemon_id":263,"gender_id":0,"shiny":false}}],"latitude":43.848406,"pokestop_name":"Stadio Filippo Raciti","type":4,"updated":1571340931,"conditions":[{"type":1,"info":{"pokemon_type_ids":[1]}}],"longitude":10.970345,"target":5,"template":"t3_2019_catch_normal_easy_pkmn","pokestop_url":"http://lh3.googleusercontent.com/s_oSMPzXFaZFY1jCJgJrsG9GOmTkANM7cCUJwrJKw9zPPYTgU0TOwNQX7J47XrXAAlPeeM17pg7VhH5dgLrO"}},{"type":"quest","message":{"pokestop_id":"63e85cb37d8c40d2a5af04044ca7b76b.16","rewards":[{"type":2,"info":{"amount":1,"item_id":705}}],"latitude":43.864931,"pokestop_name":"La Nave","type":16,"updated":1571340932,"conditions":[{"type":14,"info":{"hit":false,"throw_type_id":10}},{"type":15}],"longitude":10.245531,"target":2,"template":"challenge_land_nice_curve_inarow_easy","pokestop_url":"http://lh3.ggpht.com/TpHxUyjYm4JNp7e8fL58rJdFZ59n3YCyyDImRQuh6bueq9nO_yhd9tOt-LpsZ5Ttlzj9ml4lrut_yIr_ra--"}},{"type":"quest","message":{"pokestop_id":"5f75539e8f834a32ba1c457c466ba488.16","rewards":[{"type":7,"info":{"form_id":0,"costume_id":0,"pokemon_id":86,"gender_id":0,"shiny":false}}],"latitude":43.797328,"pokestop_name":"Giardino Doposcuola Firenze Careggi","type":4,"updated":1571340929,"conditions":[{"type":1,"info":{"pokemon_type_ids":[15,12]}}],"longitude":11.244711,"target":4,"template":"t3_2019_catch_types_medium_pkmn","pokestop_url":"http://lh6.ggpht.com/GXFJ_k65c4oPtsOrAVApOkc7yx0eozmPIJ8Ma6LG14gak1WlbkGKUmP89_IKa1yTrt0y-HgIJq_HcNzrzakKBw"}},{"type":"quest","message":{"pokestop_id":"9248e2851fdb4de68d1c9e95b40e40e0.16","rewards":[{"type":7,"info":{"form_id":0,"costume_id":0,"pokemon_id":246,"gender_id":0,"shiny":false}}],"latitude":39.354124,"pokestop_name":"Parodia del Monte Rushmore ","type":16,"updated":1571340931,"conditions":[{"type":14,"info":{"hit":false,"throw_type_id":12}}],"longitude":16.231835,"target":3,"template":"challenge_land_excellent_inarow_veryhard_pkmn","pokestop_url":"http://lh3.googleusercontent.com/4qr93fpnV5Y5NQuBuONTvO8CdXd1hi9LURcsiv9Vbl8Hr9BD09Adk-Dno2dhdKsB4pL4VtpEGUCKcuFDPh1G"}},{"type":"quest","message":{"pokestop_id":"cb96d70215714c8586d053a0589b3eb2.16","rewards":[{"type":7,"info":{"form_id":0,"costume_id":0,"pokemon_id":129,"gender_id":0,"shiny":false}}],"latitude":43.714713,"pokestop_name":"Chiesa di Santa Maria Maddalena","type":4,"updated":1571340932,"conditions":[],"longitude":10.398856,"target":10,"template":"challenge_catch_easy_pkmn","pokestop_url":"http://lh3.googleusercontent.com/zzvhmaApr8ecY-biL99uBisGnprz6McMOZiJxNTZa_qkhPe6Ugwd_dVyfw4YyDLiI1-KkfBFRLdWxSfS-EhALg"}},{"type":"gym_details","message":{"latitude":45.466619,"slots_available":1,"name":"\"Il Cavallo del Mare\" Parco Delle Belle Arti","url":"http://lh3.googleusercontent.com/8gQa_8FjhPhcAXsTDOj6FfDb22byJcuxw5hNXdvFVi9wmhRAG59vGe-8u1Nht_Nm3thpVkRXshvNOVl35QGm","longitude":9.154452,"id":"8ddaa8a0d4dd4bf4b38f62c768a0b07a.11","ex_raid_eligible":true,"in_battle":true,"team":3}},{"type":"raid","message":{"level":1,"latitude":40.826146,"pokemon_id":0,"is_exclusive":false,"form":0,"longitude":8.416153,"ex_raid_eligible":false,"start":1571342678,"gym_name":"Campetto di calcio a 5.","move_1":0,"move_2":0,"gender":0,"gym_id":"36c6b6acdc0c4820920acce7afa5aa49.16","team_id":3,"end":1571345378,"gym_url":"http://lh3.googleusercontent.com/zqXXTEI_NOgIZRZW9sWNXA2L6vz3QU9YQUmusRJM3mCXdmQ3g4UFsutDbjlEB6pAg8L32XhDeG8Nir7XL_S7LuTWxw","spawn":1571339078,"cp":0}},{"type":"weather","message":{"latitude":45.4708623624896,"cloud_level":1,"severity":0,"polygon":[[45.4293499926637,11.9455630525467],[45.5024707283596,11.9765478474091],[45.5124252464723,11.8822977972565],[45.4392542159797,11.8515426867682]],"special_effect_level":0,"longitude":11.9139762963947,"id":5151784821688631296,"rain_level":0,"fog_level":0,"wind_direction":211,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":3,"wind_level":0}},{"type":"weather","message":{"latitude":43.8280086998633,"cloud_level":2,"severity":0,"polygon":[[43.7945850878676,10.9807003113263],[43.7857783639575,11.071175792204],[43.8613668356012,11.071175792204],[43.8701745107509,10.9807003113263]],"special_effect_level":0,"longitude":11.0259450112985,"id":1381020689347117056,"rain_level":0,"fog_level":0,"wind_direction":276,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":4,"wind_level":0}},{"type":"weather","message":{"latitude":45.558593533964,"cloud_level":3,"severity":0,"polygon":[[45.5177939726209,9.38804970571081],[45.5919055424714,9.41259333792649],[45.5994404022873,9.32121735603331],[45.5252903088402,9.29690351090113]],"special_effect_level":0,"longitude":9.35468115793202,"id":5154008034199994368,"rain_level":0,"fog_level":0,"wind_direction":225,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":4,"wind_level":0}},{"type":"weather","message":{"latitude":43.8012162436135,"cloud_level":2,"severity":0,"polygon":[[43.7679178210492,11.2523792165521],[43.7588636360255,11.3431058345569],[43.8344491562498,11.3431058345569],[43.8435043415869,11.2523792165521]],"special_effect_level":0,"longitude":11.297749700792,"id":1381009694230839296,"rain_level":0,"fog_level":0,"wind_direction":299,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":4,"wind_level":0}},{"type":"weather","message":{"latitude":43.7255718466817,"cloud_level":2,"severity":0,"polygon":[[43.6922144976459,11.2523792165521],[43.683161377657,11.3431058345569],[43.7588636360255,11.3431058345569],[43.7679178210492,11.2523792165521]],"special_effect_level":0,"longitude":11.297749700792,"id":1381007495207583744,"rain_level":0,"fog_level":0,"wind_direction":299,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":4,"wind_level":0}},{"type":"weather","message":{"latitude":43.8283110882444,"cloud_level":1,"severity":0,"polygon":[[43.7944582391897,10.1703257316642],[43.7863845843273,10.2600124940266],[43.8620985377482,10.2600124940266],[43.8701730658909,10.1703257316642]],"special_effect_level":0,"longitude":10.2151754374476,"id":1357170083117400064,"rain_level":0,"fog_level":0,"wind_direction":62,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":3,"wind_level":0}},{"type":"weather","message":{"latitude":43.7362304269065,"cloud_level":1,"severity":0,"polygon":[[43.7023999862442,10.3497894692016],[43.6941659708343,10.439656018971],[43.7699953894573,10.439656018971],[43.7782303670409,10.3497894692016]],"special_effect_level":0,"longitude":10.394729208151,"id":1357150291908100096,"rain_level":0,"fog_level":0,"wind_direction":286,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":3,"wind_level":0}},{"type":"weather","message":{"latitude":45.6175040448251,"cloud_level":2,"severity":0,"polygon":[[45.5765862205904,9.59563978540641],[45.6507285387613,9.62074782024341],[45.6584694300093,9.52894506342497],[45.5842875575441,9.5040677015903]],"special_effect_level":0,"longitude":9.56234009324671,"id":5152484111083896832,"rain_level":0,"fog_level":0,"wind_direction":231,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":4,"wind_level":0}},{"type":"weather","message":{"latitude":43.8102295885151,"cloud_level":2,"severity":0,"polygon":[[43.7768893362968,11.1617356477747],[43.7679178210492,11.2523792165521],[43.8435043415869,11.2523792165521],[43.852476840566,11.1617356477747]],"special_effect_level":0,"longitude":11.2070645352827,"id":1381011893254094848,"rain_level":0,"fog_level":0,"wind_direction":299,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":4,"wind_level":0}},{"type":"weather","message":{"latitude":43.90396671664,"cloud_level":1,"severity":0,"polygon":[[43.8701730658909,10.1703257316642],[43.8620985377482,10.2600124940266],[43.9376950156217,10.2600124940266],[43.9457703594196,10.1703257316642]],"special_effect_level":0,"longitude":10.2151754374476,"id":1357167884094144512,"rain_level":0,"fog_level":0,"wind_direction":62,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":3,"wind_level":0}},{"type":"weather","message":{"latitude":45.3977679538283,"cloud_level":1,"severity":0,"polygon":[[45.3563321662796,11.9147053956319],[45.4293499926637,11.9455630525467],[45.4392542159797,11.8515426867682],[45.3661863570488,11.8209138798751]],"special_effect_level":0,"longitude":11.8831697521146,"id":5151795816804909056,"rain_level":0,"fog_level":0,"wind_direction":211,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":3,"wind_level":0}},{"type":"weather","message":{"latitude":44.052853289772,"cloud_level":1,"severity":0,"polygon":[[44.0204006271098,12.4380580857776],[44.0102504613752,12.5298028236952],[44.0852406104109,12.5298028236952],[44.0953916549739,12.4380580857776]],"special_effect_level":0,"longitude":12.4839385860058,"id":1381691391440060416,"rain_level":0,"fog_level":0,"wind_direction":51,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":3,"wind_level":0}},{"type":"weather","message":{"latitude":45.2321533236805,"cloud_level":1,"severity":0,"polygon":[[45.1908325056109,12.0402798394855],[45.26354365757,12.0713393526993],[45.2735247667916,11.9776172992633],[45.2007632623612,11.9467847089432]],"special_effect_level":0,"longitude":12.0089937897736,"id":5151813408990953472,"rain_level":0,"fog_level":0,"wind_direction":211,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":3,"wind_level":0}},{"type":"weather","message":{"latitude":45.5660677411195,"cloud_level":3,"severity":0,"polygon":[[45.5252903088402,9.29690351090113],[45.5994404022873,9.32121735603331],[45.6068923654095,9.22994041447834],[45.5327041573623,9.20585635339776]],"special_effect_level":0,"longitude":9.26346965723377,"id":5154010233223249920,"rain_level":0,"fog_level":0,"wind_direction":225,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":4,"wind_level":0}},{"type":"weather","message":{"latitude":43.7345841674188,"cloud_level":2,"severity":0,"polygon":[[43.7011849650405,11.1617356477747],[43.6922144976459,11.2523792165521],[43.7679178210492,11.2523792165521],[43.7768893362968,11.1617356477747]],"special_effect_level":0,"longitude":11.2070645352827,"id":1381005296184328192,"rain_level":0,"fog_level":0,"wind_direction":299,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":4,"wind_level":0}},{"type":"weather","message":{"latitude":1.02017884530479,"cloud_level":1,"severity":0,"polygon":[[0.981922574896951,0.982066803751583],[0.981899215635976,1.05861348561316],[1.05843284065938,1.05861348561316],[1.05845801984726,0.982066803751583]],"special_effect_level":0,"longitude":1.0203406000252,"id":1153278845885874176,"rain_level":0,"fog_level":0,"wind_direction":0,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":3,"wind_level":0}},{"type":"weather","message":{"latitude":45.6022047451643,"cloud_level":2,"severity":0,"polygon":[[45.5615392805802,8.84267106482423],[45.6358758710673,8.86583611528516],[45.6429168285314,8.77506414600088],[45.5685441577976,8.75212879311064]],"special_effect_level":0,"longitude":8.80891562727359,"id":5153959655688372224,"rain_level":0,"fog_level":0,"wind_direction":201,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":4,"wind_level":0}},{"type":"weather","message":{"latitude":39.2509115861077,"cloud_level":0,"severity":0,"polygon":[[39.213156630097,9.10134979604822],[39.2061734298067,9.18990521297463],[39.2885998079402,9.18990521297463],[39.2955870966989,9.10134979604822]],"special_effect_level":0,"longitude":9.14563301321285,"id":1362115686419136512,"rain_level":0,"fog_level":0,"wind_direction":151,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":1,"wind_level":0}},{"type":"weather","message":{"latitude":45.6091869719173,"cloud_level":2,"severity":0,"polygon":[[45.5685441577976,8.75212879311064],[45.6429168285314,8.77506414600088],[45.6498762905267,8.68439510347529],[45.5754679441912,8.66168941670029]],"special_effect_level":0,"longitude":8.71831003359811,"id":5153913476200005632,"rain_level":0,"fog_level":0,"wind_direction":201,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":4,"wind_level":0}},{"type":"weather","message":{"latitude":44.0426601108955,"cloud_level":1,"severity":0,"polygon":[[44.0102504613752,12.5298028236952],[44.0000151614512,12.6216210646625],[44.0750044147535,12.6216210646625],[44.0852406104109,12.5298028236952]],"special_effect_level":0,"longitude":12.5757201503191,"id":1381693590463315968,"rain_level":0,"fog_level":0,"wind_direction":51,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":3,"wind_level":0}}]"#
        ];
        let mut weathers = Vec::new();
        for s in &strings {
            let temps = serde_json::from_str::<Vec<Request>>(s).unwrap();
            for temp in temps {
                if let Request::Weather(weather) = temp {
                    weathers.push(weather);
                }
            }
        }
        for weather1 in &weathers {
            for weather2 in &weathers {
                if weather1.id != weather2.id {
                    assert!(weather1 != weather2);
                }
            }
        }
    }

    #[test]
    fn weather_change() {
        let weather1: Weather = serde_json::from_str(r#"{"latitude":44.0426601108955,"cloud_level":1,"severity":0,"polygon":[[44.0102504613752,12.5298028236952],[44.0000151614512,12.6216210646625],[44.0750044147535,12.6216210646625],[44.0852406104109,12.5298028236952]],"special_effect_level":0,"longitude":12.5757201503191,"id":1381693590463315968,"rain_level":0,"fog_level":0,"wind_direction":51,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":3,"wind_level":0}"#).unwrap();
        let weather2: Weather = serde_json::from_str(r#"{"latitude":45.6091869719173,"cloud_level":2,"severity":0,"polygon":[[45.5685441577976,8.75212879311064],[45.6429168285314,8.77506414600088],[45.6498762905267,8.68439510347529],[45.5754679441912,8.66168941670029]],"special_effect_level":0,"longitude":8.71831003359811,"id":5153913476200005632,"rain_level":0,"fog_level":0,"wind_direction":201,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":4,"wind_level":0}"#).unwrap();
        assert!(weather1 != weather2);
        let weather2: Weather = serde_json::from_str(r#"{"latitude":44.0426601108955,"cloud_level":2,"severity":0,"polygon":[[44.0102504613752,12.5298028236952],[44.0000151614512,12.6216210646625],[44.0750044147535,12.6216210646625],[44.0852406104109,12.5298028236952]],"special_effect_level":0,"longitude":12.5757201503191,"id":5153913476200005633,"rain_level":0,"fog_level":0,"wind_direction":201,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":4,"wind_level":0}"#).unwrap();
        assert!(weather1 != weather2);
        let weather2: Weather = serde_json::from_str(r#"{"latitude":45.6091869719173,"cloud_level":1,"severity":0,"polygon":[[44.0102504613752,12.5298028236952],[44.0000151614512,12.6216210646625],[44.0750044147535,12.6216210646625],[44.0852406104109,12.5298028236952]],"special_effect_level":0,"longitude":8.71831003359811,"id":1381693590463315968,"rain_level":0,"fog_level":0,"wind_direction":51,"snow_level":0,"warn_weather":false,"updated":1,"gameplay_condition":3,"wind_level":0}"#).unwrap();
        assert!(weather1 == weather2);
    }
}

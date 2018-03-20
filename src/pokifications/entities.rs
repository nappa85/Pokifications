extern crate serde;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "type", content = "message")]
pub enum Request {
    #[serde(rename = "pokemon")]
    Pokemon(Pokemon),
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pokemon {
    spawnpoint_id: usize,
    encounter_id: usize,
    pokemon_id: usize,
    latitude: f64,
    longitude: f64,
    disappear_time: usize,
    time_until_hidden_ms: i64,
    last_modified_time: usize,
    seconds_until_despawn: usize,
    spawn_start: usize,
    spawn_end: usize,
    gender: Gender,
    #[serde(default)]
    cp: Option<usize>,
    #[serde(default)]
    form: Option<String>,
    #[serde(default)]
    costume: Option<usize>,
    #[serde(default)]
    individual_attack: Option<usize>,
    #[serde(default)]
    individual_defense: Option<usize>,
    #[serde(default)]
    individual_stamina: Option<usize>,
    #[serde(default)]
    cp_multiplier: Option<f64>,
    #[serde(default)]
    move_1: Option<usize>,
    #[serde(default)]
    move_2: Option<usize>,
    #[serde(default)]
    weight: Option<f64>,
    #[serde(default)]
    height: Option<f64>,
    #[serde(default)]
    base_catch: Option<f64>,
    #[serde(default)]
    great_catch: Option<f64>,
    #[serde(default)]
    ultra_catch: Option<f64>,
    #[serde(default)]
    boosted_weather: Option<usize>,
    #[serde(default)]
    def_grade: Option<String>,
    #[serde(default)]
    atk_grade: Option<String>,
    #[serde(default)]
    rating_attack: Option<String>,
    #[serde(default)]
    rating_defense: Option<String>,
    #[serde(default)]
    catch_prob_1: Option<f64>,
    #[serde(default)]
    catch_prob_2: Option<f64>,
    #[serde(default)]
    catch_prob_3: Option<f64>,
    #[serde(default)]
    weather: Option<usize>,
    #[serde(default)]
    weather_boosted_condition: Option<usize>,
    #[serde(default)]
    pokemon_level: Option<usize>,
    #[serde(default)]
    s2_cell_id: Option<usize>,
    player_level: usize,
    verified: bool,
}

#[derive(Clone, Debug)]
pub enum Gender {
    Unset,
    Male,
    Female,
    Genderless, 
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

impl ::serde::Serialize for Gender {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: ::serde::Serializer
    {
        serializer.serialize_u32(match *self {
            Gender::Unset => 0,
            Gender::Male => 1,
            Gender::Female => 2,
            Gender::Genderless => 3,
        })
    }
}

impl<'de> ::serde::Deserialize<'de> for Gender {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: ::serde::Deserializer<'de>
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pokestop {
    pokestop_id: String,
    latitude: f64,
    longitude: f64,
    enabled: bool,
    last_modified: usize,
    #[serde(default)]
    active_fort_modifier: Option<bool>,
    #[serde(default)]
    lure_expiration: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gym {
    gym_id: String,
    latitude: f64,
    longitude: f64,
    enabled: bool,
    team_id: Team,
    occupied_since: usize,
    last_modified: usize,
    guard_pokemon_id: usize,
    total_cp: usize,
    slots_available: usize,
    lowest_pokemon_motivation: f64,
    raid_active_until: usize,
}

#[derive(Clone, Debug)]
pub enum Team {
    Uncontested,
    Mystic,
    Valor,
    Instinct,
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

impl ::serde::Serialize for Team {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: ::serde::Serializer
    {
        serializer.serialize_u32(match *self {
            Team::Uncontested => 0,
            Team::Mystic => 1,
            Team::Valor => 2,
            Team::Instinct => 3,
        })
    }
}

impl<'de> ::serde::Deserialize<'de> for Team {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: ::serde::Deserializer<'de>
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GymDetails {
    id: String,
    name: String,
    description: String,
    url: String,
    latitude: f64,
    longitude: f64,
    team: Team,
    pokemon: Vec<GymPokemon>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GymPokemon {
    pokemon_uid: usize,
    pokemon_id: usize,
    cp: usize,
    cp_decayed: usize,
    stamina_max: usize,
    stamina: usize,
    move_1: usize,
    move_2: usize,
    height: f64,
    weight: f64,
    form: usize,
    costume: usize,
    iv_attack: usize,
    iv_defense: usize,
    iv_stamina: usize,
    cp_multiplier: f64,
    additional_cp_multiplier: f64,
    num_upgrades: usize,
    deployment_time: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Raid {
    gym_id: String,
    team_id: Team,
    latitude: f64,
    longitude: f64,
    spawn: usize,
    start: usize,
    end: usize,
    level: usize,
    #[serde(default)]
    pokemon_id: Option<usize>,
    #[serde(default)]
    cp: Option<usize>,
    #[serde(default)]
    move_1: Option<usize>,
    #[serde(default)]
    move_2: Option<usize>,
    #[serde(default)]
    weather: Option<usize>,
    #[serde(default)]
    s2_cell_id: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scheduler {
    name: String,
    instance: String,
    tth_found: f64,
    spawns_found: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Captcha {
    status_name: String,
    account: String,
    status: CaptchaStatus,
    captcha: usize,
    time: usize,
    mode: String,
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

impl ::serde::Serialize for CaptchaStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: ::serde::Serializer
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> ::serde::Deserialize<'de> for CaptchaStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: ::serde::Deserializer<'de>
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {

}

impl Config {
    pub fn matches(&self, _r: Request) {

    }
}

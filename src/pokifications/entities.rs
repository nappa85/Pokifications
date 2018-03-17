use std::fmt;

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct Pokemon {
    spawnpoint_id: String,
    encounter_id: String,
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
    cp: Option<String>,
    #[serde(default)]
    form: Option<String>,
    #[serde(default)]
    costume: Option<String>,
    #[serde(default)]
    individual_attack: Option<String>,
    #[serde(default)]
    individual_defense: Option<String>,
    #[serde(default)]
    individual_stamina: Option<String>,
    #[serde(default)]
    cp_multiplier: Option<String>,
    #[serde(default)]
    move_1: Option<String>,
    #[serde(default)]
    move_2: Option<String>,
    #[serde(default)]
    weight: Option<String>,
    #[serde(default)]
    height: Option<String>,
    player_level: usize,
    verified: bool,
}

#[derive(Debug)]
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
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> ::serde::Deserialize<'de> for Gender {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: ::serde::Deserializer<'de>
    {
        struct Visitor;

        impl<'de> ::serde::de::Visitor<'de> for Visitor {
            type Value = Gender;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Gender, E>
                where E: ::serde::de::Error
            {
                // Rust does not come with a simple way of converting a
                // number to an enum, so use a big `match`.
                match value {
                    "unset" => Ok(Gender::Unset),
                    "male" => Ok(Gender::Male),
                    "female" => Ok(Gender::Female),
                    "genderless" => Ok(Gender::Genderless),
                    _ => Err(E::custom(format!("unknown Gender value: {}", value))),
                }
            }
        }

        // Deserialize the enum from a u64.
        deserializer.deserialize_str(Visitor)
    }
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug)]
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
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> ::serde::Deserialize<'de> for Team {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: ::serde::Deserializer<'de>
    {
        struct Visitor;

        impl<'de> ::serde::de::Visitor<'de> for Visitor {
            type Value = Team;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Team, E>
                where E: ::serde::de::Error
            {
                // Rust does not come with a simple way of converting a
                // number to an enum, so use a big `match`.
                match value {
                    "uncontested" => Ok(Team::Uncontested),
                    "mystic" => Ok(Team::Mystic),
                    "valor" => Ok(Team::Valor),
                    "instinct" => Ok(Team::Instinct),
                    _ => Err(E::custom(format!("unknown Team value: {}", value))),
                }
            }
        }

        // Deserialize the enum from a u64.
        deserializer.deserialize_str(Visitor)
    }
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Scheduler {
    name: String,
    instance: String,
    tth_found: f64,
    spawns_found: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Captcha {
    status_name: String,
    account: String,
    status: CaptchaStatus,
    captcha: usize,
    time: usize,
    mode: String,
}

#[derive(Debug)]
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
        struct Visitor;

        impl<'de> ::serde::de::Visitor<'de> for Visitor {
            type Value = CaptchaStatus;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("string")
            }

            fn visit_str<E>(self, value: &str) -> Result<CaptchaStatus, E>
                where E: ::serde::de::Error
            {
                // Rust does not come with a simple way of converting a
                // number to an enum, so use a big `match`.
                match value {
                    "encounter" => Ok(CaptchaStatus::Encounter),
                    "success" => Ok(CaptchaStatus::Success),
                    "failure" => Ok(CaptchaStatus::Failure),
                    "error" => Ok(CaptchaStatus::Error),
                    _ => Err(E::custom(format!("unknown CaptchaStatus value: {}", value))),
                }
            }
        }

        // Deserialize the enum from a u64.
        deserializer.deserialize_str(Visitor)
    }
}

use std::collections::HashMap;

use futures_util::future::join_all;

use geo::{Point, Polygon};

use mysql_async::{prelude::Queryable, Row};

use tokio::{spawn, time::{Duration, Instant, interval_at}, sync::RwLock};

use once_cell::sync::Lazy;

use log::error;

use crate::db::MYSQL;

pub static LIST: Lazy<RwLock<HashMap<u16, Pokemon>>> = Lazy::new(|| RwLock::new(HashMap::new()));
// pub static SCANNED: Lazy<Vec<u16>> = Lazy::new(|| LIST.iter().filter(|(_, p)| p.scanned == 1).map(|(id, _)| *id).collect());
// pub static COMMON: Lazy<Vec<u16>> = Lazy::new(|| LIST.iter().filter(|(_, p)| p.rarity < 2).map(|(id, _)| *id).collect());
// pub static RARES: Lazy<Vec<u16>> = Lazy::new(|| LIST.iter().filter(|(_, p)| p.rarity >= 2).map(|(id, _)| *id).collect());

pub static MOVES: Lazy<RwLock<HashMap<u16, String>>> = Lazy::new(|| RwLock::new(HashMap::new()));

pub static FORMS: Lazy<RwLock<HashMap<u16, Form>>> = Lazy::new(|| RwLock::new(HashMap::new()));

pub static GRUNTS: Lazy<RwLock<HashMap<u8, GruntType>>> = Lazy::new(|| RwLock::new(HashMap::new()));

pub static CITIES: Lazy<RwLock<HashMap<u16, City>>> = Lazy::new(|| RwLock::new(HashMap::new()));

pub static CITYSTATS: Lazy<RwLock<HashMap<u16, CityStats>>> = Lazy::new(|| RwLock::new(HashMap::new()));

pub static CITYPARKS: Lazy<RwLock<HashMap<u16, Vec<CityPark>>>> = Lazy::new(|| RwLock::new(HashMap::new()));

pub struct Pokemon {
    pub id: u16,
    pub name: String,
    pub ptype: Option<String>,
    pub rarity: u8,
    pub scanned: u8,
    pub status: u8,
    pub raid: u8,
}

pub struct Form {
    pub id: u16,
    pub name: String,
    pub pokemon_id: Option<u16>,
    pub hidden: bool,
}

pub struct GruntType {
    pub id: u8,
    pub name: String,
    pub sex: Option<String>,
    pub element: Option<String>,
}

pub struct City {
    pub id: u16,
    pub name: String,
    pub coordinates: Polygon<f64>,
    pub scadenza: i64,
    pub scan_iv: u8,
    pub admins_users: Vec<String>,
}

impl From<Row> for City {
    fn from(mut row: Row) -> Self {
        let id = row.take("id").expect("MySQL city.id error");
        let name = row.take("name").expect("MySQL city.name error");
        let coords = row.take::<String, _>("coordinates").expect("MySQL city.coordinates encoding error");
        let coords = coords.replace(char::is_whitespace, "");

        let poly: Vec<Point<f64>> = if coords.len() < 2 {
            error!("City \"{}\" ({}) has empty coordinates", name, id);
            Vec::new()
        }
        else {
            (&coords[1..(coords.len() - 1)]).split("),(")
                .map(|s| {
                    let x_y: Vec<f64> = s.split(',')
                        .map(|s| match s.parse::<f64>() {
                            Ok(f) => f,
                            Err(_) => panic!("Error parsing \"{}\" as a float", s),
                        })
                        .collect();
                    if x_y.len() == 2 {
                        Some(Point::new(x_y[0], x_y[1]))
                    }
                    else {
                        error!("City \"{}\" ({}) has invalid coordinates", name, id);
                        None
                    }
                })
                .filter(Option::is_some)
                .map(Option::unwrap)
                .collect()
        };

        City {
            id,
            name,
            coordinates: Polygon::new(poly.into(), vec![]),
            scadenza: row.take("scadenza").expect("MySQL city.scadenza error"),
            scan_iv: row.take("monitor").expect("MySQL city.monitor error"),
            admins_users: row.take::<String, _>("admins_users")
                .expect("MySQL city.admins_users error")
                .split_whitespace()
                .map(|s| s.to_owned())
                .collect(),
        }
    }
}

#[derive(Default)]
pub struct CityStats {
    pub last_raid: Option<i64>,
    pub last_pokemon: Option<i64>,
    pub last_iv: Option<i64>,
    pub last_quest: Option<i64>,
    pub last_invasion: Option<i64>,
}

pub struct CityPark {
    pub id: u64,
    pub city_id: u16,
    pub coordinates: Polygon<f64>,
}

async fn load_pokemons() -> Result<(), ()> {
    let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn.query_iter("SELECT * FROM pokemon_list").await.map_err(|e| error!("MySQL query error: get pokemon list\n{}", e))?;

    let mut list = LIST.write().await;
    list.clear();
    res.for_each_and_drop(|ref mut row| {
        let id = row.take("id").expect("MySQL pokemon_list.id error");
        list.insert(id, Pokemon {
            id,
            name: row.take("name").expect("MySQL pokemon_list.name error"),
            ptype: row.take("type").expect("MySQL pokemon_list.type error"),
            rarity: row.take("rarity").expect("MySQL pokemon_list.rarity error"),
            scanned: row.take("scanned").expect("MySQL pokemon_list.scanned error"),
            status: row.take("status").expect("MySQL pokemon_list.status error"),
            raid: row.take("raid").expect("MySQL pokemon_list.raid error"),
        });
    }).await.map_err(|e| error!("MySQL for_each error: {}", e))?;

    Ok(())
}

async fn load_moves() -> Result<(), ()> {
    let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn.query_iter("SELECT * FROM pokemon_moves").await.map_err(|e| error!("MySQL query error: get pokemon moves\n{}", e))?;

    let mut moves = MOVES.write().await;
    moves.clear();
    res.for_each_and_drop(|ref mut row| {
        moves.insert(row.take("id").expect("MySQL pokemon_moves.id error"), row.take("move").expect("MySQL pokemon_moves.move error"));
    }).await.map_err(|e| error!("MySQL for_each error: {}", e))?;

    Ok(())
}

async fn load_forms() -> Result<(), ()> {
    let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn.query_iter("SELECT * FROM pokemon_forms").await.map_err(|e| error!("MySQL query error: get pokemon forms\n{}", e))?;

    let mut forms = FORMS.write().await;
    forms.clear();
    res.for_each_and_drop(|ref mut row| {
        let id = row.take("id").expect("MySQL pokemon_forms.id error");
        forms.insert(id, Form {
            id,
            name: row.take("name").expect("MySQL pokemon_forms.name error"),
            pokemon_id: row.take("pokemon_id").expect("MySQL pokemon_forms.pokemon_id error"),
            hidden: row.take("hidden").expect("MySQL pokemon_forms.hidden error"),
        });
    }).await.map_err(|e| error!("MySQL for_each error: {}", e))?;

    Ok(())
}

async fn load_grunts() -> Result<(), ()> {
    let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn.query_iter("SELECT * FROM grunt_types").await.map_err(|e| error!("MySQL query error: get grunt types\n{}", e))?;

    let mut grunts = GRUNTS.write().await;
    grunts.clear();
    res.for_each_and_drop(|ref mut row| {
        let id = row.take("id").expect("MySQL grunt_types.id error");
        grunts.insert(id, GruntType {
            id,
            name: row.take("name").expect("MySQL grunt_types.name error"),
            sex: row.take("sex").expect("MySQL grunt_types.type error"),
            element: row.take("type").expect("MySQL grunt_types.rarity error"),
        });
    }).await.map_err(|e| error!("MySQL for_each error: {}", e))?;

    Ok(())
}

async fn load_cities() -> Result<(), ()> {
    let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn.query_iter("SELECT id, name, coordinates, scadenza, monitor, admins_users FROM city").await.map_err(|e| error!("MySQL query error: get cities\n{}", e))?;

    let mut cities = CITIES.write().await;
    cities.clear();
    res.for_each_and_drop(|row| {
        let city: City = row.into();
        cities.insert(city.id, city);
    }).await.map_err(|e| error!("MySQL for_each error: {}", e))?;

    Ok(())
}

async fn load_parks() -> Result<(), ()> {
    let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn.query_iter("SELECT id, city_id, coordinates FROM city_parks").await.map_err(|e| error!("MySQL query error: get city parks\n{}", e))?;

    let mut parks = CITYPARKS.write().await;
    parks.clear();
    res.for_each_and_drop(|ref mut row| {
        let id = row.take("id").expect("MySQL city_parks.id error");
        let city_id = row.take("city_id").expect("MySQL city_parks.city_id error");
        let coords = row.take::<String, _>("coordinates").expect("MySQL city_parks.coordinates encoding error");
        let coords = coords.replace(char::is_whitespace, "");

        let poly: Vec<Point<f64>> = if coords.len() < 2 {
            error!("Park {} has empty coordinates", id);
            Vec::new()
        }
        else {
            (&coords[1..(coords.len() - 1)]).split("),(")
                .map(|s| {
                    let x_y: Vec<f64> = s.split(',')
                        .map(|s| s.parse::<f64>().map_err(|e| error!("Error parsing \"{}\" as a float: {}", s, e)).ok())
                        .filter(Option::is_some)
                        .map(Option::unwrap)
                        .collect();
                    if x_y.len() == 2 {
                        Some(Point::new(x_y[0], x_y[1]))
                    }
                    else {
                        error!("Park {} has invalid coordinates", id);
                        None
                    }
                })
                .filter(Option::is_some)
                .map(Option::unwrap)
                .collect()
        };

        let cityparks = parks.entry(city_id).or_insert_with(Vec::new);
        cityparks.push(CityPark {
            id,
            city_id,
            coordinates: Polygon::new(poly.into(), vec![]),
        });
    }).await.map_err(|e| error!("MySQL for_each error: {}", e))?;

    Ok(())
}

async fn load() {
    join_all((0_u8..6_u8).map(|i| async move {
        match i {
            0 => load_pokemons().await,
            1 => load_moves().await,
            2 => load_forms().await,
            3 => load_grunts().await,
            4 => load_cities().await,
            5 => load_parks().await,
            _ => panic!("WTF"),
        }
    })).await;
}

pub async fn init() {
    // force first load
    load().await;
    spawn(async {
        let period = Duration::from_secs(1800);
        let mut interval = interval_at(Instant::now() + period, period);
        loop {
            interval.tick().await;
            load().await;
        }
    });
}

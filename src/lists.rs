use std::time::Duration;
use std::collections::HashMap;
use std::sync::Arc;

use futures_util::{future::join_all, stream::StreamExt};

use geo::{Point, Polygon};

use mysql_async::prelude::Queryable;

use async_std::sync::RwLock;

use tokio::{spawn, time::interval};

use once_cell::sync::Lazy;

use log::error;

use crate::db::MYSQL;

pub static LIST: Lazy<Arc<RwLock<HashMap<u16, Pokemon>>>> = Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));
// pub static SCANNED: Lazy<Vec<u16>> = Lazy::new(|| LIST.iter().filter(|(_, p)| p.scanned == 1).map(|(id, _)| *id).collect());
// pub static COMMON: Lazy<Vec<u16>> = Lazy::new(|| LIST.iter().filter(|(_, p)| p.rarity < 2).map(|(id, _)| *id).collect());
// pub static RARES: Lazy<Vec<u16>> = Lazy::new(|| LIST.iter().filter(|(_, p)| p.rarity >= 2).map(|(id, _)| *id).collect());

pub static MOVES: Lazy<Arc<RwLock<HashMap<u16, String>>>> = Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

pub static FORMS: Lazy<Arc<RwLock<HashMap<u16, String>>>> = Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

pub static GRUNTS: Lazy<Arc<RwLock<HashMap<u8, GruntType>>>> = Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

pub static CITIES: Lazy<Arc<RwLock<HashMap<u16, City>>>> = Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

pub static CITYSTATS: Lazy<Arc<RwLock<HashMap<u16, CityStats>>>> = Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

pub static CITYPARKS: Lazy<Arc<RwLock<HashMap<u16, Vec<CityPark>>>>> = Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

pub struct Pokemon {
    pub id: u16,
    pub name: String,
    pub ptype: Option<String>,
    pub rarity: u8,
    pub scanned: u8,
    pub status: u8,
    pub raid: u8,
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
    pub admins_users: String,
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
    pub name: String,
    pub coordinates: Polygon<f64>,
}

async fn load_pokemons() -> Result<(), ()> {
    let conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn.query("SELECT * FROM pokemon_list").await.map_err(|e| error!("MySQL query error: {}", e))?;

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
    let conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn.query("SELECT * FROM pokemon_moves").await.map_err(|e| error!("MySQL query error: {}", e))?;

    let mut moves = MOVES.write().await;
    moves.clear();
    res.for_each_and_drop(|ref mut row| {
        moves.insert(row.take("id").expect("MySQL pokemon_moves.id error"), row.take("move").expect("MySQL pokemon_moves.move error"));
    }).await.map_err(|e| error!("MySQL for_each error: {}", e))?;

    Ok(())
}

async fn load_forms() -> Result<(), ()> {
    let conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn.query("SELECT * FROM pokemon_forms").await.map_err(|e| error!("MySQL query error: {}", e))?;

    let mut forms = FORMS.write().await;
    forms.clear();
    res.for_each_and_drop(|ref mut row| {
        forms.insert(row.take("id").expect("MySQL pokemon_forms.id error"), row.take("name").expect("MySQL pokemon_forms.name error"));
    }).await.map_err(|e| error!("MySQL for_each error: {}", e))?;

    Ok(())
}

async fn load_grunts() -> Result<(), ()> {
    let conn =MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn.query("SELECT * FROM grunt_types").await.map_err(|e| error!("MySQL query error: {}", e))?;

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
    let conn =MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn.query("SELECT id, name, coordinates, scadenza, monitor, admins_users FROM city WHERE scadenza > UNIX_TIMESTAMP()").await.map_err(|e| error!("MySQL query error: {}", e))?;

    let mut cities = CITIES.write().await;
    cities.clear();
    res.for_each_and_drop(|ref mut row| {
        let id = row.take("id").expect("MySQL city.id error");
        let name = row.take("name").expect("MySQL city.name error");
        let coords = row.take::<String, _>("coordinates").expect("MySQL city.coordinates encoding error");
        let coords = coords.replace(char::is_whitespace, "");

        let poly: Vec<Point<f64>> = if coords.is_empty() {
            Vec::new()
        }
        else {
            (&coords[1..(coords.len() - 2)]).split("),(")
                .map(|s| {
                    let x_y: Vec<f64> = s.split(",")
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

        cities.insert(id, City {
            id,
            name,
            coordinates: Polygon::new(poly.into(), vec![]),
            scadenza: row.take("scadenza").expect("MySQL city.scadenza error"),
            scan_iv: row.take("monitor").expect("MySQL city.monitor error"),
            admins_users: row.take("admins_users").expect("MySQL city.admins_users error"),
        });
    }).await.map_err(|e| error!("MySQL for_each error: {}", e))?;

    Ok(())
}

async fn load_parks() -> Result<(), ()> {
    let conn =MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn.query("SELECT id, city_id, name, coordinates FROM city_parks").await.map_err(|e| error!("MySQL query error: {}", e))?;

    let mut parks = CITYPARKS.write().await;
    parks.clear();
    res.for_each_and_drop(|ref mut row| {
        let id = row.take("id").expect("MySQL city_parks.id error");
        let city_id = row.take("city_id").expect("MySQL city_parks.city_id error");
        let name = row.take("name").expect("MySQL city_parks.name error");
        let coords = row.take::<String, _>("coordinates").expect("MySQL city_parks.coordinates encoding error");
        let coords = coords.replace(char::is_whitespace, "");

        let poly: Vec<Point<f64>> = if coords.is_empty() {
            Vec::new()
        }
        else {
            (&coords[1..(coords.len() - 2)]).split("),(")
                .map(|s| {
                    let x_y: Vec<f64> = s.split(",")
                        .map(|s| match s.parse::<f64>() {
                            Ok(f) => f,
                            Err(_) => panic!("Error parsing \"{}\" as a float", s),
                        })
                        .collect();
                    if x_y.len() == 2 {
                        Some(Point::new(x_y[0], x_y[1]))
                    }
                    else {
                        error!("Park \"{}\" ({}) has invalid coordinates", name, id);
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
            name,
            coordinates: Polygon::new(poly.into(), vec![]),
        });
    }).await.map_err(|e| error!("MySQL for_each error: {}", e))?;

    Ok(())
}

pub fn init() {
    spawn(async {
        interval(Duration::from_secs(1800))
            .for_each(|_| async {
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
            }).await;
    });
}

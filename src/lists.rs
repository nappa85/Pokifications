use std::collections::HashMap;
use std::sync::Arc;

use geo::Polygon;

use mysql_async::prelude::Queryable;

use future_parking_lot::rwlock::{RwLock, write::FutureWriteable};

use log::error;

use crate::db::MYSQL;

pub static LIST: Arc<RwLock<HashMap<u16, Pokemon>>> = Arc::new(RwLock::new(HashMap::new()));
// pub static SCANNED: Lazy<Vec<u16>> = Lazy::new(|| LIST.iter().filter(|(_, p)| p.scanned == 1).map(|(id, _)| *id).collect());
// pub static COMMON: Lazy<Vec<u16>> = Lazy::new(|| LIST.iter().filter(|(_, p)| p.rarity < 2).map(|(id, _)| *id).collect());
// pub static RARES: Lazy<Vec<u16>> = Lazy::new(|| LIST.iter().filter(|(_, p)| p.rarity >= 2).map(|(id, _)| *id).collect());

pub static MOVES: Arc<RwLock<HashMap<u16, String>>> = Arc::new(RwLock::new(HashMap::new()));

pub static FORMS: Arc<RwLock<HashMap<u16, String>>> = Arc::new(RwLock::new(HashMap::new()));

pub static GRUNTS: Arc<RwLock<HashMap<u8, GruntType>>> = Arc::new(RwLock::new(HashMap::new()));

pub static CITIES: Arc<RwLock<HashMap<u16, City>>> = Arc::new(RwLock::new(HashMap::new()));

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
    pub stats: Arc<RwLock<CityStats>>,
}

#[derive(Default)]
pub struct CityStats {
    pub last_raid: Option<i64>,
    pub last_pokemon: Option<i64>,
    pub last_iv: Option<i64>,
    pub last_quest: Option<i64>,
    pub last_invasion: Option<i64>,
}

async fn load_pokemons() -> Result<(), ()> {
    let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn.query("SELECT * FROM pokemon_list").await.map_err(|e| error!("MySQL query error: {}", e))?;

    let list = LIST.future_write().await;
    list.clear();
    res.for_each_and_drop(|row| {
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
    let res = conn.query("SELECT * FROM pokemon_moves").await.map_err(|e| error!("MySQL query error: {}", e))?;

    let moves = MOVES.future_write().await;
    moves.clear();
    res.for_each_and_drop(|row| {
        moves.insert(row.take("id").expect("MySQL pokemon_moves.id error"), row.take("move").expect("MySQL pokemon_moves.move error"));
    }).await.map_err(|e| error!("MySQL for_each error: {}", e))?;

    Ok(())
}

async fn load_forms() -> Result<(), ()> {
    let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn.query("SELECT * FROM pokemon_forms").await.map_err(|e| error!("MySQL query error: {}", e))?;

    let forms = FORMS.future_write().await;
    forms.clear();
    res.for_each_and_drop(|row| {
        forms.insert(row.take("id").expect("MySQL pokemon_forms.id error"), row.take("name").expect("MySQL pokemon_forms.name error"));
    }).await.map_err(|e| error!("MySQL for_each error: {}", e))?;

    Ok(())
}

async fn load_grunts() -> Result<(), ()> {
    let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn.query("SELECT * FROM grunt_types").await.map_err(|e| error!("MySQL query error: {}", e))?;

    let grunts = GRUNTS.future_write().await;
    grunts.clear();
    res.for_each_and_drop(|row| {
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
    let res = conn.query("SELECT id, name, coordinates, scadenza, monitor, admins_users FROM city WHERE scadenza > UNIX_TIMESTAMP()").await.map_err(|e| error!("MySQL query error: {}", e))?;

    let cities = CITIES.future_write().await;
    cities.clear();
    res.for_each_and_drop(|row| {
        let id = row.take("id").expect("MySQL city.id error");
        let coords: String = row.take("coordinates").expect("MySQL city.coordinates encoding error");
        let mut poly: Vec<[f64; 2]> = Vec::new();
        for (i, c) in coords.replace("(", "").replace(")", "").split(",").enumerate() {
            let f: f64 = c.trim().parse().expect("Coordinate parse error");
            if i % 2 == 0 {
                poly.push([f, 0_f64]);
            }
            else {
                let len = poly.len();
                poly[len - 1][1] = f;
            }
        }

        cities.insert(id, City {
            id,
            name: row.take("name").expect("MySQL city.name error"),
            coordinates: Polygon::new(poly.into(), vec![]),
            scadenza: row.take("scadenza").expect("MySQL city.scadenza error"),
            scan_iv: row.take("monitor").expect("MySQL city.monitor error"),
            admins_users: row.take("admins_users").expect("MySQL city.admins_users error"),
            stats: Arc::new(RwLock::new(CityStats::default())),
        });
    }).await.map_err(|e| error!("MySQL for_each error: {}", e))?;

    Ok(())
}

async fn init() {
    //TODO: paralellize
    load_pokemons().await.unwrap();
    load_moves().await.unwrap();
    load_forms().await.unwrap();
    load_grunts().await.unwrap();
    load_cities().await.unwrap();
}

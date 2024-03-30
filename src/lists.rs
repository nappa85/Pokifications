use std::{collections::HashMap, future, sync::Arc};

use arc_swap::ArcSwap;

use futures_util::{future::join_all, TryStreamExt};

use geo::{Point, Polygon};

use mysql_async::{
    prelude::{FromRow, Queryable},
    Row,
};

use rocketmap_entities::gamemaster::Cache;

use tokio::{
    spawn,
    sync::RwLock,
    time::{interval_at, Duration, Instant},
};

use once_cell::sync::Lazy;

use tracing::error;

use crate::db::MYSQL;

pub static LIST: Lazy<ArcSwap<HashMap<u16, Pokemon>>> = Lazy::new(Default::default);
// pub static SCANNED: Lazy<Vec<u16>> = Lazy::new(|| LIST.iter().filter(|(_, p)| p.scanned == 1).map(|(id, _)| *id).collect());
// pub static COMMON: Lazy<Vec<u16>> = Lazy::new(|| LIST.iter().filter(|(_, p)| p.rarity < 2).map(|(id, _)| *id).collect());
// pub static RARES: Lazy<Vec<u16>> = Lazy::new(|| LIST.iter().filter(|(_, p)| p.rarity >= 2).map(|(id, _)| *id).collect());

pub static MOVES: Lazy<ArcSwap<HashMap<u16, String>>> = Lazy::new(Default::default);

pub static FORMS: Lazy<ArcSwap<HashMap<u16, Form>>> = Lazy::new(Default::default);

pub static GRUNTS: Lazy<ArcSwap<HashMap<u16, GruntType>>> = Lazy::new(Default::default);

pub static CITIES: Lazy<ArcSwap<HashMap<u16, City>>> = Lazy::new(Default::default);

pub static CITYSTATS: Lazy<RwLock<HashMap<u16, CityStats>>> = Lazy::new(Default::default);

pub static CITYPARKS: Lazy<ArcSwap<HashMap<u16, Vec<CityPark>>>> = Lazy::new(Default::default);

pub struct Pokemon {
    pub id: u16,
    pub name: String,
    pub ptype: Option<String>,
    pub rarity: u8,
    pub scanned: u8,
    pub status: u8,
    pub raid: u8,
}

impl FromRow for Pokemon {
    fn from_row_opt(mut row: Row) -> Result<Self, mysql_async::FromRowError> {
        Ok(Pokemon {
            id: row.take("id").expect("MySQL pokemon_list.id error"),
            name: row.take("name").expect("MySQL pokemon_list.name error"),
            ptype: row.take("type").expect("MySQL pokemon_list.type error"),
            rarity: row.take("rarity").expect("MySQL pokemon_list.rarity error"),
            scanned: row.take("scanned").expect("MySQL pokemon_list.scanned error"),
            status: row.take("status").expect("MySQL pokemon_list.status error"),
            raid: row.take("raid").expect("MySQL pokemon_list.raid error"),
        })
    }
}

#[derive(Debug)]
pub struct PokemonCache;

impl Cache for PokemonCache {
    type Id = u16;
    fn get(id: Self::Id) -> Option<String> {
        let list = LIST.load();
        list.get(&id).map(|p| p.name.clone())
    }

    fn reverse(name: &str) -> Option<Self::Id> {
        let list = LIST.load();
        list.iter().find(|(_, p)| p.name.eq_ignore_ascii_case(name)).map(|(id, _)| *id)
    }
}

pub struct Form {
    pub id: u16,
    pub name: String,
    pub pokemon_id: Option<u16>,
    pub hidden: bool,
}

impl FromRow for Form {
    fn from_row_opt(mut row: Row) -> Result<Self, mysql_async::FromRowError> {
        Ok(Form {
            id: row.take("id").expect("MySQL pokemon_forms.id error"),
            name: row.take("name").expect("MySQL pokemon_forms.name error"),
            pokemon_id: row.take("pokemon_id").expect("MySQL pokemon_forms.pokemon_id error"),
            hidden: row.take("hidden").expect("MySQL pokemon_forms.hidden error"),
        })
    }
}

#[derive(Debug)]
pub struct FormCache;

impl Cache for FormCache {
    type Id = u16;
    fn get(id: Self::Id) -> Option<String> {
        let forms = FORMS.load();
        forms.get(&id).map(|f| f.name.clone())
    }

    fn reverse(name: &str) -> Option<Self::Id> {
        let forms = FORMS.load();
        forms.iter().find(|(_, f)| f.name == name).map(|(id, _)| *id)
    }
}

pub struct GruntType {
    pub id: u16,
    pub name: String,
    pub sex: Option<String>,
    pub element: Option<String>,
}

impl FromRow for GruntType {
    fn from_row_opt(mut row: Row) -> Result<Self, mysql_async::FromRowError> {
        Ok(GruntType {
            id: row.take("id").expect("MySQL grunt_types.id error"),
            name: row.take("name").expect("MySQL grunt_types.name error"),
            sex: row.take("sex").expect("MySQL grunt_types.type error"),
            element: row.take("type").expect("MySQL grunt_types.rarity error"),
        })
    }
}

pub struct City {
    pub id: u16,
    pub name: String,
    pub coordinates: Polygon<f64>,
    pub scadenza: i64,
    pub scan_iv: u8,
    pub admins_users: Vec<String>,
}

impl FromRow for City {
    fn from_row_opt(mut row: Row) -> Result<Self, mysql_async::FromRowError> {
        let id = row.take("id").expect("MySQL city.id error");
        let name = row.take("name").expect("MySQL city.name error");
        let coords = row.take::<String, _>("coordinates").expect("MySQL city.coordinates encoding error");
        let coords = coords.replace(char::is_whitespace, "");

        let poly: Vec<Point<f64>> = if coords.len() < 2 {
            error!("City \"{}\" ({}) has empty coordinates", name, id);
            Vec::new()
        } else {
            coords[1..(coords.len() - 1)]
                .split("),(")
                .filter_map(|s| {
                    let x_y: Vec<f64> = s
                        .split(',')
                        .map(|s| match s.parse::<f64>() {
                            Ok(f) => f,
                            Err(_) => panic!("Error parsing \"{}\" as a float", s),
                        })
                        .collect();
                    if x_y.len() == 2 {
                        Some(Point::new(x_y[0], x_y[1]))
                    } else {
                        error!("City \"{}\" ({}) has invalid coordinates", name, id);
                        None
                    }
                })
                .collect()
        };

        Ok(City {
            id,
            name,
            coordinates: Polygon::new(poly.into(), vec![]),
            scadenza: row.take("scadenza").expect("MySQL city.scadenza error"),
            scan_iv: row.take("monitor").expect("MySQL city.monitor error"),
            admins_users: row
                .take::<String, _>("admins_users")
                .expect("MySQL city.admins_users error")
                .split_whitespace()
                .map(|s| s.to_owned())
                .collect(),
        })
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

impl FromRow for CityPark {
    fn from_row_opt(mut row: Row) -> Result<Self, mysql_async::FromRowError> {
        let id = row.take("id").expect("MySQL city_parks.id error");
        let city_id = row.take("city_id").expect("MySQL city_parks.city_id error");
        let coords = row.take::<String, _>("coordinates").expect("MySQL city_parks.coordinates encoding error");
        let coords = coords.replace(char::is_whitespace, "");

        let poly: Vec<Point<f64>> = if coords.len() < 2 {
            error!("Park {} has empty coordinates", id);
            Vec::new()
        } else {
            coords[1..(coords.len() - 1)]
                .split("),(")
                .filter_map(|s| {
                    let x_y: Vec<f64> = s
                        .split(',')
                        .filter_map(|s| {
                            s.parse::<f64>().map_err(|e| error!("Error parsing \"{}\" as a float: {}", s, e)).ok()
                        })
                        .collect();
                    if x_y.len() == 2 {
                        Some(Point::new(x_y[0], x_y[1]))
                    } else {
                        error!("Park {} has invalid coordinates", id);
                        None
                    }
                })
                .collect()
        };

        Ok(CityPark { id, city_id, coordinates: Polygon::new(poly.into(), vec![]) })
    }
}

async fn load_pokemons() -> Result<(), ()> {
    let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn
        .query_iter("SELECT * FROM pokemon_list")
        .await
        .map_err(|e| error!("MySQL query error: get pokemon list\n{}", e))?;

    let data = res
        .stream_and_drop::<Pokemon>()
        .await
        .map_err(|e| error!("MySQL load_pokemons error: {}", e))?
        .ok_or_else(|| error!("MySQL load_pokemons empty"))?
        .map_ok(|p| (p.id, p))
        .try_collect()
        .await
        .map_err(|e| error!("MySQL load_pokemons collect error: {}", e))?;
    LIST.swap(Arc::new(data));

    Ok(())
}

async fn load_moves() -> Result<(), ()> {
    let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn
        .query_iter("SELECT id, move FROM pokemon_moves")
        .await
        .map_err(|e| error!("MySQL query error: get pokemon moves\n{}", e))?;

    let data = res
        .stream_and_drop::<(u16, String)>()
        .await
        .map_err(|e| error!("MySQL load_moves error: {}", e))?
        .ok_or_else(|| error!("MySQL load_moves empty"))?
        .try_collect()
        .await
        .map_err(|e| error!("MySQL load_moves collect error: {}", e))?;
    MOVES.swap(Arc::new(data));

    Ok(())
}

async fn load_forms() -> Result<(), ()> {
    let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn
        .query_iter("SELECT * FROM pokemon_forms")
        .await
        .map_err(|e| error!("MySQL query error: get pokemon forms\n{}", e))?;

    let data = res
        .stream_and_drop::<Form>()
        .await
        .map_err(|e| error!("MySQL load_forms error: {}", e))?
        .ok_or_else(|| error!("MySQL load_forms empty"))?
        .map_ok(|f| (f.id, f))
        .try_collect()
        .await
        .map_err(|e| error!("MySQL load_forms collect error: {}", e))?;
    FORMS.swap(Arc::new(data));

    Ok(())
}

async fn load_grunts() -> Result<(), ()> {
    let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn
        .query_iter("SELECT * FROM grunt_types")
        .await
        .map_err(|e| error!("MySQL query error: get grunt types\n{}", e))?;

    let data = res
        .stream_and_drop::<GruntType>()
        .await
        .map_err(|e| error!("MySQL load_grunts error: {}", e))?
        .ok_or_else(|| error!("MySQL load_grunts empty"))?
        .map_ok(|gt| (gt.id, gt))
        .try_collect()
        .await
        .map_err(|e| error!("MySQL load_grunts collect error: {}", e))?;
    GRUNTS.swap(Arc::new(data));

    Ok(())
}

pub async fn load_cities() -> Result<(), ()> {
    let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn
        .query_iter("SELECT id, name, coordinates, scadenza, monitor, admins_users FROM city")
        .await
        .map_err(|e| error!("MySQL query error: get cities\n{}", e))?;

    let data = res
        .stream_and_drop::<City>()
        .await
        .map_err(|e| error!("MySQL load_cities error: {}", e))?
        .ok_or_else(|| error!("MySQL load_cities empty"))?
        .map_ok(|c| (c.id, c))
        .try_collect()
        .await
        .map_err(|e| error!("MySQL load_cities collect error: {}", e))?;
    CITIES.swap(Arc::new(data));

    Ok(())
}

async fn load_parks() -> Result<(), ()> {
    let mut conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
    let res = conn
        .query_iter("SELECT id, city_id, coordinates FROM city_parks")
        .await
        .map_err(|e| error!("MySQL query error: get city parks\n{}", e))?;

    let data = res
        .stream_and_drop::<CityPark>()
        .await
        .map_err(|e| error!("MySQL load_cities error: {}", e))?
        .ok_or_else(|| error!("MySQL load_cities empty"))?
        .try_fold(HashMap::new(), |mut acc, cp| {
            let cityparks: &mut Vec<CityPark> = acc.entry(cp.city_id).or_default();
            cityparks.push(cp);
            future::ready(Ok(acc))
        })
        .await
        .map_err(|e| error!("MySQL load_cities collect error: {}", e))?;
    CITYPARKS.swap(Arc::new(data));

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
    }))
    .await;
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

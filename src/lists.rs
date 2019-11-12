use std::collections::HashMap;

use geo::Polygon;

use future_parking_lot::rwlock::RwLock;

use once_cell::sync::Lazy;

use crate::db::MYSQL;

pub static LIST: Lazy<HashMap<u16, Pokemon>> = Lazy::new(load_pokemons);
// pub static SCANNED: Lazy<Vec<u16>> = Lazy::new(|| LIST.iter().filter(|(_, p)| p.scanned == 1).map(|(id, _)| *id).collect());
// pub static COMMON: Lazy<Vec<u16>> = Lazy::new(|| LIST.iter().filter(|(_, p)| p.rarity < 2).map(|(id, _)| *id).collect());
// pub static RARES: Lazy<Vec<u16>> = Lazy::new(|| LIST.iter().filter(|(_, p)| p.rarity >= 2).map(|(id, _)| *id).collect());

pub static MOVES: Lazy<HashMap<u16, String>> = Lazy::new(load_moves);

pub static FORMS: Lazy<HashMap<u16, String>> = Lazy::new(load_forms);

pub static GRUNTS: Lazy<HashMap<u8, GruntType>> = Lazy::new(load_grunts);

pub static CITIES: Lazy<HashMap<u16, City>> = Lazy::new(load_cities);

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
    pub stats: RwLock<CityStats>,
}

#[derive(Default)]
pub struct CityStats {
    pub last_raid: Option<i64>,
    pub last_pokemon: Option<i64>,
    pub last_iv: Option<i64>,
    pub last_quest: Option<i64>,
    pub last_invasion: Option<i64>,
}

fn load_pokemons() -> HashMap<u16, Pokemon> {
    let mut conn = MYSQL.get_conn().expect("MySQL retrieve connection error");
    let res = conn.query("SELECT * FROM pokemon_list").expect("MySQL query error");

    let mut ret = HashMap::new();
    for r in res {
        let mut row = r.expect("MySQL row error");
        let id = row.take("id").expect("MySQL pokemon_list.id error");
        ret.insert(id, Pokemon {
            id,
            name: row.take("name").expect("MySQL pokemon_list.name error"),
            ptype: row.take("type").expect("MySQL pokemon_list.type error"),
            rarity: row.take("rarity").expect("MySQL pokemon_list.rarity error"),
            scanned: row.take("scanned").expect("MySQL pokemon_list.scanned error"),
            status: row.take("status").expect("MySQL pokemon_list.status error"),
            raid: row.take("raid").expect("MySQL pokemon_list.raid error"),
        });
    }
    ret
}

fn load_moves() -> HashMap<u16, String> {
    let mut conn = MYSQL.get_conn().expect("MySQL retrieve connection error");
    let res = conn.query("SELECT * FROM pokemon_moves").expect("MySQL query error");

    let mut ret = HashMap::new();
    for r in res {
        let mut row = r.expect("MySQL row error");
        ret.insert(row.take("id").expect("MySQL pokemon_moves.id error"), row.take("move").expect("MySQL pokemon_moves.move error"));
    }
    ret
}

fn load_forms() -> HashMap<u16, String> {
    let mut conn = MYSQL.get_conn().expect("MySQL retrieve connection error");
    let res = conn.query("SELECT * FROM pokemon_forms").expect("MySQL query error");

    let mut ret = HashMap::new();
    for r in res {
        let mut row = r.expect("MySQL row error");
        ret.insert(row.take("id").expect("MySQL pokemon_forms.id error"), row.take("name").expect("MySQL pokemon_forms.name error"));
    }
    ret
}

fn load_grunts() -> HashMap<u8, GruntType> {
    let mut conn = MYSQL.get_conn().expect("MySQL retrieve connection error");
    let res = conn.query("SELECT * FROM grunt_types").expect("MySQL query error");

    let mut ret = HashMap::new();
    for r in res {
        let mut row = r.expect("MySQL row error");
        let id = row.take("id").expect("MySQL grunt_types.id error");
        ret.insert(id, GruntType {
            id,
            name: row.take("name").expect("MySQL grunt_types.name error"),
            sex: row.take("sex").expect("MySQL grunt_types.type error"),
            element: row.take("type").expect("MySQL grunt_types.rarity error"),
        });
    }
    ret
}

fn load_cities() -> HashMap<u16, City> {
    let mut conn = MYSQL.get_conn().expect("MySQL retrieve connection error");
    let res = conn.query("SELECT id, name, coordinates, scadenza, monitor, admins_users FROM city WHERE scadenza > UNIX_TIMESTAMP()").expect("MySQL query error");

    let mut ret = HashMap::new();
    for r in res {
        let mut row = r.expect("MySQL row error");

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

        ret.insert(id, City {
            id,
            name: row.take("name").expect("MySQL city.name error"),
            coordinates: Polygon::new(poly.into(), vec![]),
            scadenza: row.take("scadenza").expect("MySQL city.scadenza error"),
            scan_iv: row.take("monitor").expect("MySQL city.monitor error"),
            admins_users: row.take("admins_users").expect("MySQL city.admins_users error"),
            stats: RwLock::new(CityStats::default()),
        });
    }
    ret
}

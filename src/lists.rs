use std::collections::HashMap;

use lazy_static::lazy_static;

use crate::db::MYSQL;

lazy_static! {
    pub static ref LIST: HashMap<u16, Pokemon> = load_pokemons();
    pub static ref SCANNED: Vec<u16> = LIST.iter().filter(|(_, p)| p.scanned == 1).map(|(id, _)| *id).collect();
    pub static ref COMMON: Vec<u16> = LIST.iter().filter(|(_, p)| p.rarity < 2).map(|(id, _)| *id).collect();
    pub static ref RARES: Vec<u16> = LIST.iter().filter(|(_, p)| p.rarity >= 2).map(|(id, _)| *id).collect();
}

lazy_static! {
    pub static ref MOVES: HashMap<u16, String> = load_moves();
}

lazy_static! {
    pub static ref FORMS: HashMap<u16, String> = load_forms();
}

lazy_static! {
    pub static ref GRUNTS: HashMap<u8, GruntType> = load_grunts();
}

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

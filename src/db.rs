use mysql::Pool;

use once_cell::sync::Lazy;

use crate::config::CONFIG;

pub static MYSQL: Lazy<Pool> = Lazy::new(|| Pool::new(&CONFIG.database.url).expect("Can't connect to MySQL"));

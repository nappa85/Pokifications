use mysql::Pool;

use lazy_static::lazy_static;

use crate::config::CONFIG;

lazy_static! {
    pub static ref MYSQL: Pool = Pool::new(&CONFIG.database.url).expect("Can't connect to MySQL");
}

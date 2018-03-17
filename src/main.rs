#![deny(warnings)]
#![deny(missing_docs)]

//! # pokefications
//!
//! RocketMap webhook clients
//!
//! A notifications daemon alternative to PokeAlarm

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
extern crate hyper;
extern crate toml;

mod pokifications;

use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use hyper::server::Http;
use toml::Value;

use pokifications::Pokifications as Pokifications;

/// Launch Pokifications according to config
fn main() {
    let args:Vec<String> = env::args().collect();

    //config file can be the first argument
    let config_file = if args.len() > 1 {
        args.get(1).expect("Cannot retrieve config path").to_owned()
    }
    else {
        let path = Path::new(args.get(0).expect("Cannot find executable path"));
        format!("config/{}.toml", path.file_stem().expect("Cannot find executable name").to_str().expect("Cannot parse executable name"))
    };
    let mut toml = File::open(&config_file).expect(&format!("File {} not found", config_file));
    let mut s = String::new();
    toml.read_to_string(&mut s).expect("Unable to read Toml file");
    //read config file in toml format
    let config:Value = toml::from_str(&s).expect("Syntax error on Tolm file");

    //retrieve address and port, defaulting if not configured
    let addr = format!("{}:{}", config["address"].as_str().expect("Error interpreting address value"), if config.get("port").is_none() {
            "80"
        } else {
            config["port"].as_str().expect("Error interpreting port value")
        }).parse().expect("Error parsing webserver address");

    //start webserver
    let server = Http::new().bind(&addr, || Ok(Pokifications)).expect("Error on webserver init");
    server.run().expect("Error on webserver run");
}

extern crate serde_json;
extern crate futures;

use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender};

use self::futures::executor::{self, Spawn};

use pokifications::entities::Request;

mod worker;
mod config;

use self::worker::Worker;
use self::config::Config;

pub struct Dispatcher {
    workers: HashMap<String, (Sender<Request>, Spawn<Worker>)>,
}

impl Dispatcher {
    pub fn new() -> Result<Dispatcher, String> {
        let mut instance = Dispatcher {
            workers: HashMap::new(),
        };
        instance.load_workers()?;
        Ok(instance)
    }

    fn load_workers(&mut self) -> Result<(), String> {
        let (request_tx, request_rx) = channel();

        let config = Config {};

        self.workers.insert(String::from("test"), (request_tx, executor::spawn(Worker::new(request_rx, config))));

        Ok(())
    }

    pub fn dispatch(&self, requests: Vec<Request>) -> Result<String, String> {
        for r in requests {
            for (key, &(ref request_sender, ref _thread)) in &self.workers {
                match request_sender.send(r.clone()) {
                    Err(e) => {
                        return Err(format!("Send error for {:?}: {:?}", key, e));
                    },
                    _ => {},
                }
            }
        }
        Ok(String::from("Ok"))
    }
}
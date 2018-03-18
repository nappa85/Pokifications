use std::sync::RwLock;

use pokifications::entities::{Request, Config};
use std::sync::mpsc::Receiver;

pub struct Worker {
    _request_recv: Receiver<Request>,
    _config_recv: Receiver<Config>,
    _config: RwLock<Config>,
}

impl Worker {
    pub fn new(request_rx: Receiver<Request>, config_rx: Receiver<Config>, config: Config) -> Worker {
        Worker {
            _request_recv: request_rx,
            _config_recv: config_rx,
            _config: RwLock::new(config),
        }
    }

    pub fn run(&self) {
        
    }
}

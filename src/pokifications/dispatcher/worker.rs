use std::sync::RwLock;

use pokifications::entities::{Request, Config};
use std::sync::mpsc::Receiver;

pub struct Worker {
    request_recv: Receiver<Request>,
    config_recv: Receiver<Config>,
    config: RwLock<Config>,
}

impl Worker {
    pub fn new(request_rx: Receiver<Request>, config_rx: Receiver<Config>, config: Config) -> Worker {
        Worker {
            request_recv: request_rx,
            config_recv: config_rx,
            config: RwLock::new(config),
        }
    }
}

impl Future for Worker {
    /// The type of the value returned when the future completes.
    type Item = ();

    /// The type representing errors that occured while processing the
    /// computation.
    type Error = String;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        println!("test");

        match self.config_recv.try_recv().and_then(|config| self.config.write()) {
            Ok(mut cfg) => { *cfg = config; },
            Err(e) => if e != TryRecvError::Empty { println!("Error updating configuration: {:?}", e); },
        }

        match self.request_recv.try_recv().and_then(|request| self.config.read()) {
            Ok(mut cfg) => { cfg.match(request); },
            Err(e) => if e != TryRecvError::Empty { println!("Error reading configuration: {:?}", e); },
        }

        Ok(Async::NotReady)
    }
}
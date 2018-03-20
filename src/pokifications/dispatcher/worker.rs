extern crate futures;

use std::sync::RwLock;

use self::futures::{Async, Future};

use pokifications::entities::{Request, Config};
use std::sync::mpsc::{Receiver, TryRecvError};

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

        match (self.config_recv.try_recv().map_err(|e| if e == TryRecvError::Empty { String::from("") } else { format!("{:?}", e) }))
            .and_then(|config| (self.config.write().map_err(|e| format!("{:?}", e)))
                .and_then(|mut cfg| { *cfg = config; Ok(()) })
            ) {
            Ok(_) => {},
            Err(e) => if e.len() > 0 { println!("Error updating configuration: {:?}", e); },
        }

        match (self.request_recv.try_recv().map_err(|e| if e == TryRecvError::Empty { String::from("") } else { format!("{:?}", e) }))
            .and_then(|request| (self.config.read().map_err(|e| format!("{:?}", e)))
                .and_then(|cfg| { (*cfg).matches(request); Ok(()) })
            ) {
            Ok(_) => {},
            Err(e) => if e.len() > 0 { println!("Error reading configuration: {:?}", e); },
        }

        Ok(Async::NotReady)
    }
}
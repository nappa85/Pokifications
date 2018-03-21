extern crate futures;

use self::futures::{Async, Future};

use pokifications::entities::Request;
use pokifications::dispatcher::config::Config;
use std::sync::mpsc::{Receiver, TryRecvError};

pub struct Worker {
    request_recv: Receiver<Request>,
    config: Config,
}

impl Worker {
    pub fn new(request_rx: Receiver<Request>, config: Config) -> Worker {
        Worker {
            request_recv: request_rx,
            config: config,
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

        match self.request_recv.try_recv() {
            Ok(request) => { self.config.matches(request); },
            Err(e) => if e != TryRecvError::Empty { println!("Error reading configuration: {:?}", e); },
        }

        Ok(Async::NotReady)
    }
}
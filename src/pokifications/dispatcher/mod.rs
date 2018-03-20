extern crate notify;
extern crate serde_json;
extern crate futures;

use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::time::Duration;
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::ffi::OsString;
use std::io::Read;

use self::futures::executor::{self, Spawn};

use pokifications::entities::{Request, Config};

mod worker;

use self::worker::Worker;

use self::notify::{RecommendedWatcher, Watcher, RecursiveMode, DebouncedEvent};

pub struct Dispatcher<'a> {
    _watcher: RecommendedWatcher,
    _watch_recv: Receiver<DebouncedEvent>,
    workers: HashMap<OsString, (Sender<Request>, Sender<Config>, Spawn<Worker>)>,
    config_path: &'a Path,
}

impl<'a> Dispatcher<'a> {
    pub fn new() -> Result<Dispatcher<'a>, String> {
        let config_path = Path::new("config");
        let (tx, rx) = channel();

        //those expect are fine, if it fails we want to panic!
        let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_secs(2)).expect("Unable to init config watcher");
        watcher.watch(config_path, RecursiveMode::NonRecursive).expect("Unable to watch config dir");

        let mut instance = Dispatcher {
            _watcher: watcher,
            _watch_recv: rx,
            workers: HashMap::new(),
            config_path: config_path,
        };
        instance.load_workers()?;
        Ok(instance)
    }

    fn load_workers(&mut self) -> Result<(), String> {
        //read config dir
        (fs::read_dir(self.config_path)
            //transform io::Error to String
            .map_err(|e| format!("Error opening config dir: {:?}", e))
        ).and_then(|entries| {
            //cycle entries
            for entry in entries {
                //transform io::Error to String
                match entry.map_err(|e| format!("Error scanning config dir: {:?}", e))
                    //open file
                    .and_then(|file| {
                        self.read_config(file.path())
                        .and_then(|config| {
                            let (request_tx, request_rx) = channel();
                            let (config_tx, config_rx) = channel();

                            self.workers.insert(file.file_name(), (request_tx, config_tx, executor::spawn(Worker::new(request_rx, config_rx, config))));

                            Ok(())
                        })
                    }) {
                    Err(e) => {
                        return Err(e);
                    },
                    Ok(()) => {},
                }
            }
            Ok(())
        })
    }

    fn read_config(&self, path: PathBuf) -> Result<Config, String> {
        //open file
        (File::open(&path)
                //transform io::Error to String
                .map_err(|e| format!("File {:?} not found: {:?}", path, e))
            )
            .and_then(|mut file| {
                //read file to string
                let mut contents = String::new();
                (file.read_to_string(&mut contents)
                    //transform io::Error to String
                    .map_err(|e| format!("Unable to read config file {:?}: {:?}", path, e))
                )
                .and_then(|_| serde_json::from_str::<Config>(&contents)
                    //transform serde_json::Error to String
                    .map_err(|e| format!("Error decoding config file {:?}: {:?}", path, e))
                )
            })
    }

    pub fn dispatch(&self, requests: Vec<Request>) -> Result<String, String> {
        for r in requests {
            for (key, &(ref request_sender, ref _config_sender, ref _thread)) in &self.workers {
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
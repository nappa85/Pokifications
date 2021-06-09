use std::{collections::HashMap, future::Future, io, path::PathBuf, sync::Arc, time::Duration};

use tokio::{fs::File, sync::{RwLock, RwLockReadGuard, RwLockWriteGuard}, time::sleep};

use once_cell::sync::Lazy;

static REGISTRY: Lazy<FileRegistry> = Lazy::new(FileRegistry::new);

/// This struct works as an async file handler where file is created only once
pub struct FileRegistry {
    inner: RwLock<HashMap<PathBuf, Arc<RwLock<()>>>>,
}

impl FileRegistry {
    fn new() -> Self {
        FileRegistry {
            inner: RwLock::new(HashMap::new()),
        }
    }

    async fn read(&'_ self) -> RwLockReadGuard<'_, HashMap<PathBuf, Arc<RwLock<()>>>> {
        self.inner.read().await
    }

    async fn write(&'_ self) -> RwLockWriteGuard<'_, HashMap<PathBuf, Arc<RwLock<()>>>> {
        self.inner.write().await
    }

    async fn _get(&self, path: &PathBuf) -> Option<Result<File, std::io::Error>> {
        let lock = REGISTRY.read().await;
        if let Some(rw) = lock.get(path) {
            rw.read().await;
            Some(File::open(path).await)
        }
        else {
            None
        }
    }

    pub async fn get<C, F>(path: PathBuf, callback: C) -> Result<File, io::Error>
    where
        C: Fn(&PathBuf) -> F,
        F: Future<Output=Result<File, io::Error>>
    {
        // check if entry exists
        if let Some(res) = REGISTRY._get(&path).await {
            return res;
        }

        // entry still doesn't exists
        let mut lock = REGISTRY.write().await;
        if lock.get(&path).is_some() {
            // we are late, drop write lock to dequeue and retry
            drop(lock);
            if let Some(res) = REGISTRY._get(&path).await {
                return res;
            }
            else {
                return Err(io::Error::from(io::ErrorKind::NotFound));
            }
        }

        // create the entry and write-lock it
        let inner = Arc::new(RwLock::new(()));
        lock.insert(path.to_owned(), Arc::clone(&inner));
        let temp = inner.write().await;
        drop(lock);

        // create the file
        let res = callback(&path).await;
        // delete the entry in one hour
        tokio::spawn(async move {
            sleep(Duration::from_secs(3600)).await;
            let mut lock = REGISTRY.write().await;
            lock.remove(&path);
        });
        //free write lock and return
        drop(temp);
        res
    }
}

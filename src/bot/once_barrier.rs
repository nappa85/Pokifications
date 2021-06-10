use std::{cmp::Eq, collections::HashMap, future::Future, hash::Hash, sync::Arc, time::Duration};

use tokio::{sync::RwLock, time::sleep};

/// This struct works as an async barrier like OnceCell, but locks disappear from memory after a given time, and there are different callbacks for read and write
pub struct OnceBarrier<T>
where T: Hash + Eq + Clone + Send + Sync + 'static {
    delay: Duration,
    inner: Arc<RwLock<HashMap<T, Arc<RwLock<()>>>>>,
}

impl<T> OnceBarrier<T>
where T: Hash + Eq + Clone + Send + Sync + 'static {
    pub fn new(delay: Duration) -> Self {
        OnceBarrier {
            delay,
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn can_read(&self, key: &T) -> bool {
        let lock = self.inner.read().await;
        if let Some(rw) = lock.get(key) {
            rw.read().await;
            true
        }
        else {
            false
        }
    }

    pub async fn get<Read, Write, ReadFut, WriteFut, Out>(&self, key: T, read_callback: Read, write_callback: Write) -> Out
    where
        // at the moment it's impossible to express the lifetime  of the future, until HRTB is stable
        // therefore we need to pass the owned value T instead of &T
        Read: FnOnce(T) -> ReadFut,
        Write: FnOnce(T) -> WriteFut,
        ReadFut: Future<Output=Out>,
        WriteFut: Future<Output=Out>,
    {
        // check if entry exists
        if self.can_read(&key).await {
            return read_callback(key).await;
        }

        // entry still doesn't exists
        let mut lock = self.inner.write().await;
        if lock.get(&key).is_some() {
            // we are late, drop write lock to dequeue and retry
            drop(lock);
            if self.can_read(&key).await {
                return read_callback(key).await;
            }
            else {
                unreachable!();
            }
        }

        // create the entry and write-lock it
        let inner = Arc::new(RwLock::new(()));
        lock.insert(key.clone(), Arc::clone(&inner));
        let temp = inner.write().await;
        drop(lock);

        // create the file
        let res = write_callback(key.clone()).await;
        // delete the entry after given time
        let delay = self.delay;
        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move {
            sleep(delay).await;
            let mut lock = inner.write().await;
            lock.remove(&key);
        });
        // free write lock and return
        drop(temp);
        res
    }
}

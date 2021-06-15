use std::{future::Future, hash::Hash, sync::Arc};

use lru::LruCache;

use tokio::sync::{OnceCell, Mutex};

pub struct FileCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    inner: Mutex<LruCache<K, Arc<OnceCell<V>>>>,
}

impl<K, V> FileCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    pub fn new(size: usize) -> Self {
        FileCache {
            inner: Mutex::new(LruCache::new(size)),
        }
    }

    pub async fn get<Create, CreateFut>(&self, key: K, create: Create) -> V
    where
        Create: FnOnce(K) -> CreateFut,
        CreateFut: Future<Output=V>,
    {
        let mut lock = self.inner.lock().await;
        let oc = if let Some(temp) = lock.get(&key) {
            Arc::clone(temp)
        }
        else {
            let oc = Arc::new(OnceCell::new());
            lock.put(key.clone(), Arc::clone(&oc));
            oc
        };
        drop(lock);
        return oc.get_or_init(|| async move {
            create(key).await
        }).await.clone()
    }
}

use futures::lock::Mutex;
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

pub struct Memoized<T> {
    supplier: Box<dyn Fn() -> Pin<Box<dyn Future<Output = T>>> + Send + Sync>,
    timeout: Duration,
    cached: Mutex<Cached<T>>,
}

pub struct Cached<T> {
    at: Instant,
    value: T,
}

impl<T> Memoized<T>
where
    T: Clone,
{
    pub async fn new<S, R>(timeout: Duration, supplier: S) -> Self
    where
        S: (Fn() -> R) + Send + Sync + 'static,
        R: Future<Output = T> + 'static,
    {
        let cached = supplier().await;
        Memoized {
            supplier: Box::new(move || Box::pin(supplier())),
            timeout,
            cached: Mutex::new(Cached {
                at: Instant::now(),
                value: cached,
            }),
        }
    }

    pub async fn get(&self) -> T {
        let mut cached = self.cached.lock().await;
        if cached.at.elapsed() < self.timeout {
            return cached.value.clone();
        }
        let new_value = (self.supplier)().await;
        *cached = Cached {
            at: Instant::now(),
            value: new_value,
        };
        cached.value.clone()
    }
}

mod tests {
    #![allow(unused_imports)]
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI32, Ordering};

    #[actix_web::test]
    pub async fn test() {
        let counter = Arc::new(AtomicI32::new(1));
        let memoized = Memoized::new(Duration::from_secs(5), move || {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::Relaxed) + 1
            }
        })
        .await;
        assert_eq!(memoized.get().await, 2);
        assert_eq!(memoized.get().await, 2);
    }
}

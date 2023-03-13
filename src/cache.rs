// use futures::lock::Mutex;
// use std::future::Future;
// use std::pin::Pin;
// use std::time::{Duration, Instant};
//
// pub struct Memoized<T> {
//     supplier: Pin<Box<dyn Fn() -> Pin<Box<dyn Future<Output = T>>>>>,
//     timeout: Duration,
//     cached: Mutex<Cached<T>>,
// }
//
// pub struct Cached<T> {
//     at: Instant,
//     value: T,
// }
//
// impl<T> Memoized<T>
// where
//     T: Clone,
// {
//     pub async fn new(
//         timeout: Duration,
//         supplier: impl Fn() -> Pin<Box<dyn Future<Output = T>>> + 'static,
//     ) -> Self {
//         let cached = supplier().await;
//         Memoized {
//             supplier: Box::pin(supplier),
//             timeout,
//             cached: Mutex::new(Cached {
//                 at: Instant::now(),
//                 value: cached,
//             }),
//         }
//     }
//
//     pub async fn get(&self) -> T {
//         let mut cached = self.cached.lock().await;
//         if cached.at.elapsed() < self.timeout {
//             return cached.value.clone();
//         }
//         let new_value = (self.supplier)().await;
//         *cached = Cached {
//             at: Instant::now(),
//             value: new_value,
//         };
//         cached.value.clone()
//     }
// }

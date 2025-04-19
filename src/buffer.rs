//! The [`ServiceExt::buffer`](crate::ServiceExt::buffer) combinator returns [`Buffer`], whose
//! [`Service::acquire`] immediately resolves until the buffer is at maximum capacity, at which point
//! it defers to the inner service's [`Service::acquire`]. The buffer is drained when the inner
//! service's permit becomes available.
//!
//! # Example
//!
//! ```rust
//! use burger::*;
//! # use tokio::{join, time::sleep};
//! # use std::time::Duration;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let svc = service_fn(|x| async move {
//!     sleep(Duration::from_secs(1)).await;
//!     x + 1
//! })
//! .concurrency_limit(1)
//! .buffer(2)
//! .load_shed();
//! let (a, b, c, d) = join! {
//!     svc.oneshot(9),
//!     svc.oneshot(2),
//!     svc.oneshot(1),
//!     svc.oneshot(5)
//! };
//! assert_eq!(a, Ok(10));
//! assert_eq!(b, Ok(3));
//! assert_eq!(c, Ok(2));
//! assert_eq!(d, Err(5));
//! # }
//! ```
//!
//! # Load
//!
//! The [`Load::load`] on [`Buffer`] defers to the inner service.

use futures_util::FutureExt;
use tokio::sync::Semaphore;

use crate::{load::Load, Middleware, Service};

/// A wrapper [`Service`] for the [`ServiceExt::buffer`](crate::ServiceExt::buffer) combinator.
///
/// See the [module](crate::buffer) for more information.
#[derive(Debug)]
pub struct Buffer<S> {
    inner: S,
    semaphore: Semaphore,
}

impl<S> Buffer<S> {
    pub(crate) fn new(inner: S, capacity: usize) -> Self {
        Self {
            inner,
            semaphore: Semaphore::new(capacity),
        }
    }
}

impl<Request, S> Service<Request> for Buffer<S>
where
    S: Service<Request>,
{
    type Response = S::Response;

    async fn acquire(&self) -> impl AsyncFnOnce(Request) -> Self::Response {
        let permit = self.inner.acquire().now_or_never();
        async |request| {
            if let Some(permit) = permit {
                permit(request).await
            } else {
                let semaphore_permit = self.semaphore.acquire().await;
                let permit = self.inner.acquire().await;
                drop(semaphore_permit);
                permit(request).await
            }
        }
    }
}

impl<S> Load for Buffer<S>
where
    S: Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}

impl<S, T> Middleware<S> for Buffer<T>
where
    T: Middleware<S>,
{
    type Service = Buffer<T::Service>;

    fn apply(self, svc: S) -> Self::Service {
        let Self { inner, semaphore } = self;
        Buffer {
            inner: inner.apply(svc),
            semaphore,
        }
    }
}

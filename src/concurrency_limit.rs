//! The [`ServiceExt::concurrency_limit`](crate::ServiceExt::concurrency_limit) combinator returns
//! [`ConcurrencyLimit`] which restricts the number of inflight [calls](Service::call) to a specified
//! value.
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
//!     2 * x
//! })
//! .concurrency_limit(1);
//! let (a, b) = join! {
//!     svc.oneshot(6),
//!     svc.oneshot(2)
//! };
//! # }
//! ```
//!
//! # Load
//!
//! The [`Load::load`] on [ConcurrencyLimit] defers to the inner service.

use tokio::sync::Semaphore;

use crate::{load::Load, Middleware, Service};

/// A wrapper for the [`ServiceExt::concurrency_limit`](crate::ServiceExt::concurrency_limit)
/// combinator.
///
/// See the [module](crate::concurrency_limit) for more information.
#[derive(Debug)]
pub struct ConcurrencyLimit<S> {
    inner: S,
    semaphore: Semaphore,
}

impl<S> ConcurrencyLimit<S> {
    pub(crate) fn new(inner: S, n_permits: usize) -> Self {
        Self {
            inner,
            semaphore: Semaphore::new(n_permits),
        }
    }
}

impl<Request, S> Service<Request> for ConcurrencyLimit<S>
where
    S: Service<Request>,
{
    type Response = S::Response;

    async fn acquire(&self) -> impl AsyncFnOnce(Request) -> Self::Response {
        let semaphore = self.semaphore.acquire().await.expect("not closed");
        let permit = self.inner.acquire().await;
        async |request| {
            let response = permit(request).await;
            drop(semaphore);
            response
        }
    }
}

impl<S> Load for ConcurrencyLimit<S>
where
    S: Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}

impl<S, T> Middleware<S> for ConcurrencyLimit<T>
where
    T: Middleware<S>,
{
    type Service = ConcurrencyLimit<T::Service>;

    fn apply(self, svc: S) -> Self::Service {
        let Self { inner, semaphore } = self;
        ConcurrencyLimit {
            inner: inner.apply(svc),
            semaphore,
        }
    }
}

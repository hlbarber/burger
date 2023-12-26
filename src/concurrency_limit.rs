//! The [ServiceExt::concurrency_limit](crate::ServiceExt::concurrency_limit) combinator restricts
//! the number of inflight [Service::call]s to a specified value.
//!
//! # Example
//!
//! ```rust
//! use burger::*;
//! use tokio::{join, time::sleep};
//!
//! use std::time::Duration;
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
//! The [Load::load] on [ConcurrencyLimit] defers to the inner service.

use std::fmt;

use tokio::sync::{Semaphore, SemaphorePermit};

use crate::{load::Load, Service};

/// A wrapper for the [ServiceExt::concurrency_limit](crate::ServiceExt::concurrency_limit)
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

/// The [Service::Permit] type for [ConcurrencyLimit].
pub struct ConcurrencyLimitPermit<'a, S, Request>
where
    S: Service<Request> + 'a,
{
    inner: S::Permit<'a>,
    _semaphore_permit: SemaphorePermit<'a>,
}

impl<'a, S, Request> fmt::Debug for ConcurrencyLimitPermit<'a, S, Request>
where
    S: Service<Request>,
    S::Permit<'a>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConcurrencyLimitPermit")
            .field("inner", &self.inner)
            .field("_semaphore_permit", &self._semaphore_permit)
            .finish()
    }
}

impl<Request, S> Service<Request> for ConcurrencyLimit<S>
where
    S: Service<Request>,
{
    type Response = S::Response;
    type Permit<'a> = ConcurrencyLimitPermit<'a, S, Request>
    where
        S: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        ConcurrencyLimitPermit {
            inner: self.inner.acquire().await,
            _semaphore_permit: self.semaphore.acquire().await.expect("not closed"),
        }
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        S::call(permit.inner, request).await
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

//! In [`burger`](crate) backpressure is exerted by [`Service::acquire`]. The
//! [`ServiceExt::depressurize`] combinator returns [`Depressurize`], which moves the
//! [`Service::acquire`] execution into the [`Service::call`], causing [`Service::acquire`] to
//! resolve immediately.
//!
//! # Example
//!
//! ```rust
//! use burger::*;
//! # use futures::FutureExt;
//! # use tokio::time::sleep;
//! # use std::time::Duration;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let svc = service_fn(|x: usize| async move {
//!     sleep(Duration::from_secs(1)).await;
//!     x.to_string()
//! })
//! .concurrency_limit(1)
//! .depressurize();
//! let permit = svc.acquire().now_or_never().unwrap();
//! # }
//! ```
//!
//! # Load
//!
//! The [`Load::load`] on [`Depressurize`] defers to the inner service.

use crate::{load::Load, Middleware, Service};

/// A wrapper for the [`ServiceExt::depressurize`] combinator.
///
/// See the [module](crate::depressurize) for more information.
#[derive(Clone, Debug)]
pub struct Depressurize<S> {
    inner: S,
}

impl<S> Depressurize<S> {
    pub(crate) fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<Request, S> Service<Request> for Depressurize<S>
where
    S: Service<Request>,
{
    type Response = S::Response;

    async fn acquire(&self) -> impl AsyncFnOnce(Request) -> Self::Response {
        async |request| self.inner.acquire().await(request).await
    }
}

impl<S> Load for Depressurize<S>
where
    S: Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}

impl<S, T> Middleware<S> for Depressurize<T>
where
    T: Middleware<S>,
{
    type Service = Depressurize<T::Service>;

    fn apply(self, svc: S) -> Self::Service {
        let Self { inner } = self;
        Depressurize {
            inner: inner.apply(svc),
        }
    }
}

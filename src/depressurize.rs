//! In [burger](crate) backpressure is exerted by [Service::acquire]. The
//! [ServiceExt::depressurize] combinator moves the [Service::acquire] execution into the
//! [Service::call], causing [Service::acquire] to resolve immediately.
//!
//! # Example
//!
//! ```rust
//! use burger::*;
//! use futures::FutureExt;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let svc = service_fn(|x: usize| async move { x.to_string() })
//!     .concurrency_limit(1)
//!     .depressurize();
//! let permit = svc.acquire().now_or_never().unwrap();
//! # }
//! ```
//!
//! # Load
//!
//! The [Load::load] on [Depressurize] defers to the inner service.
//!

use crate::{load::Load, Service, ServiceExt};

/// A wrapper for the [ServiceExt::depressurize] combinator.
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
    type Permit<'a> = &'a S
    where
        S: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        &self.inner
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        permit.oneshot(request).await
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

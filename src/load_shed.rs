//! The [`ServiceExt::load_shed`](crate::ServiceExt::load_shed) combinator returns [`LoadShed`], which
//! causes [`Service::acquire`] to immediately return [`Some(permit)`](Some) when the inner
//! [permit](Service::Permit) is ready and immediately return [`None`] otherwise.
//!
//! This may be used to discard any work which cannot be permitted at the time [`Service::acquire`]
//! is called.
//!
//! This is a relative of [`ServiceExt::depressurize`](crate::ServiceExt::depressurize), which
//! immediately accepts all work.
//!
//! # Example
//!
//! ```rust
//! use burger::*;
//! # use tokio::time::sleep;
//! # use std::time::Duration;
//!
//! # #[tokio::main]
//! async fn main() {
//! let svc = service_fn(|x| async move {
//!     sleep(Duration::from_secs(1)).await;
//!     x + 5
//! })
//! .concurrency_limit(1)
//! .load_shed();
//! let (a, b) = tokio::join! {
//!     svc.oneshot(32),
//!     svc.oneshot(31)
//! };
//! assert_eq!(a, Ok(37));
//! assert_eq!(b, Err(31));
//! # }
//! ```
//!
//! # Load
//!
//! The [`Load::load`] on [LoadShed] defers to the inner service.

use futures_util::FutureExt;

use crate::{load::Load, Middleware, Service};

/// A wrapper [`Service`] for the [`ServiceExt::load_shed`](crate::ServiceExt::load_shed)
/// combinator.
///
/// See the [module](crate::load_shed) for more information.
#[derive(Clone, Debug)]
pub struct LoadShed<S> {
    inner: S,
}

impl<S> LoadShed<S> {
    pub(crate) fn new(inner: S) -> Self {
        LoadShed { inner }
    }
}

impl<Request, S> Service<Request> for LoadShed<S>
where
    S: Service<Request>,
{
    type Response = Result<S::Response, Request>;
    type Permit<'a>
        = Option<S::Permit<'a>>
    where
        S: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        self.inner.acquire().now_or_never()
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        if let Some(permit) = permit {
            Ok(S::call(permit, request).await)
        } else {
            Err(request)
        }
    }
}

impl<S> Load for LoadShed<S>
where
    S: Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}

impl<S, T> Middleware<S> for LoadShed<T>
where
    T: Middleware<S>,
{
    type Service = LoadShed<T::Service>;

    fn apply(self, svc: S) -> Self::Service {
        let Self { inner } = self;
        LoadShed {
            inner: inner.apply(svc),
        }
    }
}

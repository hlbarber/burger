//! The [`ServiceExt::then`](crate::ServiceExt::then) combinator returns [`Then`], which extends a
//! service with a closure modifying a [`Service::Response`] asynchronously.
//!
//! For a synchronous version see the [map](mod@crate::map) module.
//!
//! # Example
//!
//! ```rust
//! use burger::*;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let svc = service_fn(|x| async move { 7 * x }).then(|x| async move { x + 1 });
//! let response = svc.oneshot(2u32).await;
//! assert_eq!(response, 15);
//! # }
//! ```
//!
//! # Load
//!
//! The [`Load::load`] on [Then] defers to the inner service.

use std::future::Future;

use crate::{load::Load, Middleware, Service};

/// A wrapper for the [`ServiceExt::then`](crate::ServiceExt::then) combinator.
///
/// See the [module](crate::then) for more information.
#[derive(Debug, Clone)]
pub struct Then<S, F> {
    inner: S,
    closure: F,
}

impl<S, F> Then<S, F> {
    pub(crate) fn new(inner: S, closure: F) -> Self {
        Self { inner, closure }
    }
}

impl<Request, S, F, Fut> Service<Request> for Then<S, F>
where
    S: Service<Request>,
    F: Fn(S::Response) -> Fut,
    Fut: Future,
{
    type Response = Fut::Output;

    async fn acquire(&self) -> impl AsyncFnOnce(Request) -> Self::Response {
        let inner_permit = self.inner.acquire().await;
        async |request| (self.closure)(inner_permit(request).await).await
    }
}

impl<S, F> Load for Then<S, F>
where
    S: Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}

impl<S, T, F> Middleware<S> for Then<T, F>
where
    T: Middleware<S>,
{
    type Service = Then<T::Service, F>;

    fn apply(self, svc: S) -> Self::Service {
        let Self { inner, closure } = self;
        Then {
            inner: inner.apply(svc),
            closure,
        }
    }
}

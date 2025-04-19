//! The [`ServiceExt::map`](crate::ServiceExt::map) combinator returns [`Map`], which extends a
//! service with a specified closure from the modifying the [`Service::Response`].
//!
//! For an asynchronous version of this combinator see [then](mod@crate::then) module.
//!
//! # Example
//!
//! ```rust
//! use burger::*;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let svc = service_fn(|x: u32| async move { x.to_string() }).map(|x: String| x.parse());
//! let response: usize = svc.oneshot(32).await.unwrap();
//! assert_eq!(response, 32);
//! # }
//! ```
//!
//! # Load
//!
//! [`Load`](crate::load::Load) measurements defer to the inner service.

use crate::{Middleware, Service};

/// A wrapper [`Service`] for the [`ServiceExt::map`](crate::ServiceExt::map) combinator.
///
/// See the [module](crate::map) for more information.
#[derive(Clone, Debug)]
pub struct Map<S, F> {
    inner: S,
    closure: F,
}

impl<S, F> Map<S, F> {
    pub(crate) fn new(inner: S, closure: F) -> Self {
        Self { inner, closure }
    }
}

impl<Request, S, F, Output> Service<Request> for Map<S, F>
where
    S: Service<Request>,
    F: Fn(S::Response) -> Output,
{
    type Response = Output;

    async fn acquire(&self) -> impl AsyncFnOnce(Request) -> Self::Response {
        let permit = self.inner.acquire().await;
        async |request| (self.closure)(permit(request).await)
    }
}

impl<S, T, F> Middleware<S> for Map<T, F>
where
    T: Middleware<S>,
{
    type Service = Map<T::Service, F>;

    fn apply(self, svc: S) -> Self::Service {
        let Self { inner, closure } = self;
        Map {
            inner: inner.apply(svc),
            closure,
        }
    }
}

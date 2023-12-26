//! The [ServiceExt::then](crate::ServiceExt::then) combinator extends a service with a closure
//! accepting the current [Service::Response] and returning a [Future] whose [Future::Output] is
//! the new [Service::Response].
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
//! The [Load::load] on [Then] defers to the inner service.

use std::{any, fmt, future::Future};

use crate::{load::Load, Service};

/// A wrapper for the [ServiceExt::then](crate::ServiceExt::then) combinator.
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

/// The [Service::Permit] type for [Then].
pub struct ThenPermit<'a, S, F, Request>
where
    S: Service<Request> + 'a,
{
    inner: S::Permit<'a>,
    closure: &'a F,
}

impl<'a, S, F, Request> fmt::Debug for ThenPermit<'a, S, F, Request>
where
    S: Service<Request>,
    S::Permit<'a>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThenPermit")
            .field("inner", &self.inner)
            .field("closure", &format_args!("{}", any::type_name::<F>()))
            .finish()
    }
}

impl<Request, S, F, Fut> Service<Request> for Then<S, F>
where
    S: Service<Request>,
    F: Fn(S::Response) -> Fut,
    Fut: Future,
{
    type Response = Fut::Output;
    type Permit<'a> = ThenPermit<'a, S, F, Request> where S: 'a, F: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        ThenPermit {
            inner: self.inner.acquire().await,
            closure: &self.closure,
        }
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        (permit.closure)(S::call(permit.inner, request).await).await
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

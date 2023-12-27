//! The [ServiceExt::then](crate::ServiceExt::map) combinator returns [Map], which extends a
//! service with a specified closure from the modifying the [Service::Response].
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
//! [Load](crate::load::Load) measurements defer to the inner service.

use std::{any, fmt};

use crate::Service;

/// A wrapper [Service] for the [ServiceExt::map](crate::ServiceExt::map) combinator.
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

/// The [Service::Permit] type for [Map].
pub struct MapPermit<'a, S, F, Request>
where
    S: Service<Request> + 'a,
{
    inner: S::Permit<'a>,
    closure: &'a F,
}

impl<'a, S, F, Request> fmt::Debug for MapPermit<'a, S, F, Request>
where
    S: Service<Request>,
    S::Permit<'a>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapPermit")
            .field("inner", &self.inner)
            .field("closure", &format_args!("{}", any::type_name::<F>()))
            .finish()
    }
}

impl<Request, S, F, Output> Service<Request> for Map<S, F>
where
    S: Service<Request>,
    F: Fn(S::Response) -> Output,
{
    type Response = Output;
    type Permit<'a> = MapPermit<'a, S, F, Request> where S: 'a, F: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        MapPermit {
            inner: self.inner.acquire().await,
            closure: &self.closure,
        }
    }

    async fn call(permit: Self::Permit<'_>, request: Request) -> Self::Response {
        (permit.closure)(S::call(permit.inner, request).await)
    }
}

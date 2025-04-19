//! The [`service_fn`] function accepts a closure accepting a request and returning a [`Future`]
//! and returns [`ServiceFn`], a [`Service`] which is immediately permitted to run the closure.
//!
//! # Example
//!
//! ```rust
//! use burger::*;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let svc = service_fn(|x: u64| async move { x.to_string() });
//! let response = svc.oneshot(32).await;
//! assert_eq!(response, "32");
//! # }
//! ```
//!
//! # Load
//!
//! This has _no_ [`Load`](crate::load::Load) implementation.

use std::{any, fmt};

use crate::Service;

/// The [`Service`] returned by the [`service_fn`] constructor.
///
/// See the [module](mod@crate::service_fn) for more information.
#[derive(Clone)]
pub struct ServiceFn<F> {
    closure: F,
}

impl<F> fmt::Debug for ServiceFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceFn")
            .field("closure", &format_args!("{}", any::type_name::<F>()))
            .finish()
    }
}

impl<Request, F, Response> Service<Request> for ServiceFn<F>
where
    F: AsyncFn(Request) -> Response,
{
    type Response = Response;

    async fn acquire(&self) -> impl AsyncFnOnce(Request) -> Self::Response {
        &self.closure
    }
}

/// Constructs a [`Service`] from a closure.
///
/// See the [module](mod@crate::service_fn) for more details.
pub fn service_fn<F>(closure: F) -> ServiceFn<F> {
    ServiceFn { closure }
}

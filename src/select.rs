//! Given a collection of some [services](Service), [select] constructs a [Service] which uses the
//! first permit available.
//!
//! # Example
//!
//! ```rust
//! use burger::*;
//! # use tokio::time::sleep;
//! # use std::time::Duration;
//!  
//! pub struct Wait(u64);
//!
//! #[non_exhaustive]
//! pub struct WaitPermit<'a>(&'a u64);
//!
//! impl Service<u64> for Wait {
//!     type Response = u64;
//!     type Permit<'a> = WaitPermit<'a>;
//!
//!     async fn acquire(&self) -> Self::Permit<'_> {
//!         sleep(Duration::from_secs(self.0)).await;
//!         WaitPermit(&self.0)
//!     }
//!
//!     async fn call(permit: Self::Permit<'_>, request: u64) -> u64 {
//!         permit.0 * request
//!     }
//! }
//!
//! # #[tokio::main]
//! async fn main() {
//! let svcs: Vec<_> = (0..10).map(Wait).collect();
//! let svc = select(svcs);
//! let response = svc.oneshot(7).await;
//! assert_eq!(0, response);
//! # }
//! ```
//!
//! # Load
//!
//! This has _no_ [Load](crate::Load) implementation.
use std::{fmt, marker::PhantomData};

use futures_util::future::select_all;

use crate::Service;

/// A wrapper [Service] for the [select] constructor.
///
/// See the [module](mod@crate::select) for more information.
pub struct Select<S, I> {
    _inner: PhantomData<S>,
    services: I,
}

impl<S, I> fmt::Debug for Select<S, I>
where
    I: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Select")
            .field("_inner", &self._inner)
            .field("services", &self.services)
            .finish()
    }
}

impl<S, I> Clone for Select<S, I>
where
    I: Clone,
{
    fn clone(&self) -> Self {
        Self {
            _inner: self._inner,
            services: self.services.clone(),
        }
    }
}

impl<Request, S, I> Service<Request> for Select<S, I>
where
    for<'a> &'a I: IntoIterator<Item = &'a S>,
    S: Service<Request>,
{
    type Response = S::Response;
    type Permit<'a> = S::Permit<'a>
    where
        S: 'a, I: 'a;

    async fn acquire(&self) -> Self::Permit<'_> {
        // This `Box::pin` could be removed with `return_type_notation`.
        let iter = self.services.into_iter().map(|s| s.acquire()).map(Box::pin);
        let (permit, _, _) = select_all(iter).await;
        permit
    }

    async fn call<'a>(permit: Self::Permit<'a>, request: Request) -> Self::Response
    where
        Self: 'a,
    {
        S::call(permit, request).await
    }
}

/// Constructs a [Service] from a collection ([IntoIterator] must be implemented for its reference)
/// of services whose [Service::call] is the by the first available child.
///
/// See [module](mod@crate::select) for more information.
pub fn select<S, I>(services: I) -> Select<S, I> {
    Select {
        _inner: PhantomData,
        services,
    }
}
